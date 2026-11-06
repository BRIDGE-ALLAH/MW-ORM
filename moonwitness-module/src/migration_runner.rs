use async_trait::async_trait;
use thiserror::Error;

use crate::{AppliedMigration, MigrationCatalog, MigrationError, MigrationPlan};

/// Backend contract required by the migration coordinator.
///
/// Implementations must serialize runners that use the same namespace.
/// `apply_and_record` must not record a migration as applied unless its
/// operation has completed successfully. Backends should make the operation
/// and history write atomic where the database supports transactional DDL.
#[async_trait]
pub trait MigrationBackend: Send + Sync {
    /// Acquire an exclusive migration lock for an application namespace.
    async fn acquire_lock(&self, namespace: &str) -> Result<(), String>;

    /// Load applied migration identities and checksums for a namespace.
    async fn load_history(&self, namespace: &str) -> Result<Vec<AppliedMigration>, String>;

    /// Execute one migration and persist its applied-history record.
    async fn apply_and_record(&self, migration: &crate::MigrationDescriptor) -> Result<(), String>;

    /// Release the exclusive migration lock.
    async fn release_lock(&self, namespace: &str) -> Result<(), String>;
}

/// Failures returned by the migration coordinator.
#[derive(Debug, Error)]
pub enum MigrationRunError {
    /// The migration lock could not be acquired.
    #[error("could not acquire migration lock: {0}")]
    LockAcquire(String),
    /// Applied migration history could not be loaded.
    #[error("could not load migration history: {0}")]
    HistoryRead(String),
    /// The current catalog conflicts with applied history.
    #[error(transparent)]
    InvalidPlan(#[from] MigrationError),
    /// One migration failed to execute and record.
    #[error("migration '{migration}' failed: {message}")]
    Apply { migration: String, message: String },
    /// The lock could not be released after successful execution.
    #[error("migration work completed but lock release failed: {0}")]
    LockRelease(String),
    /// Both migration work and lock release failed.
    #[error("migration work failed ({operation_error}); lock release also failed ({release_error})")]
    OperationAndRelease {
        operation_error: String,
        release_error: String,
    },
}

/// Coordinates a forward-only, resumable migration run.
#[derive(Clone, Copy, Debug, Default)]
pub struct MigrationRunner;

impl MigrationRunner {
    /// Acquire a lock, validate history, apply pending migrations, then release the lock.
    ///
    /// Each migration is applied in its own backend-defined unit. If one fails,
    /// earlier successful migrations remain recorded and the next run resumes
    /// from history. This coordinator never assumes that DDL can be rolled back.
    pub async fn run<B: MigrationBackend>(
        &self,
        namespace: &str,
        module_order: &[String],
        catalog: &MigrationCatalog,
        backend: &B,
    ) -> Result<MigrationPlan, MigrationRunError> {
        if namespace.trim().is_empty() {
            return Err(MigrationRunError::LockAcquire(
                "migration namespace must not be empty".into(),
            ));
        }
        backend.acquire_lock(namespace).await.map_err(MigrationRunError::LockAcquire)?;
        let operation = self.run_locked(namespace, module_order, catalog, backend).await;
        let release = backend.release_lock(namespace).await;
        match (operation, release) {
            (Ok(plan), Ok(())) => Ok(plan),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(MigrationRunError::LockRelease(error)),
            (Err(operation_error), Err(release_error)) => {
                Err(MigrationRunError::OperationAndRelease {
                    operation_error: operation_error.to_string(),
                    release_error,
                })
            }
        }
    }

    async fn run_locked<B: MigrationBackend>(
        &self,
        namespace: &str,
        module_order: &[String],
        catalog: &MigrationCatalog,
        backend: &B,
    ) -> Result<MigrationPlan, MigrationRunError> {
        let history = backend.load_history(namespace).await
            .map_err(MigrationRunError::HistoryRead)?;
        let plan = catalog.plan(module_order, &history)?;
        for migration in plan.pending() {
            backend.apply_and_record(migration).await.map_err(|message| {
                MigrationRunError::Apply {
                    migration: migration.key(),
                    message,
                }
            })?;
        }
        Ok(plan)
    }
}

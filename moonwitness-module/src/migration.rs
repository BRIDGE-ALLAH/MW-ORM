use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Immutable identity and content checksum for a module-owned migration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationDescriptor {
    /// Owning module identifier.
    pub module: String,
    /// Stable identifier unique within the owning module.
    pub id: String,
    /// Monotonically increasing migration version within the module.
    pub version: u64,
    /// SHA-256 checksum of the migration source supplied at construction.
    pub checksum: String,
}

impl MigrationDescriptor {
    /// Create a descriptor and calculate its SHA-256 checksum from source bytes.
    pub fn new(
        module: impl Into<String>,
        id: impl Into<String>,
        version: u64,
        source: &[u8],
    ) -> Result<Self, MigrationError> {
        let module = module.into();
        let id = id.into();
        if !valid_identifier(&module) || !valid_identifier(&id) {
            return Err(MigrationError::InvalidIdentifier { module, id });
        }
        if version == 0 {
            return Err(MigrationError::InvalidVersion { module, id });
        }
        let digest = Sha256::digest(source);
        let checksum = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(Self { module, id, version, checksum })
    }

    /// Return the globally namespaced migration key.
    pub fn key(&self) -> String { format!("{}:{}", self.module, self.id) }
}

/// A row recorded by a migration history store after successful application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedMigration {
    /// Module that owns the migration.
    pub module: String,
    /// Stable migration identifier.
    pub id: String,
    /// Checksum recorded when the migration was applied.
    pub checksum: String,
}

/// Errors produced while validating migration catalogs and plans.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum MigrationError {
    /// Module or migration identifier is malformed.
    #[error("invalid migration identifier '{module}:{id}'")]
    InvalidIdentifier { module: String, id: String },
    /// Migration version must start at one or greater.
    #[error("migration '{module}:{id}' must have a version greater than zero")]
    InvalidVersion { module: String, id: String },
    /// A module declares the same migration id or version more than once.
    #[error("duplicate migration id or version in module '{module}': '{id}'")]
    Duplicate { module: String, id: String },
    /// An applied migration no longer exists in the current catalog.
    #[error("applied migration '{module}:{id}' is missing from the current catalog")]
    UnknownApplied { module: String, id: String },
    /// The source of a previously applied migration has changed.
    #[error("checksum mismatch for applied migration '{module}:{id}': recorded {recorded}, current {current}")]
    ChecksumMismatch { module: String, id: String, recorded: String, current: String },
}

/// Validated, deterministically ordered collection of module migrations.
#[derive(Clone, Debug, Default)]
pub struct MigrationCatalog {
    migrations: Vec<MigrationDescriptor>,
}

impl MigrationCatalog {
    /// Validate and sort migration descriptors by module, version, then id.
    pub fn new(mut migrations: Vec<MigrationDescriptor>) -> Result<Self, MigrationError> {
        let mut ids = BTreeSet::new();
        let mut versions = BTreeSet::new();
        for migration in &migrations {
            if !ids.insert((migration.module.clone(), migration.id.clone()))
                || !versions.insert((migration.module.clone(), migration.version))
            {
                return Err(MigrationError::Duplicate {
                    module: migration.module.clone(),
                    id: migration.id.clone(),
                });
            }
        }
        migrations.sort_by(|left, right| {
            (&left.module, left.version, &left.id).cmp(&(&right.module, right.version, &right.id))
        });
        Ok(Self { migrations })
    }

    /// Return every migration in deterministic catalog order.
    pub fn migrations(&self) -> &[MigrationDescriptor] { &self.migrations }

    /// Build a pending plan and reject missing or modified applied migrations.
    ///
    /// Module order should be supplied from the module activation plan. Modules
    /// not present in that list are appended alphabetically for stable output.
    pub fn plan(
        &self,
        module_order: &[String],
        applied: &[AppliedMigration],
    ) -> Result<MigrationPlan, MigrationError> {
        let current: BTreeMap<(&str, &str), &MigrationDescriptor> = self.migrations
            .iter()
            .map(|migration| ((migration.module.as_str(), migration.id.as_str()), migration))
            .collect();
        let mut applied_keys = BTreeSet::new();
        for record in applied {
            let key = (record.module.as_str(), record.id.as_str());
            if !applied_keys.insert((record.module.clone(), record.id.clone())) {
                return Err(MigrationError::Duplicate {
                    module: record.module.clone(),
                    id: record.id.clone(),
                });
            }
            let Some(migration) = current.get(&key) else {
                return Err(MigrationError::UnknownApplied {
                    module: record.module.clone(),
                    id: record.id.clone(),
                });
            };
            if migration.checksum != record.checksum {
                return Err(MigrationError::ChecksumMismatch {
                    module: record.module.clone(),
                    id: record.id.clone(),
                    recorded: record.checksum.clone(),
                    current: migration.checksum.clone(),
                });
            }
        }

        let order: BTreeMap<&str, usize> = module_order.iter().enumerate()
            .map(|(index, name)| (name.as_str(), index))
            .collect();
        let mut pending: Vec<_> = self.migrations.iter()
            .filter(|migration| !applied_keys.contains(&(migration.module.clone(), migration.id.clone())))
            .cloned()
            .collect();
        pending.sort_by(|left, right| {
            let left_order = order.get(left.module.as_str()).copied().unwrap_or(usize::MAX);
            let right_order = order.get(right.module.as_str()).copied().unwrap_or(usize::MAX);
            (left_order, &left.module, left.version, &left.id)
                .cmp(&(right_order, &right.module, right.version, &right.id))
        });
        Ok(MigrationPlan { pending })
    }
}

/// A read-only list of migrations not yet present in the supplied history.
#[derive(Clone, Debug, Default)]
pub struct MigrationPlan {
    pending: Vec<MigrationDescriptor>,
}

impl MigrationPlan {
    /// Return pending migrations in execution order.
    pub fn pending(&self) -> &[MigrationDescriptor] { &self.pending }

    /// Return whether the plan contains no pending migrations.
    pub fn is_empty(&self) -> bool { self.pending.is_empty() }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.as_bytes()[0].is_ascii_lowercase()
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
                })
        })
}

use std::sync::Mutex;

use crate::{
    AppliedMigration, MigrationBackend, MigrationCatalog, MigrationDescriptor,
    MigrationRunError, MigrationRunner,
};

#[derive(Default)]
struct FakeBackend {
    locked: Mutex<bool>,
    history: Mutex<Vec<AppliedMigration>>,
    executed: Mutex<Vec<String>>,
    namespaces: Mutex<Vec<String>>,
    fail_apply: bool,
    fail_history: bool,
    fail_release: bool,
}

#[async_trait::async_trait]
impl MigrationBackend for FakeBackend {
    async fn acquire_lock(&self, _namespace: &str) -> Result<(), String> {
        let mut locked = self.locked.lock().unwrap();
        if *locked { return Err("already locked".into()); }
        *locked = true;
        Ok(())
    }

    async fn load_history(&self, namespace: &str) -> Result<Vec<AppliedMigration>, String> {
        self.namespaces.lock().unwrap().push(format!("history:{namespace}"));
        if self.fail_history {
            return Err("simulated history read failure".into());
        }
        Ok(self.history.lock().unwrap().clone())
    }

    async fn apply_and_record(
        &self,
        namespace: &str,
        migration: &MigrationDescriptor,
    ) -> Result<(), String> {
        self.namespaces.lock().unwrap().push(format!("apply:{namespace}"));
        if self.fail_apply { return Err("simulated DDL failure".into()); }
        self.executed.lock().unwrap().push(migration.key());
        self.history.lock().unwrap().push(AppliedMigration {
            module: migration.module.clone(),
            id: migration.id.clone(),
            version: migration.version,
            checksum: migration.checksum.clone(),
        });
        Ok(())
    }

    async fn release_lock(&self, namespace: &str) -> Result<(), String> {
        self.namespaces.lock().unwrap().push(format!("release:{namespace}"));
        *self.locked.lock().unwrap() = false;
        if self.fail_release {
            return Err("simulated lock release failure".into());
        }
        Ok(())
    }
}

#[tokio::test]
async fn runner_applies_in_plan_order_and_releases_lock() {
    let catalog = MigrationCatalog::new(vec![
        MigrationDescriptor::new("mw.audit", "log", 1, b"audit").unwrap(),
        MigrationDescriptor::new("mw.core", "base", 1, b"core").unwrap(),
    ]).unwrap();
    let backend = FakeBackend::default();
    let order = vec!["mw.core".into(), "mw.audit".into()];
    let plan = MigrationRunner.run("application", &order, &catalog, &backend).await.unwrap();

    assert_eq!(plan.pending().len(), 2);
    assert_eq!(*backend.executed.lock().unwrap(), vec!["mw.core:base", "mw.audit:log"]);
    assert!(!*backend.locked.lock().unwrap());
    assert_eq!(backend.history.lock().unwrap().len(), 2);
    assert_eq!(
        *backend.namespaces.lock().unwrap(),
        vec![
            "history:application".to_owned(),
            "apply:application".to_owned(),
            "apply:application".to_owned(),
            "release:application".to_owned(),
        ]
    );
}

#[tokio::test]
async fn runner_releases_lock_after_migration_failure() {
    let catalog = MigrationCatalog::new(vec![
        MigrationDescriptor::new("mw.core", "base", 1, b"core").unwrap(),
    ]).unwrap();
    let backend = FakeBackend { fail_apply: true, ..FakeBackend::default() };
    let error = MigrationRunner.run("application", &[], &catalog, &backend).await.unwrap_err();

    assert!(matches!(error, MigrationRunError::Apply { .. }));
    assert!(!*backend.locked.lock().unwrap());
    assert!(backend.history.lock().unwrap().is_empty());
}

#[tokio::test]
async fn runner_releases_lock_after_history_read_failure() {
    let catalog = MigrationCatalog::default();
    let backend = FakeBackend {
        fail_history: true,
        ..FakeBackend::default()
    };

    let error = MigrationRunner
        .run("application", &[], &catalog, &backend)
        .await
        .unwrap_err();

    assert!(matches!(error, MigrationRunError::HistoryRead(_)));
    assert!(!*backend.locked.lock().unwrap());
}

#[tokio::test]
async fn runner_reports_lock_release_failure_after_successful_migration_work() {
    let catalog = MigrationCatalog::default();
    let backend = FakeBackend {
        fail_release: true,
        ..FakeBackend::default()
    };

    let error = MigrationRunner
        .run("application", &[], &catalog, &backend)
        .await
        .unwrap_err();

    assert!(matches!(error, MigrationRunError::LockRelease(_)));
    assert!(!*backend.locked.lock().unwrap());
}

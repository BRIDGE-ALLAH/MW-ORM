//! Reference audit add-on built on MoonWitness typed module contributions.
//!
//! This add-on records events published by the application in an in-memory
//! store. Applications must publish `RecordChanged` only after their own
//! transaction commits. This crate does not intercept ORM writes or provide
//! durable audit storage.

use std::sync::{Arc, Mutex};

use moonwitness_module::{
    LifecycleError, Module, ModuleManifest, ExtensionRegistryBuilder,
};
use semver::Version;

/// A committed record-change event published by an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordChanged {
    /// Logical entity or table name.
    pub entity: String,
    /// Stable application-level record key.
    pub record_key: String,
    /// Operation such as `insert`, `update`, or `delete`.
    pub operation: String,
    /// Optional actor identifier supplied by the application.
    pub actor: Option<String>,
}

/// One in-memory audit entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    /// Logical entity or table name.
    pub entity: String,
    /// Stable application-level record key.
    pub record_key: String,
    /// Operation that was committed.
    pub operation: String,
    /// Optional actor identifier.
    pub actor: Option<String>,
}

/// Thread-safe in-memory store shared by the add-on and its subscribers.
#[derive(Clone, Default)]
pub struct AuditStore {
    records: Arc<Mutex<Vec<AuditRecord>>>,
}

impl AuditStore {
    /// Return a snapshot of recorded audit entries.
    pub fn records(&self) -> Result<Vec<AuditRecord>, LifecycleError> {
        self.records.lock()
            .map(|records| records.clone())
            .map_err(|_| LifecycleError::new("audit store lock is poisoned"))
    }

    fn record(&self, event: &RecordChanged) -> Result<(), LifecycleError> {
        let mut records = self.records.lock()
            .map_err(|_| LifecycleError::new("audit store lock is poisoned"))?;
        records.push(AuditRecord {
            entity: event.entity.clone(),
            record_key: event.record_key.clone(),
            operation: event.operation.clone(),
            actor: event.actor.clone(),
        });
        Ok(())
    }
}

/// Reference audit module that subscribes to committed record-change events.
pub struct AuditModule {
    manifest: ModuleManifest,
    store: AuditStore,
}

impl Default for AuditModule {
    fn default() -> Self { Self::new() }
}

impl AuditModule {
    /// Create the audit add-on and its empty in-memory store.
    pub fn new() -> Self {
        Self {
            manifest: ModuleManifest {
                name: "mw.audit".into(),
                version: Version::new(0, 1, 0),
                api_version: Version::new(1, 0, 0),
                dependencies: Vec::new(),
                provides: vec!["audit.record-change".into()],
                requires_capabilities: Vec::new(),
            },
            store: AuditStore::default(),
        }
    }
}

impl Module for AuditModule {
    fn manifest(&self) -> &ModuleManifest { &self.manifest }

    fn contribute(&self, extensions: &mut ExtensionRegistryBuilder) -> Result<(), LifecycleError> {
        extensions.register_service(self.store.clone())?;
        let store = self.store.clone();
        extensions.subscribe_event::<RecordChanged, _>("mw.audit", "record-change", move |event| {
            store.record(event)
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use moonwitness_module::ModuleRegistryBuilder;

    use super::{AuditModule, RecordChanged, AuditStore};

    #[test]
    fn audit_registry_accepts_record_change_events() {
        let mut builder = ModuleRegistryBuilder::new();
        builder.register(AuditModule::new()).unwrap();
        let registry = builder.build().unwrap();
        let store = registry.extensions().service::<AuditStore>().unwrap();
        registry.extensions().publish_event(&RecordChanged {
            entity: "users".into(),
            record_key: "user-42".into(),
            operation: "update".into(),
            actor: Some("operator-7".into()),
        }).unwrap();
        let records = store.records().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_key, "user-42");
    }
}

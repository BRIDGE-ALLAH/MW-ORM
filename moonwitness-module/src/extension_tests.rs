use std::sync::{Arc, Mutex};

use crate::{
    AppliedMigration, EventBus, HookContext, HookRegistry, MigrationCatalog,
    MigrationDescriptor, MigrationError, ModuleContext, ServiceRegistryBuilder,
};

#[test]
fn hooks_run_in_priority_then_identity_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut hooks = HookRegistry::new();
    for (module, id, priority) in [("mw.z", "late", 10), ("mw.a", "second", 0), ("mw.a", "first", 0)] {
        let calls = Arc::clone(&calls);
        hooks.register("entity.before_save", module, id, priority, move |_| {
            calls.lock().unwrap().push(format!("{module}:{id}"));
            Ok(())
        }).unwrap();
    }
    let count = hooks.dispatch("entity.before_save", &mut HookContext::new()).unwrap();
    assert_eq!(count, 3);
    assert_eq!(*calls.lock().unwrap(), vec!["mw.a:first", "mw.a:second", "mw.z:late"]);
}

#[test]
fn hook_dispatch_stops_on_first_error() {
    let mut hooks = HookRegistry::new();
    hooks.register("x", "mw.a", "fail", 0, |_| Err(crate::LifecycleError::new("stop"))).unwrap();
    assert!(hooks.dispatch("x", &mut HookContext::new()).is_err());
}

#[test]
fn events_are_typed_and_deterministically_dispatched() {
    #[derive(Debug)]
    struct Saved { id: u64 }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut events = EventBus::new();
    for module in ["mw.z", "mw.a"] {
        let calls = Arc::clone(&calls);
        events.subscribe::<Saved, _>(module, "handler", move |event| {
            calls.lock().unwrap().push(format!("{module}:{}", event.id));
            Ok(())
        }).unwrap();
    }
    assert_eq!(events.publish(&Saved { id: 42 }).unwrap(), 2);
    assert_eq!(*calls.lock().unwrap(), vec!["mw.a:42", "mw.z:42"]);
}

#[test]
fn services_resolve_by_concrete_type() {
    let mut builder = ServiceRegistryBuilder::new();
    builder.register(String::from("connection-pool")).unwrap();
    let services = builder.build();
    assert_eq!(services.get::<String>().as_deref().map(String::as_str), Some("connection-pool"));
    assert!(services.get::<u64>().is_none());
}

#[test]
fn migration_plan_is_namespaced_ordered_and_checksum_checked() {
    let core = MigrationDescriptor::new("mw.core", "create_base", 1, b"CREATE TABLE base").unwrap();
    let audit = MigrationDescriptor::new("mw.audit", "create_log", 1, b"CREATE TABLE audit").unwrap();
    let catalog = MigrationCatalog::new(vec![audit.clone(), core.clone()]).unwrap();
    let plan = catalog.plan(&["mw.core".into(), "mw.audit".into()], &[]).unwrap();
    assert_eq!(plan.pending().iter().map(|m| m.module.as_str()).collect::<Vec<_>>(),
        vec!["mw.core", "mw.audit"]);
    let applied = [AppliedMigration { module: "mw.core".into(), id: "create_base".into(), checksum: core.checksum.clone() }];
    let pending = catalog.plan(&["mw.core".into(), "mw.audit".into()], &applied).unwrap();
    assert_eq!(pending.pending(), &[audit]);
}

#[test]
fn migration_catalog_rejects_checksum_drift_and_unknown_history() {
    let migration = MigrationDescriptor::new("mw.core", "base", 1, b"v1").unwrap();
    let catalog = MigrationCatalog::new(vec![migration]).unwrap();
    let changed = AppliedMigration { module: "mw.core".into(), id: "base".into(), checksum: "0".repeat(64) };
    assert!(matches!(catalog.plan(&[], &[changed]), Err(MigrationError::ChecksumMismatch { .. })));
    let missing = AppliedMigration { module: "mw.legacy".into(), id: "old".into(), checksum: "0".repeat(64) };
    assert!(matches!(catalog.plan(&[], &[missing]), Err(MigrationError::UnknownApplied { .. })));
}

#[test]
fn module_context_exposes_registered_services() {
    let mut builder = ServiceRegistryBuilder::new();
    builder.register(42_u64).unwrap();
    let context = ModuleContext::with_services(
        std::collections::BTreeMap::new(),
        builder.build(),
    );
    assert_eq!(*context.service::<u64>().unwrap(), 42);
}

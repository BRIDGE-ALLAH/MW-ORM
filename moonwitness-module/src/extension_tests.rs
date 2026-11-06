use std::{collections::BTreeMap, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};

use semver::Version;

use crate::{
    AppliedMigration, EventBus, HookContext, HookRegistry, MigrationCatalog,
    ExtensionRegistryBuilder, KernelError, LifecycleError, MigrationDescriptor, MigrationError, Module,
    ModuleContext, ModuleHost, ModuleManifest, ModuleRegistryBuilder, ServiceRegistryBuilder,
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
    let applied = [AppliedMigration { module: "mw.core".into(), id: "create_base".into(), version: 1, checksum: core.checksum.clone() }];
    let pending = catalog.plan(&["mw.core".into(), "mw.audit".into()], &applied).unwrap();
    assert_eq!(pending.pending(), &[audit]);
}

#[test]
fn migration_catalog_rejects_checksum_drift_and_unknown_history() {
    let migration = MigrationDescriptor::new("mw.core", "base", 1, b"v1").unwrap();
    let catalog = MigrationCatalog::new(vec![migration.clone()]).unwrap();
    assert_eq!(migration.source(), b"v1");
    let wrong_version = AppliedMigration { module: "mw.core".into(), id: "base".into(), version: 2, checksum: migration.checksum.clone() };
    assert!(matches!(catalog.plan(&[], &[wrong_version]), Err(MigrationError::VersionMismatch { .. })));
    let changed = AppliedMigration { module: "mw.core".into(), id: "base".into(), version: 1, checksum: "0".repeat(64) };
    assert!(matches!(catalog.plan(&[], &[changed]), Err(MigrationError::ChecksumMismatch { .. })));
    let missing = AppliedMigration { module: "mw.legacy".into(), id: "old".into(), version: 1, checksum: "0".repeat(64) };
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

struct ContributingModule {
    manifest: ModuleManifest,
    prepared: Arc<AtomicBool>,
    events_seen: Arc<AtomicUsize>,
}

impl Module for ContributingModule {
    fn manifest(&self) -> &ModuleManifest { &self.manifest }

    fn contribute(&self, extensions: &mut ExtensionRegistryBuilder) -> Result<(), LifecycleError> {
        extensions.register_service(String::from("registered-before-start"))?;
        let migration = MigrationDescriptor::new("mw.contrib", "create_audit", 1, b"CREATE TABLE audit")
            .map_err(|error| LifecycleError::new(error.to_string()))?;
        extensions.register_migration(migration)?;
        extensions.register_hook("test.before", "mw.contrib", "mark", 0, |context| {
            context.insert("hook", "ran");
            Ok(())
        })?;
        let events_seen = Arc::clone(&self.events_seen);
        extensions.subscribe_event::<u32, _>("mw.contrib", "count", move |value| {
            if *value == 7 {
                events_seen.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        })?;
        Ok(())
    }

    fn prepare(&self, context: &ModuleContext) -> Result<(), LifecycleError> {
        if context.service::<String>().as_deref().map(String::as_str) != Some("registered-before-start") {
            return Err(LifecycleError::new("contributed service is missing"));
        }
        let mut hook_context = HookContext::new();
        context.dispatch_hook("test.before", &mut hook_context)?;
        if hook_context.get("hook") != Some("ran") {
            return Err(LifecycleError::new("contributed hook did not run"));
        }
        context.publish_event(&7_u32)?;
        self.prepared.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn modules_contribute_services_hooks_and_events_before_prepare() {
    let prepared = Arc::new(AtomicBool::new(false));
    let events_seen = Arc::new(AtomicUsize::new(0));
    let module = ContributingModule {
        manifest: ModuleManifest {
            name: "mw.contrib".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
        prepared: Arc::clone(&prepared),
        events_seen: Arc::clone(&events_seen),
    };
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(module).unwrap();
    let registry = builder.build().unwrap();
    assert_eq!(registry.extensions().migrations().migrations().len(), 1);
    let mut host = ModuleHost::new(registry, ModuleContext::new(BTreeMap::new()));
    host.start().unwrap();

    assert!(prepared.load(Ordering::SeqCst));
    assert_eq!(events_seen.load(Ordering::SeqCst), 1);
    host.stop().unwrap();
}

struct ForeignMigrationModule {
    manifest: ModuleManifest,
}

impl Module for ForeignMigrationModule {
    fn manifest(&self) -> &ModuleManifest { &self.manifest }

    fn contribute(&self, extensions: &mut ExtensionRegistryBuilder) -> Result<(), LifecycleError> {
        let migration = MigrationDescriptor::new("mw.other", "foreign", 1, b"foreign")
            .map_err(|error| LifecycleError::new(error.to_string()))?;
        extensions.register_migration(migration)?;
        Ok(())
    }
}

#[test]
fn module_cannot_register_migration_owned_by_another_module() {
    let module = ForeignMigrationModule {
        manifest: ModuleManifest {
            name: "mw.owner".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
    };
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(module).unwrap();
    assert!(matches!(builder.build(), Err(KernelError::Lifecycle(_))));
}

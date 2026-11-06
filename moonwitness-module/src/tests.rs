use std::{collections::BTreeMap, sync::{Arc, Mutex}};

use semver::{Version, VersionReq};

use crate::{
    KernelError, LifecycleError, Module, ModuleContext, ModuleDependency, ModuleHost,
    ModuleManifest, ModuleRegistryBuilder,
};

fn manifest(name: &str, version: &str) -> ModuleManifest {
    ModuleManifest {
        name: name.into(),
        version: Version::parse(version).expect("valid test version"),
        api_version: Version::new(1, 0, 0),
        dependencies: Vec::new(),
        provides: Vec::new(),
        requires_capabilities: Vec::new(),
    }
}

#[test]
fn toml_manifest_is_parsed_and_validated() {
    let parsed = ModuleManifest::from_toml(r#"
name = "mw.audit"
version = "0.2.0"
api_version = "1.0.0"
provides = ["audit.events"]

[[dependencies]]
name = "mw.core"
version = "^1.0"
optional = false
"#).expect("manifest should parse");

    assert_eq!(parsed.name, "mw.audit");
    assert_eq!(parsed.dependencies[0].name, "mw.core");
}

#[test]
fn resolver_orders_dependencies_before_dependents() {
    let core = manifest("mw.core", "1.2.0");
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.dependencies.push(ModuleDependency {
        name: "mw.core".into(),
        version: VersionReq::parse("^1.0").expect("valid requirement"),
        optional: false,
    });

    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(core)).expect("register core");
    builder.register(TestModule::new(audit)).expect("register audit");
    let registry = builder.build().expect("registry should resolve");

    assert_eq!(
        registry.activation_plan().ordered_modules(),
        &["mw.core".to_owned(), "mw.audit".to_owned()]
    );
}

#[test]
fn resolver_rejects_missing_required_dependency() {
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.dependencies.push(ModuleDependency {
        name: "mw.core".into(),
        version: VersionReq::STAR,
        optional: false,
    });
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(audit)).expect("register audit");

    assert!(matches!(
        builder.build(),
        Err(KernelError::MissingDependency { .. })
    ));
}

#[test]
fn resolver_rejects_incompatible_dependency() {
    let core = manifest("mw.core", "2.0.0");
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.dependencies.push(ModuleDependency {
        name: "mw.core".into(),
        version: VersionReq::parse("^1.0").expect("valid requirement"),
        optional: false,
    });
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(core)).expect("register core");
    builder.register(TestModule::new(audit)).expect("register audit");

    assert!(matches!(
        builder.build(),
        Err(KernelError::IncompatibleDependency { .. })
    ));
}

#[test]
fn resolver_rejects_cycles() {
    let mut alpha = manifest("mw.alpha", "1.0.0");
    let mut beta = manifest("mw.beta", "1.0.0");
    alpha.dependencies.push(ModuleDependency {
        name: "mw.beta".into(),
        version: VersionReq::STAR,
        optional: false,
    });
    beta.dependencies.push(ModuleDependency {
        name: "mw.alpha".into(),
        version: VersionReq::STAR,
        optional: false,
    });
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(alpha)).expect("register alpha");
    builder.register(TestModule::new(beta)).expect("register beta");

    assert!(matches!(
        builder.build(),
        Err(KernelError::DependencyCycle(_))
    ));
}

#[test]
fn optional_missing_dependency_does_not_block_resolution() {
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.dependencies.push(ModuleDependency {
        name: "mw.metrics".into(),
        version: VersionReq::STAR,
        optional: true,
    });
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(audit)).expect("register audit");
    assert!(builder.build().is_ok());
}

#[test]
fn host_activates_in_order_and_stops_in_reverse() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut core = TestModule::new(manifest("mw.core", "1.0.0"));
    core.calls = Arc::clone(&calls);
    let mut audit_manifest = manifest("mw.audit", "1.0.0");
    audit_manifest.dependencies.push(ModuleDependency {
        name: "mw.core".into(),
        version: VersionReq::STAR,
        optional: false,
    });
    let mut audit = TestModule::new(audit_manifest);
    audit.calls = Arc::clone(&calls);

    let mut builder = ModuleRegistryBuilder::new();
    builder.register(core).expect("register core");
    builder.register(audit).expect("register audit");
    let registry = builder.build().expect("build registry");
    let mut host = ModuleHost::new(registry, ModuleContext::new(BTreeMap::new()));

    host.start().expect("start host");
    host.stop().expect("stop host");

    assert_eq!(
        *calls.lock().expect("test mutex"),
        vec!["prepare:mw.core", "prepare:mw.audit", "activate:mw.core", "activate:mw.audit", "deactivate:mw.audit", "deactivate:mw.core"]
            .into_iter().map(String::from).collect::<Vec<_>>()
    );
}

#[test]
fn host_rolls_back_activated_modules_when_activation_fails() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut core = TestModule::new(manifest("mw.core", "1.0.0"));
    core.calls = Arc::clone(&calls);
    let mut failing = TestModule::new(manifest("mw.fail", "1.0.0"));
    failing.calls = Arc::clone(&calls);
    failing.fail_activation = true;

    let mut builder = ModuleRegistryBuilder::new();
    builder.register(core).expect("register core");
    builder.register(failing).expect("register failing module");
    let registry = builder.build().expect("build registry");
    let mut host = ModuleHost::new(registry, ModuleContext::default());

    assert!(host.start().is_err());
    assert!(host.active_modules().is_empty());
    assert_eq!(
        *calls.lock().expect("test mutex"),
        vec!["prepare:mw.core", "prepare:mw.fail", "activate:mw.core", "activate:mw.fail", "deactivate:mw.core"]
            .into_iter().map(String::from).collect::<Vec<_>>()
    );
}

#[test]
fn resolver_orders_required_capability_provider_first() {
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.requires_capabilities.push("audit.events".into());
    let mut events = manifest("mw.events", "1.0.0");
    events.provides.push("audit.events".into());

    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(audit)).expect("register audit");
    builder.register(TestModule::new(events)).expect("register events");
    let registry = builder.build().expect("capability should resolve");

    assert_eq!(
        registry.activation_plan().ordered_modules(),
        &["mw.events".to_owned(), "mw.audit".to_owned()]
    );
}

#[test]
fn resolver_rejects_missing_capability() {
    let mut audit = manifest("mw.audit", "0.1.0");
    audit.requires_capabilities.push("audit.events".into());
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(audit)).expect("register audit");

    assert!(matches!(
        builder.build(),
        Err(KernelError::MissingCapability { .. })
    ));
}

#[test]
fn registry_rejects_duplicate_module_identifiers() {
    let mut builder = ModuleRegistryBuilder::new();
    builder.register(TestModule::new(manifest("mw.audit", "0.1.0"))).expect("first registration");
    assert!(matches!(
        builder.register(TestModule::new(manifest("mw.audit", "0.2.0"))),
        Err(KernelError::DuplicateModule(_))
    ));
}

struct TestModule {
    manifest: ModuleManifest,
    calls: Arc<Mutex<Vec<String>>>,
    fail_activation: bool,
}

impl TestModule {
    fn new(manifest: ModuleManifest) -> Self {
        Self {
            manifest,
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_activation: false,
        }
    }

    fn record(&self, action: &str) {
        self.calls.lock().expect("test mutex").push(format!("{action}:{}", self.manifest.name));
    }
}

impl Module for TestModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        self.record("prepare");
        Ok(())
    }

    fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        self.record("activate");
        if self.fail_activation {
            Err(LifecycleError::new("intentional test failure"))
        } else {
            Ok(())
        }
    }

    fn deactivate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        self.record("deactivate");
        Ok(())
    }
}

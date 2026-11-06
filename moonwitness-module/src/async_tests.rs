use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use semver::{Version, VersionReq};

use crate::{
    AsyncModule, AsyncModuleHost, AsyncModuleRegistryBuilder, LifecycleError,
    ModuleContext, ModuleDependency, ModuleManifest,
};

struct AsyncTestModule {
    manifest: ModuleManifest,
}

impl AsyncTestModule {
    fn new(name: &str) -> Self {
        Self {
            manifest: ModuleManifest {
                name: name.into(),
                version: Version::new(1, 0, 0),
                api_version: Version::new(1, 0, 0),
                dependencies: Vec::new(),
                provides: Vec::new(),
                requires_capabilities: Vec::new(),
            },
        }
    }
}

#[async_trait::async_trait]
impl AsyncModule for AsyncTestModule {
    fn manifest(&self) -> &ModuleManifest { &self.manifest }

    async fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> { Ok(()) }
    async fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> { Ok(()) }
    async fn deactivate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> { Ok(()) }
}

#[tokio::test]
async fn async_host_starts_and_stops_modules() {
    let mut builder = AsyncModuleRegistryBuilder::new();
    builder.register(AsyncTestModule::new("mw.async")).unwrap();
    let registry = builder.build().unwrap();
    let mut host = AsyncModuleHost::new(registry, ModuleContext::default());

    host.start().await.unwrap();
    assert_eq!(host.active_modules(), &["mw.async"]);
    host.stop().await.unwrap();
    assert!(host.active_modules().is_empty());
}

struct RetryRollbackModule {
    manifest: ModuleManifest,
    deactivation_attempts: Arc<AtomicUsize>,
    fail_activation: bool,
}

#[async_trait::async_trait]
impl AsyncModule for RetryRollbackModule {
    fn manifest(&self) -> &ModuleManifest { &self.manifest }

    async fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        if self.fail_activation {
            Err(LifecycleError::new("activation failed"))
        } else {
            Ok(())
        }
    }

    async fn deactivate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        if self.fail_activation {
            return Ok(());
        }
        let attempt = self.deactivation_attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            Err(LifecycleError::new("transient rollback failure"))
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn async_host_retains_failed_rollback_for_shutdown_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let core = RetryRollbackModule {
        manifest: ModuleManifest {
            name: "mw.core".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
        deactivation_attempts: Arc::clone(&attempts),
        fail_activation: false,
    };
    let failing = RetryRollbackModule {
        manifest: ModuleManifest {
            name: "mw.fail".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
        deactivation_attempts: Arc::new(AtomicUsize::new(0)),
        fail_activation: true,
    };

    let mut builder = AsyncModuleRegistryBuilder::new();
    builder.register(core).unwrap();
    builder.register(failing).unwrap();
    let registry = builder.build().unwrap();
    let mut host = AsyncModuleHost::new(registry, ModuleContext::default());

    assert!(host.start().await.is_err());
    assert_eq!(host.active_modules(), &["mw.core"]);
    host.stop().await.unwrap();
    assert!(host.active_modules().is_empty());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn async_shutdown_keeps_dependencies_active_until_dependents_stop() {
    let core_attempts = Arc::new(AtomicUsize::new(0));
    let audit_attempts = Arc::new(AtomicUsize::new(0));
    let core = RetryRollbackModule {
        manifest: ModuleManifest {
            name: "mw.core".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
        deactivation_attempts: Arc::clone(&core_attempts),
        fail_activation: false,
    };
    let audit = RetryRollbackModule {
        manifest: ModuleManifest {
            name: "mw.audit".into(),
            version: Version::new(1, 0, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: vec![ModuleDependency {
                name: "mw.core".into(),
                version: VersionReq::STAR,
                optional: false,
            }],
            provides: Vec::new(),
            requires_capabilities: Vec::new(),
        },
        deactivation_attempts: Arc::clone(&audit_attempts),
        fail_activation: false,
    };

    let mut builder = AsyncModuleRegistryBuilder::new();
    builder.register(core).unwrap();
    builder.register(audit).unwrap();
    let registry = builder.build().unwrap();
    let mut host = AsyncModuleHost::new(registry, ModuleContext::default());
    host.start().await.unwrap();

    assert!(host.stop().await.is_err());
    assert_eq!(core_attempts.load(Ordering::SeqCst), 0);
    assert_eq!(host.active_modules(), &["mw.core", "mw.audit"]);

    assert!(host.stop().await.is_err());
    assert_eq!(host.active_modules(), &["mw.core"]);
    host.stop().await.unwrap();
    assert!(host.active_modules().is_empty());
    assert_eq!(audit_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(core_attempts.load(Ordering::SeqCst), 2);
}

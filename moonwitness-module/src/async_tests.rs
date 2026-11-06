use semver::Version;

use crate::{
    AsyncModule, AsyncModuleHost, AsyncModuleRegistryBuilder, LifecycleError,
    ModuleContext, ModuleManifest,
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

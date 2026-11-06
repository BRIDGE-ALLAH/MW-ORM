use std::collections::BTreeMap;

use crate::{
    ActivationPlan, ExtensionRegistry, ExtensionRegistryBuilder, KernelError,
    LifecycleError, Module, ModuleManifest, resolver,
};

/// A validated collection of statically linked modules and their contributions.
pub struct ModuleRegistry {
    pub(crate) modules: BTreeMap<String, Box<dyn Module>>,
    manifests: BTreeMap<String, ModuleManifest>,
    plan: ActivationPlan,
    pub(crate) extensions: ExtensionRegistry,
}

impl ModuleRegistry {
    /// Return the validated activation plan.
    pub fn activation_plan(&self) -> &ActivationPlan { &self.plan }

    /// Return a registered module manifest by identifier.
    pub fn manifest(&self, name: &str) -> Option<&ModuleManifest> { self.manifests.get(name) }

    /// Return the immutable contributions registered by all modules.
    pub fn extensions(&self) -> &ExtensionRegistry { &self.extensions }

    /// Return the number of registered modules.
    pub fn len(&self) -> usize { self.modules.len() }

    /// Return whether the registry contains no modules.
    pub fn is_empty(&self) -> bool { self.modules.is_empty() }
}

/// Builder for a module registry.
#[derive(Default)]
pub struct ModuleRegistryBuilder {
    modules: BTreeMap<String, Box<dyn Module>>,
}

impl ModuleRegistryBuilder {
    /// Create an empty registry builder.
    pub fn new() -> Self { Self::default() }

    /// Register a statically linked module.
    pub fn register<M: Module + 'static>(&mut self, module: M) -> Result<&mut Self, KernelError> {
        let manifest = module.manifest().clone();
        manifest.validate()?;
        let name = manifest.name.clone();
        if self.modules.contains_key(&name) {
            return Err(KernelError::DuplicateModule(name));
        }
        self.modules.insert(name, Box::new(module));
        Ok(self)
    }

    /// Validate modules, collect their contributions, and freeze the registry.
    pub fn build(self) -> Result<ModuleRegistry, KernelError> {
        let manifests: BTreeMap<String, ModuleManifest> = self.modules
            .iter()
            .map(|(name, module)| (name.clone(), module.manifest().clone()))
            .collect();
        let plan = resolver::resolve(&manifests)?;
        let mut contributions = ExtensionRegistryBuilder::new();
        for name in plan.ordered_modules() {
            contributions.begin_module(name);
            self.modules.get(name)
                .expect("registry plan only contains registered modules")
                .contribute(&mut contributions)
                .map_err(|error: LifecycleError| KernelError::Lifecycle(error))?;
            contributions.end_module();
        }
        Ok(ModuleRegistry {
            modules: self.modules,
            manifests,
            plan,
            extensions: contributions.build().map_err(KernelError::Lifecycle)?,
        })
    }
}

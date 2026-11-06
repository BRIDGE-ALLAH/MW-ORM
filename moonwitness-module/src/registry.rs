use std::collections::BTreeMap;

use crate::{ActivationPlan, KernelError, Module, ModuleManifest, resolver};

/// A validated collection of statically linked modules.
pub struct ModuleRegistry {
    pub(crate) modules: BTreeMap<String, Box<dyn Module>>,
    manifests: BTreeMap<String, ModuleManifest>,
    plan: ActivationPlan,
}

impl ModuleRegistry {
    /// Return the validated activation plan.
    pub fn activation_plan(&self) -> &ActivationPlan {
        &self.plan
    }

    /// Return a registered module manifest by identifier.
    pub fn manifest(&self, name: &str) -> Option<&ModuleManifest> {
        self.manifests.get(name)
    }

    /// Return the number of registered modules.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Return whether the registry contains no modules.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

/// Builder for a module registry.
#[derive(Default)]
pub struct ModuleRegistryBuilder {
    modules: BTreeMap<String, Box<dyn Module>>,
}

impl ModuleRegistryBuilder {
    /// Create an empty registry builder.
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Validate all module contracts and freeze the registry.
    pub fn build(self) -> Result<ModuleRegistry, KernelError> {
        let manifests: BTreeMap<String, ModuleManifest> = self.modules
            .iter()
            .map(|(name, module)| (name.clone(), module.manifest().clone()))
            .collect();
        let plan = resolver::resolve(&manifests)?;
        Ok(ModuleRegistry { modules: self.modules, manifests, plan })
    }
}

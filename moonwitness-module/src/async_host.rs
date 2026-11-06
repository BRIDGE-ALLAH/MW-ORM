use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::{
    ActivationPlan, ExtensionRegistry, ExtensionRegistryBuilder, KernelError, LifecycleError,
    ModuleContext, ModuleManifest, resolver,
};

/// An asynchronously initialized, statically linked module.
#[async_trait]
pub trait AsyncModule: Send + Sync {
    /// Return the module's immutable descriptor.
    fn manifest(&self) -> &ModuleManifest;

    /// Register services, hooks, and event subscribers before the registry is frozen.
    fn contribute(&self, _extensions: &mut ExtensionRegistryBuilder) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Validate readiness without performing irreversible side effects.
    async fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Activate the module after all modules have prepared successfully.
    async fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Deactivate a previously activated module.
    async fn deactivate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }
}

/// Validated registry of asynchronously managed static modules.
pub struct AsyncModuleRegistry {
    modules: BTreeMap<String, Box<dyn AsyncModule>>,
    plan: ActivationPlan,
    pub(crate) extensions: ExtensionRegistry,
}

impl AsyncModuleRegistry {
    /// Return the deterministic dependency-first activation plan.
    pub fn activation_plan(&self) -> &ActivationPlan { &self.plan }

    /// Return the immutable contributions registered by all modules.
    pub fn extensions(&self) -> &ExtensionRegistry { &self.extensions }

    /// Return the number of registered modules.
    pub fn len(&self) -> usize { self.modules.len() }

    /// Return whether no modules are registered.
    pub fn is_empty(&self) -> bool { self.modules.is_empty() }
}

/// Builder for an asynchronous module registry.
#[derive(Default)]
pub struct AsyncModuleRegistryBuilder {
    modules: BTreeMap<String, Box<dyn AsyncModule>>,
}

impl AsyncModuleRegistryBuilder {
    /// Create an empty asynchronous registry builder.
    pub fn new() -> Self { Self::default() }

    /// Register a statically linked asynchronous module.
    pub fn register<M: AsyncModule + 'static>(
        &mut self,
        module: M,
    ) -> Result<&mut Self, KernelError> {
        let manifest = module.manifest().clone();
        manifest.validate()?;
        let name = manifest.name.clone();
        if self.modules.contains_key(&name) {
            return Err(KernelError::DuplicateModule(name));
        }
        self.modules.insert(name, Box::new(module));
        Ok(self)
    }

    /// Validate manifests, collect contributions, and freeze the registry.
    pub fn build(self) -> Result<AsyncModuleRegistry, KernelError> {
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
                .contribute(&mut contributions)?;
            contributions.end_module();
        }
        Ok(AsyncModuleRegistry {
            modules: self.modules,
            plan,
            extensions: contributions.build().map_err(KernelError::Lifecycle)?,
        })
    }
}

/// Asynchronous lifecycle host for a validated module registry.
pub struct AsyncModuleHost {
    registry: AsyncModuleRegistry,
    context: ModuleContext,
    active: Vec<String>,
}

impl AsyncModuleHost {
    /// Create a host and attach the registry's immutable contributions to its context.
    pub fn new(registry: AsyncModuleRegistry, context: ModuleContext) -> Self {
        let mut context = context;
        context.extensions = registry.extensions.clone();
        Self { registry, context, active: Vec::new() }
    }

    /// Return modules currently considered active by the host.
    pub fn active_modules(&self) -> &[String] { &self.active }

    /// Prepare all modules, then activate them in dependency order.
    ///
    /// If activation fails, previously activated modules are deactivated in
    /// reverse order. Modules whose rollback fails remain marked active.
    pub async fn start(&mut self) -> Result<(), LifecycleError> {
        if !self.active.is_empty() {
            return Err(LifecycleError::new(
                "async module host is already started or requires shutdown retry",
            ));
        }
        let plan = self.registry.activation_plan().ordered_modules().to_vec();
        for name in &plan {
            let module = self.registry.modules.get(name)
                .expect("registry plan only contains registered modules");
            module.prepare(&self.context).await
                .map_err(|error| {
                    LifecycleError::new(format!("module '{name}' failed to prepare: {error}"))
                })?;
        }
        for name in plan {
            let module = self.registry.modules.get(&name)
                .expect("registry plan only contains registered modules");
            if let Err(error) = module.activate(&self.context).await {
                let mut failed_modules = Vec::new();
                let mut failures = Vec::new();
                for active_name in self.active.iter().rev() {
                    if let Some(active_module) = self.registry.modules.get(active_name) {
                        if let Err(rollback_error) = active_module.deactivate(&self.context).await {
                            failed_modules.push(active_name.clone());
                            failures.push(format!("{active_name}: {rollback_error}"));
                        }
                    }
                }
                self.active = failed_modules.into_iter().rev().collect();
                let suffix = if failures.is_empty() {
                    String::new()
                } else {
                    format!("; rollback failures: {}", failures.join(", "))
                };
                return Err(LifecycleError::new(format!(
                    "module '{name}' failed to activate: {error}{suffix}"
                )));
            }
            self.active.push(name);
        }
        Ok(())
    }

    /// Deactivate modules in reverse activation order, retaining failures for retry.
    pub async fn stop(&mut self) -> Result<(), LifecycleError> {
        let mut failed = Vec::new();
        for name in self.active.iter().rev() {
            if let Some(module) = self.registry.modules.get(name) {
                if let Err(error) = module.deactivate(&self.context).await {
                    failed.push((name.clone(), error.to_string()));
                }
            }
        }
        self.active = failed.iter().map(|(name, _)| name.clone()).rev().collect();
        if failed.is_empty() {
            Ok(())
        } else {
            let details = failed.into_iter()
                .map(|(name, error)| format!("{name}: {error}"))
                .collect::<Vec<_>>()
                .join(", ");
            Err(LifecycleError::new(format!("async module shutdown failures: {details}")))
        }
    }
}

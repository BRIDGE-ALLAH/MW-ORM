use std::collections::BTreeMap;

use crate::{
    ExtensionRegistry, ExtensionRegistryBuilder, LifecycleError, ModuleManifest,
    ModuleRegistry, ServiceRegistry,
};

/// Read-only configuration and host services exposed to modules.
#[derive(Clone, Debug, Default)]
pub struct ModuleContext {
    configuration: BTreeMap<String, String>,
    services: ServiceRegistry,
    pub(crate) extensions: ExtensionRegistry,
}

impl ModuleContext {
    /// Create a context from string key/value configuration.
    pub fn new(configuration: BTreeMap<String, String>) -> Self {
        Self {
            configuration,
            services: ServiceRegistry::default(),
            extensions: ExtensionRegistry::default(),
        }
    }

    /// Create a context with a prebuilt, immutable service registry.
    pub fn with_services(
        configuration: BTreeMap<String, String>,
        services: ServiceRegistry,
    ) -> Self {
        Self { configuration, services, extensions: ExtensionRegistry::default() }
    }

    /// Read a configuration value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.configuration.get(key).map(String::as_str)
    }

    /// Resolve a shared service by its concrete Rust type.
    ///
    /// Host-provided services take precedence over module-contributed services
    /// of the same concrete type.
    pub fn service<T>(&self) -> Option<std::sync::Arc<T>>
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        self.services.get::<T>().or_else(|| self.extensions.service::<T>())
    }

    /// Dispatch a hook contributed by a registered module.
    pub fn dispatch_hook(
        &self,
        name: &str,
        hook_context: &mut crate::HookContext,
    ) -> Result<usize, LifecycleError> {
        self.extensions.dispatch_hook(name, hook_context)
    }

    /// Publish an event to registered in-process subscribers.
    pub fn publish_event<E>(&self, event: &E) -> Result<usize, LifecycleError>
    where
        E: Send + Sync + 'static,
    {
        self.extensions.publish_event(event)
    }
}

/// A statically linked module and its lifecycle contract.
pub trait Module: Send + Sync {
    /// Return the module's immutable descriptor.
    fn manifest(&self) -> &ModuleManifest;

    /// Register services, hooks, and event subscribers before the registry is frozen.
    fn contribute(&self, _extensions: &mut ExtensionRegistryBuilder) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Validate readiness without performing irreversible side effects.
    fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Activate the module after all modules have prepared successfully.
    /// If this returns an error, the host calls `deactivate` on this module
    /// as best-effort cleanup before rolling back earlier activations.
    fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Deactivate a previously activated module.
    fn deactivate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }
}

/// Synchronous lifecycle host for a validated module registry.
pub struct ModuleHost {
    registry: ModuleRegistry,
    context: ModuleContext,
    active: Vec<String>,
}

impl ModuleHost {
    /// Create a host and attach the registry's immutable contributions to its context.
    pub fn new(registry: ModuleRegistry, context: ModuleContext) -> Self {
        let mut context = context;
        context.extensions = registry.extensions.clone();
        Self { registry, context, active: Vec::new() }
    }

    /// Return identifiers of modules considered active by the host.
    ///
    /// Modules whose deactivation failed remain in this list so callers can
    /// inspect the incomplete shutdown and retry it.
    pub fn active_modules(&self) -> &[String] { &self.active }

    /// Prepare and activate modules in deterministic dependency order.
    ///
    /// If activation fails, modules already activated in this call are
    /// deactivated in reverse order. Modules whose rollback fails remain marked
    /// active and can be retried with `stop`.
    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if !self.active.is_empty() {
            return Err(LifecycleError::new(
                "module host is already started or requires shutdown retry",
            ));
        }
        let plan = self.registry.activation_plan().ordered_modules().to_vec();
        for name in &plan {
            let module = self.registry.modules.get(name)
                .expect("registry plan only contains registered modules");
            module.prepare(&self.context)
                .map_err(|error| {
                    LifecycleError::new(format!("module '{name}' failed to prepare: {error}"))
                })?;
        }
        for name in plan {
            let module = self.registry.modules.get(&name)
                .expect("registry plan only contains registered modules");
            if let Err(error) = module.activate(&self.context) {
                let mut failures = Vec::new();
                if let Err(rollback_error) = module.deactivate(&self.context) {
                    self.active.push(name.clone());
                    failures.push(format!("{name}: {rollback_error}"));
                } else {
                    while let Some(active_name) = self.active.last().cloned() {
                        let active_module = self.registry.modules.get(&active_name)
                            .expect("active modules are registered");
                        if let Err(rollback_error) = active_module.deactivate(&self.context) {
                            failures.push(format!("{active_name}: {rollback_error}"));
                            break;
                        }
                        self.active.pop();
                    }
                }
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

    /// Deactivate active modules in reverse activation order, retaining failures for retry.
    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        while let Some(name) = self.active.last().cloned() {
            let module = self.registry.modules.get(&name)
                .expect("active modules are registered");
            if let Err(error) = module.deactivate(&self.context) {
                return Err(LifecycleError::new(format!(
                    "module shutdown failed at '{name}': {error}; dependent modules remain active"
                )));
            }
            self.active.pop();
        }
        Ok(())
    }
}

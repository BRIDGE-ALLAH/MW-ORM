use std::collections::BTreeMap;

use crate::{LifecycleError, ModuleManifest, ModuleRegistry, ServiceRegistry};

/// Read-only configuration and host services exposed to modules.
#[derive(Clone, Debug, Default)]
pub struct ModuleContext {
    configuration: BTreeMap<String, String>,
    services: ServiceRegistry,
}

impl ModuleContext {
    /// Create a context from string key/value configuration.
    pub fn new(configuration: BTreeMap<String, String>) -> Self {
        Self { configuration, services: ServiceRegistry::default() }
    }

    /// Create a context with a prebuilt, immutable service registry.
    pub fn with_services(
        configuration: BTreeMap<String, String>,
        services: ServiceRegistry,
    ) -> Self {
        Self { configuration, services }
    }

    /// Read a configuration value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.configuration.get(key).map(String::as_str)
    }

    /// Resolve a shared service by its concrete Rust type.
    pub fn service<T>(&self) -> Option<std::sync::Arc<T>>
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        self.services.get::<T>()
    }
}

/// A statically linked module and its lifecycle contract.
pub trait Module: Send + Sync {
    /// Return the module's immutable descriptor.
    fn manifest(&self) -> &ModuleManifest;

    /// Validate readiness without performing irreversible side effects.
    fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Activate the module after all modules have prepared successfully.
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
    /// Create a host for a validated registry.
    pub fn new(registry: ModuleRegistry, context: ModuleContext) -> Self {
        Self { registry, context, active: Vec::new() }
    }

    /// Return identifiers of modules considered active by the host.
    ///
    /// Modules whose deactivation failed remain in this list so callers can
    /// inspect the incomplete shutdown and retry it.
    pub fn active_modules(&self) -> &[String] {
        &self.active
    }

    /// Prepare and activate modules in deterministic dependency order.
    ///
    /// If activation fails, modules already activated in this call are
    /// deactivated in reverse order. Modules whose rollback fails remain marked
    /// active and can be retried with `stop`.
    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if !self.active.is_empty() {
            return Err(LifecycleError::new("module host is already started or requires shutdown retry"));
        }

        let plan = self.registry.activation_plan().ordered_modules().to_vec();
        for name in &plan {
            let module = self.registry.modules.get(name)
                .expect("registry plan only contains registered modules");
            module.prepare(&self.context)
                .map_err(|error| LifecycleError::new(format!("module '{name}' failed to prepare: {error}")))?;
        }

        for name in plan {
            let module = self.registry.modules.get(&name)
                .expect("registry plan only contains registered modules");
            if let Err(error) = module.activate(&self.context) {
                let mut failed_modules = Vec::new();
                let mut failures = Vec::new();
                for active_name in self.active.iter().rev() {
                    if let Some(active_module) = self.registry.modules.get(active_name) {
                        if let Err(rollback_error) = active_module.deactivate(&self.context) {
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

    /// Deactivate active modules in reverse activation order.
    ///
    /// Every module is given a chance to stop. Modules that fail to deactivate
    /// remain marked active so shutdown can be retried.
    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        let mut failed = Vec::new();
        for name in self.active.iter().rev() {
            if let Some(module) = self.registry.modules.get(name) {
                if let Err(error) = module.deactivate(&self.context) {
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
            Err(LifecycleError::new(format!("module shutdown failures: {details}")))
        }
    }
}

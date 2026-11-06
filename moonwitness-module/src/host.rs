use std::collections::BTreeMap;

use crate::{LifecycleError, ModuleManifest, ModuleRegistry};

/// Read-only configuration and host metadata exposed to modules.
#[derive(Clone, Debug, Default)]
pub struct ModuleContext {
    configuration: BTreeMap<String, String>,
}

impl ModuleContext {
    /// Create a context from string key/value configuration.
    pub fn new(configuration: BTreeMap<String, String>) -> Self {
        Self { configuration }
    }

    /// Read a configuration value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.configuration.get(key).map(String::as_str)
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

    /// Return identifiers of currently active modules.
    pub fn active_modules(&self) -> &[String] {
        &self.active
    }

    /// Prepare and activate modules in deterministic dependency order.
    ///
    /// If activation fails, modules already activated in this call are
    /// deactivated in reverse order before the error is returned.
    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if !self.active.is_empty() {
            return Err(LifecycleError::new("module host is already started"));
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
                let mut rollback_failures = Vec::new();
                for active_name in self.active.iter().rev() {
                    if let Some(active_module) = self.registry.modules.get(active_name) {
                        if let Err(rollback_error) = active_module.deactivate(&self.context) {
                            rollback_failures.push(format!("{active_name}: {rollback_error}"));
                        }
                    }
                }
                self.active.clear();
                let suffix = if rollback_failures.is_empty() {
                    String::new()
                } else {
                    format!("; rollback failures: {}", rollback_failures.join(", "))
                };
                return Err(LifecycleError::new(format!(
                    "module '{name}' failed to activate: {error}{suffix}"
                )));
            }
            self.active.push(name);
        }
        Ok(())
    }

    /// Deactivate all active modules in reverse activation order.
    ///
    /// All modules are given a chance to stop. Any failures are combined into
    /// one returned error after the remaining modules have been processed.
    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        let mut failures = Vec::new();
        for name in self.active.iter().rev() {
            if let Some(module) = self.registry.modules.get(name) {
                if let Err(error) = module.deactivate(&self.context) {
                    failures.push(format!("{name}: {error}"));
                }
            }
        }
        self.active.clear();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(LifecycleError::new(format!("module shutdown failures: {}", failures.join(", "))))
        }
    }
}

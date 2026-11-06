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
    ///
    /// If activation returns an error after partially acquiring resources, the
    /// host calls `deactivate` on this module as part of rollback.
    fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Deactivate a module, including cleanup after a partially failed activation.
    ///
    /// Implementations should make this operation safe to retry. If it returns
    /// an error, the host retains the module in its active set so shutdown can
    /// be retried rather than silently forgetting a resource that may remain live.
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
        Self {
            registry,
            context,
            active: Vec::new(),
        }
    }

    /// Return identifiers of modules that may still hold active resources.
    ///
    /// A module remains in this list when deactivation fails, allowing callers
    /// to observe incomplete shutdown and retry it.
    pub fn active_modules(&self) -> &[String] {
        &self.active
    }

    /// Prepare and activate modules in deterministic dependency order.
    ///
    /// If activation fails, the failing module and modules already activated in
    /// this call are deactivated in reverse order. Modules whose cleanup fails
    /// remain recorded as active so a later `stop` call can retry cleanup.
    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if !self.active.is_empty() {
            return Err(LifecycleError::new(
                "module host has modules requiring shutdown",
            ));
        }

        let plan = self.registry.activation_plan().ordered_modules().to_vec();
        for name in &plan {
            let module = self
                .registry
                .modules
                .get(name)
                .expect("registry plan only contains registered modules");
            module.prepare(&self.context).map_err(|error| {
                LifecycleError::new(format!("module '{name}' failed to prepare: {error}"))
            })?;
        }

        for name in plan {
            let module = self
                .registry
                .modules
                .get(&name)
                .expect("registry plan only contains registered modules");
            if let Err(error) = module.activate(&self.context) {
                let mut rollback_failures = Vec::new();
                let mut remaining_active = Vec::new();

                // An activation error may occur after partial resource acquisition.
                if let Err(cleanup_error) = module.deactivate(&self.context) {
                    rollback_failures.push(format!("{name}: {cleanup_error}"));
                    remaining_active.push(name.clone());
                }

                for active_name in self.active.iter().rev() {
                    if let Some(active_module) = self.registry.modules.get(active_name)
                        && let Err(rollback_error) = active_module.deactivate(&self.context)
                    {
                        rollback_failures.push(format!("{active_name}: {rollback_error}"));
                        remaining_active.push(active_name.clone());
                    }
                }
                remaining_active.reverse();
                self.active = remaining_active;

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
    /// All modules are given a chance to stop. Modules whose deactivation fails
    /// remain recorded in activation order, and a later call may retry them.
    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        let mut failures = Vec::new();
        let mut remaining_active = Vec::new();

        for name in self.active.iter().rev() {
            if let Some(module) = self.registry.modules.get(name)
                && let Err(error) = module.deactivate(&self.context)
            {
                failures.push(format!("{name}: {error}"));
                remaining_active.push(name.clone());
            }
        }

        remaining_active.reverse();
        self.active = remaining_active;

        if failures.is_empty() {
            Ok(())
        } else {
            Err(LifecycleError::new(format!(
                "module shutdown failures: {}",
                failures.join(", ")
            )))
        }
    }
}

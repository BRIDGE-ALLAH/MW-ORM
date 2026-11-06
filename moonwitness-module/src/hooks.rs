use std::collections::BTreeMap;

use crate::LifecycleError;

/// Mutable, per-dispatch data shared by ordered hook handlers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HookContext {
    values: BTreeMap<String, String>,
}

impl HookContext {
    /// Create an empty hook context.
    pub fn new() -> Self { Self::default() }

    /// Insert or replace a string value.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.values.insert(key.into(), value.into())
    }

    /// Read a string value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Iterate over context entries in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

type HookCallback = Box<dyn Fn(&mut HookContext) -> Result<(), LifecycleError> + Send + Sync>;

struct Hook {
    module: String,
    id: String,
    priority: i32,
    callback: HookCallback,
}

/// Deterministic in-process hook registry.
///
/// Handlers execute by ascending priority, then module identifier, then hook
/// identifier. Dispatch is fail-fast: the first handler error stops the chain.
#[derive(Default)]
pub struct HookRegistry {
    hooks: BTreeMap<String, Vec<Hook>>,
}

impl HookRegistry {
    /// Create an empty hook registry.
    pub fn new() -> Self { Self::default() }

    /// Register a handler for a named hook.
    pub fn register<F>(
        &mut self,
        hook_name: impl Into<String>,
        module: impl Into<String>,
        id: impl Into<String>,
        priority: i32,
        callback: F,
    ) -> Result<(), LifecycleError>
    where
        F: Fn(&mut HookContext) -> Result<(), LifecycleError> + Send + Sync + 'static,
    {
        let hook_name = hook_name.into();
        let module = module.into();
        let id = id.into();
        if hook_name.trim().is_empty() || module.trim().is_empty() || id.trim().is_empty() {
            return Err(LifecycleError::new("hook name, module, and handler id must be non-empty"));
        }
        let handlers = self.hooks.entry(hook_name.clone()).or_default();
        if handlers.iter().any(|handler| handler.module == module && handler.id == id) {
            return Err(LifecycleError::new(format!(
                "duplicate handler '{module}:{id}' for hook '{hook_name}'"
            )));
        }
        handlers.push(Hook { module, id, priority, callback: Box::new(callback) });
        handlers.sort_by(|left, right| {
            (left.priority, &left.module, &left.id).cmp(&(right.priority, &right.module, &right.id))
        });
        Ok(())
    }

    /// Dispatch a hook and return the number of handlers that completed.
    pub fn dispatch(
        &self,
        hook_name: &str,
        context: &mut HookContext,
    ) -> Result<usize, LifecycleError> {
        let Some(handlers) = self.hooks.get(hook_name) else { return Ok(0); };
        for (index, handler) in handlers.iter().enumerate() {
            (handler.callback)(context).map_err(|error| LifecycleError::new(format!(
                "hook '{hook_name}' handler '{}:{}' failed after {} completed handler(s): {error}",
                handler.module, handler.id, index
            )))?;
        }
        Ok(handlers.len())
    }

    /// Return the number of handlers registered for a hook.
    pub fn handler_count(&self, hook_name: &str) -> usize {
        self.hooks.get(hook_name).map_or(0, Vec::len)
    }
}

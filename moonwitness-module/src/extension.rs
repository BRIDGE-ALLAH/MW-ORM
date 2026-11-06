use std::sync::Arc;

use crate::{
    EventBus, HookContext, HookRegistry, LifecycleError, ServiceRegistry,
    ServiceRegistryBuilder,
};

/// Mutable collector for typed services, named hooks, and typed events.
///
/// Modules contribute to this builder during registry construction. Once the
/// registry is built, the resulting extension set is immutable.
#[derive(Default)]
pub struct ExtensionRegistryBuilder {
    services: ServiceRegistryBuilder,
    hooks: HookRegistry,
    events: EventBus,
}

impl ExtensionRegistryBuilder {
    /// Create an empty extension registry builder.
    pub fn new() -> Self { Self::default() }

    /// Register one shared service by concrete Rust type.
    pub fn register_service<T>(&mut self, service: T) -> Result<&mut Self, LifecycleError>
    where
        T: Send + Sync + 'static,
    {
        self.services.register(service)?;
        Ok(self)
    }

    /// Register a named hook handler.
    pub fn register_hook<F>(
        &mut self,
        hook_name: impl Into<String>,
        module: impl Into<String>,
        id: impl Into<String>,
        priority: i32,
        callback: F,
    ) -> Result<&mut Self, LifecycleError>
    where
        F: Fn(&mut HookContext) -> Result<(), LifecycleError> + Send + Sync + 'static,
    {
        self.hooks.register(hook_name, module, id, priority, callback)?;
        Ok(self)
    }

    /// Subscribe a module handler to a typed event.
    pub fn subscribe_event<E, F>(
        &mut self,
        module: impl Into<String>,
        id: impl Into<String>,
        callback: F,
    ) -> Result<&mut Self, LifecycleError>
    where
        E: Send + Sync + 'static,
        F: Fn(&E) -> Result<(), LifecycleError> + Send + Sync + 'static,
    {
        self.events.subscribe::<E, F>(module, id, callback)?;
        Ok(self)
    }

    /// Freeze all contributions into an immutable extension registry.
    pub fn build(self) -> ExtensionRegistry {
        ExtensionRegistry {
            services: self.services.build(),
            hooks: Arc::new(self.hooks),
            events: Arc::new(self.events),
        }
    }
}

/// Immutable, application-wide view of module contributions.
#[derive(Clone, Default)]
pub struct ExtensionRegistry {
    services: ServiceRegistry,
    hooks: Arc<HookRegistry>,
    events: Arc<EventBus>,
}

impl std::fmt::Debug for ExtensionRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ExtensionRegistry")
            .field("service_count", &self.services.len())
            .finish_non_exhaustive()
    }
}

impl ExtensionRegistry {
    /// Create an extension registry containing only the supplied services.
    pub fn from_services(services: ServiceRegistry) -> Self {
        Self { services, hooks: Arc::default(), events: Arc::default() }
    }

    /// Resolve a shared service by concrete Rust type.
    pub fn service<T>(&self) -> Option<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.services.get::<T>()
    }

    /// Dispatch a named hook against a mutable per-call context.
    pub fn dispatch_hook(
        &self,
        name: &str,
        context: &mut HookContext,
    ) -> Result<usize, LifecycleError> {
        self.hooks.dispatch(name, context)
    }

    /// Publish a typed in-process event.
    pub fn publish_event<E>(&self, event: &E) -> Result<usize, LifecycleError>
    where
        E: Send + Sync + 'static,
    {
        self.events.publish(event)
    }

    /// Return the number of services in this registry.
    pub fn service_count(&self) -> usize { self.services.len() }
}

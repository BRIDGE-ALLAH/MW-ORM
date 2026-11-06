use std::sync::Arc;

use crate::{
    EventBus, HookContext, HookRegistry, LifecycleError,
    MigrationCatalog, MigrationDescriptor, MigrationError, ServiceRegistry,
    ServiceRegistryBuilder,
};

/// Mutable collector for typed services, named hooks, typed events, and migrations.
///
/// Modules contribute to this builder during registry construction. Once the
/// registry is built, the resulting extension set is immutable.
#[derive(Default)]
pub struct ExtensionRegistryBuilder {
    services: ServiceRegistryBuilder,
    hooks: HookRegistry,
    events: EventBus,
    migrations: Vec<MigrationDescriptor>,
    current_module: Option<String>,
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
        let module = module.into();
        self.ensure_owner(&module)?;
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
        let module = module.into();
        self.ensure_owner(&module)?;
        self.events.subscribe::<E, F>(module, id, callback)?;
        Ok(self)
    }

    /// Register a migration owned by the module currently contributing.
    pub fn register_migration(
        &mut self,
        migration: MigrationDescriptor,
    ) -> Result<&mut Self, LifecycleError> {
        let Some(owner) = self.current_module.as_deref() else {
            return Err(LifecycleError::new("migrations can only be registered during module contribution"));
        };
        if migration.module.as_str() != owner {
            return Err(LifecycleError::new(format!(
                "module '{owner}' cannot register migration owned by '{}'", migration.module
            )));
        }
        self.migrations.push(migration);
        Ok(self)
    }

    fn ensure_owner(&self, declared: &str) -> Result<(), LifecycleError> {
        let Some(owner) = self.current_module.as_deref() else {
            return Err(LifecycleError::new("hooks and events can only be registered during module contribution"));
        };
        if declared != owner {
            return Err(LifecycleError::new(format!(
                "module '{owner}' cannot register an extension owned by '{declared}'"
            )));
        }
        Ok(())
    }

    pub(crate) fn begin_module(&mut self, module: &str) {
        self.current_module = Some(module.to_owned());
    }

    pub(crate) fn end_module(&mut self) {
        self.current_module = None;
    }

    /// Freeze all contributions into an immutable extension registry.
    pub fn build(self) -> Result<ExtensionRegistry, LifecycleError> {
        let migrations = MigrationCatalog::new(self.migrations)
            .map_err(|error: MigrationError| LifecycleError::new(error.to_string()))?;
        Ok(ExtensionRegistry {
            services: self.services.build(),
            hooks: Arc::new(self.hooks),
            events: Arc::new(self.events),
            migrations,
        })
    }
}

/// Immutable, application-wide view of module contributions.
#[derive(Clone, Default)]
pub struct ExtensionRegistry {
    services: ServiceRegistry,
    hooks: Arc<HookRegistry>,
    events: Arc<EventBus>,
    migrations: MigrationCatalog,
}

impl std::fmt::Debug for ExtensionRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ExtensionRegistry")
            .field("service_count", &self.services.len())
            .field("migration_count", &self.migrations.migrations().len())
            .finish_non_exhaustive()
    }
}

impl ExtensionRegistry {
    /// Create an extension registry containing only the supplied services.
    pub fn from_services(services: ServiceRegistry) -> Self {
        Self {
            services,
            hooks: Arc::default(),
            events: Arc::default(),
            migrations: MigrationCatalog::default(),
        }
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

    /// Return the immutable migration catalog contributed by registered modules.
    pub fn migrations(&self) -> &MigrationCatalog { &self.migrations }

    pub fn service_count(&self) -> usize { self.services.len() }
}

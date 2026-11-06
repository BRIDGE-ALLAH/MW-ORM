use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};

use crate::LifecycleError;

/// Builder for an immutable, type-indexed service collection.
#[derive(Default)]
pub struct ServiceRegistryBuilder {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ServiceRegistryBuilder {
    /// Create an empty service registry builder.
    pub fn new() -> Self { Self::default() }

    /// Register one shared service instance for its concrete Rust type.
    pub fn register<T>(&mut self, service: T) -> Result<&mut Self, LifecycleError>
    where
        T: Any + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        if self.services.contains_key(&type_id) {
            return Err(LifecycleError::new(format!(
                "service type '{}' is already registered", std::any::type_name::<T>()
            )));
        }
        self.services.insert(type_id, Arc::new(service));
        Ok(self)
    }

    /// Freeze the builder into a read-only service registry.
    pub fn build(self) -> ServiceRegistry {
        ServiceRegistry { services: self.services }
    }
}

/// Immutable collection of shared, strongly typed service instances.
#[derive(Clone, Default)]
pub struct ServiceRegistry {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    /// Resolve a shared service by its concrete Rust type.
    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        self.services.get(&TypeId::of::<T>()).cloned()?.downcast::<T>().ok()
    }

    /// Return the number of registered service types.
    pub fn len(&self) -> usize { self.services.len() }

    /// Return whether the registry contains no services.
    pub fn is_empty(&self) -> bool { self.services.is_empty() }
}

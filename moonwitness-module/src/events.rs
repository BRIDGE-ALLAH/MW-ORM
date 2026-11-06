use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};

use crate::LifecycleError;

type EventCallback = Arc<dyn Fn(&(dyn Any + Send + Sync)) -> Result<(), LifecycleError> + Send + Sync>;

struct Handler {
    module: String,
    id: String,
    callback: EventCallback,
}

/// Typed, in-process event dispatcher.
///
/// Subscribers are invoked deterministically by module identifier and handler
/// identifier. Delivery is synchronous and fail-fast; this is not a durable
/// message queue and does not provide retry or exactly-once guarantees.
#[derive(Default)]
pub struct EventBus {
    handlers: HashMap<TypeId, Vec<Handler>>,
}

impl EventBus {
    /// Create an empty event bus.
    pub fn new() -> Self { Self::default() }

    /// Subscribe to an event type with a stable module and handler identity.
    pub fn subscribe<E, F>(
        &mut self,
        module: impl Into<String>,
        id: impl Into<String>,
        callback: F,
    ) -> Result<(), LifecycleError>
    where
        E: Any + Send + Sync + 'static,
        F: Fn(&E) -> Result<(), LifecycleError> + Send + Sync + 'static,
    {
        let module = module.into();
        let id = id.into();
        if module.trim().is_empty() || id.trim().is_empty() {
            return Err(LifecycleError::new("event subscriber module and id must be non-empty"));
        }
        let handlers = self.handlers.entry(TypeId::of::<E>()).or_default();
        if handlers.iter().any(|handler| handler.module == module && handler.id == id) {
            return Err(LifecycleError::new(format!("duplicate event handler '{module}:{id}'")));
        }
        let erased: EventCallback = Arc::new(move |event| {
            let event = event.downcast_ref::<E>().expect("event TypeId matches subscriber type");
            callback(event)
        });
        handlers.push(Handler { module, id, callback: erased });
        handlers.sort_by(|left, right| (&left.module, &left.id).cmp(&(&right.module, &right.id)));
        Ok(())
    }

    /// Publish an event and return the number of subscribers invoked.
    pub fn publish<E>(&self, event: &E) -> Result<usize, LifecycleError>
    where
        E: Any + Send + Sync + 'static,
    {
        let Some(handlers) = self.handlers.get(&TypeId::of::<E>()) else { return Ok(0); };
        for (index, handler) in handlers.iter().enumerate() {
            (handler.callback)(event).map_err(|error| LifecycleError::new(format!(
                "event handler '{}:{}' failed after {} completed handler(s): {error}",
                handler.module, handler.id, index
            )))?;
        }
        Ok(handlers.len())
    }

    /// Return the number of subscribers for an event type.
    pub fn subscriber_count<E: Any + Send + Sync + 'static>(&self) -> usize {
        self.handlers.get(&TypeId::of::<E>()).map_or(0, Vec::len)
    }
}

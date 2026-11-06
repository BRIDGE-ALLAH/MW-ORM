//! Static, typed module composition for MoonWitness applications.
//!
//! This crate provides deterministic module registration, manifest validation,
//! dependency planning, synchronous lifecycle hosting, typed service sharing,
//! ordered hooks, in-process events, and migration planning metadata.
//! Database-specific migration execution and dynamic library loading are
//! intentionally outside this crate's current scope.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod async_host;
mod error;
mod events;
mod extension;
mod hooks;
mod host;
mod manifest;
mod migration;
mod registry;
mod resolver;
mod services;

pub use async_host::{AsyncModule, AsyncModuleHost, AsyncModuleRegistry, AsyncModuleRegistryBuilder};
pub use error::{KernelError, LifecycleError};
pub use events::EventBus;
pub use extension::{ExtensionRegistry, ExtensionRegistryBuilder};
pub use hooks::{HookContext, HookRegistry};
pub use host::{Module, ModuleContext, ModuleHost};
pub use manifest::{ModuleDependency, ModuleManifest};
pub use migration::{
    AppliedMigration, MigrationCatalog, MigrationDescriptor, MigrationError, MigrationPlan,
};
pub use registry::{ModuleRegistry, ModuleRegistryBuilder};
pub use resolver::ActivationPlan;
pub use services::{ServiceRegistry, ServiceRegistryBuilder};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod extension_tests;

#[cfg(test)]
mod async_tests;

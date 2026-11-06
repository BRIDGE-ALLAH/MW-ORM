//! Static, typed module composition for MoonWitness applications.
//!
//! This crate is deliberately independent from the ORM crate. It provides a
//! deterministic module registry, manifest validation, dependency planning,
//! and a synchronous lifecycle host. Dynamic library loading and database
//! migration execution are intentionally outside this MVP.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod host;
mod manifest;
mod registry;
mod resolver;

pub use error::{KernelError, LifecycleError};
pub use host::{Module, ModuleContext, ModuleHost};
pub use manifest::{ModuleDependency, ModuleManifest};
pub use registry::{ModuleRegistry, ModuleRegistryBuilder};
pub use resolver::ActivationPlan;

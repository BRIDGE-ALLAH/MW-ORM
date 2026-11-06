use thiserror::Error;

/// Errors raised while validating or composing a module application.
#[derive(Debug, Error)]
pub enum KernelError {
    /// A module identifier is malformed.
    #[error("invalid module identifier: {0}")]
    InvalidModuleName(String),
    /// A module identifier is already registered.
    #[error("module '{0}' is registered more than once")]
    DuplicateModule(String),
    /// A module manifest contains inconsistent declarations.
    #[error("invalid manifest for module '{module}': {reason}")]
    InvalidManifest {
        /// Identifier of the module with the invalid manifest.
        module: String,
        /// Explanation of the manifest validation failure.
        reason: String,
    },
    /// A required module is not registered.
    #[error("module '{module}' requires missing module '{dependency}'")]
    MissingDependency {
        /// Identifier of the module declaring the dependency.
        module: String,
        /// Identifier of the required module that is not registered.
        dependency: String,
    },
    /// A dependency's version does not satisfy the declared requirement.
    #[error("module '{module}' requires '{dependency}' {required}, but found {found}")]
    IncompatibleDependency {
        /// Identifier of the module declaring the dependency.
        module: String,
        /// Identifier of the dependency whose version did not match.
        dependency: String,
        /// Version requirement declared by the dependent module.
        required: String,
        /// Version actually provided by the registered dependency.
        found: String,
    },
    /// The module dependency graph contains a cycle.
    #[error("module dependency cycle detected: {0}")]
    DependencyCycle(String),
    /// No module provides a required capability.
    #[error("module '{module}' requires capability '{capability}', but no provider is registered")]
    MissingCapability {
        /// Identifier of the module requiring the capability.
        module: String,
        /// Capability for which no provider was registered.
        capability: String,
    },
    /// More than one module provides a capability that must be unique.
    #[error("capability '{capability}' has multiple providers: {providers}")]
    AmbiguousCapability {
        /// Capability requested by a module.
        capability: String,
        /// Comma-separated identifiers of the conflicting providers.
        providers: String,
    },
    /// A manifest document could not be decoded.
    #[error("could not parse module manifest: {0}")]
    ManifestParse(String),
    /// A lifecycle operation failed.
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
}

/// An error returned by a module during a lifecycle operation.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct LifecycleError {
    /// Human-readable failure detail.
    pub message: String,
}

impl LifecycleError {
    /// Create a lifecycle error with a human-readable message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::KernelError;

/// A required or optional dependency on another module.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleDependency {
    /// Stable module identifier.
    pub name: String,
    /// Accepted semantic-version range.
    pub version: VersionReq,
    /// Whether the dependency may be absent.
    #[serde(default)]
    pub optional: bool,
}

/// Declarative identity and compatibility contract for a module.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// Stable, lowercase module identifier (for example, `mw.audit`).
    pub name: String,
    /// Published module version.
    pub version: Version,
    /// Version of the MoonWitness module API implemented by this module.
    pub api_version: Version,
    /// Other modules this module depends on.
    #[serde(default)]
    pub dependencies: Vec<ModuleDependency>,
    /// Capabilities exported by this module.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Capabilities that must be supplied by another registered module.
    #[serde(default)]
    pub requires_capabilities: Vec<String>,
}

impl ModuleManifest {
    /// Validate identifiers and reject duplicate or self-referential declarations.
    pub fn validate(&self) -> Result<(), KernelError> {
        validate_name(&self.name)?;
        let mut dependencies = std::collections::BTreeSet::new();
        for dependency in &self.dependencies {
            validate_name(&dependency.name)?;
            if dependency.name == self.name {
                return Err(KernelError::InvalidManifest {
                    module: self.name.clone(),
                    reason: "a module cannot depend on itself".into(),
                });
            }
            if !dependencies.insert(&dependency.name) {
                return Err(KernelError::InvalidManifest {
                    module: self.name.clone(),
                    reason: format!(
                        "dependency '{}' is declared more than once",
                        dependency.name
                    ),
                });
            }
        }
        validate_unique_names(&self.name, "provided capability", &self.provides)?;
        validate_unique_names(
            &self.name,
            "required capability",
            &self.requires_capabilities,
        )?;
        Ok(())
    }

    /// Parse a TOML module manifest.
    pub fn from_toml(source: &str) -> Result<Self, KernelError> {
        let manifest: Self = toml::from_str(source)
            .map_err(|error| KernelError::ManifestParse(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
}

fn validate_name(name: &str) -> Result<(), KernelError> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name.split('.').all(|part| {
            !part.is_empty()
                && part.as_bytes()[0].is_ascii_lowercase()
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'
                        || byte == b'_'
                })
        });
    if valid {
        Ok(())
    } else {
        Err(KernelError::InvalidModuleName(name.to_owned()))
    }
}

fn validate_unique_names(module: &str, label: &str, values: &[String]) -> Result<(), KernelError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || !seen.insert(value) {
            return Err(KernelError::InvalidManifest {
                module: module.to_owned(),
                reason: format!("{label} '{value}' is empty or duplicated"),
            });
        }
    }
    Ok(())
}

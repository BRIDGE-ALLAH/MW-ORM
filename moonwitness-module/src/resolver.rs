use std::collections::{BTreeMap, BTreeSet};

use semver::Version;

use crate::{KernelError, ModuleManifest};

/// A deterministic, dependency-first order in which modules may be activated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationPlan {
    ordered: Vec<String>,
}

impl ActivationPlan {
    /// Return module identifiers in activation order.
    pub fn ordered_modules(&self) -> &[String] {
        &self.ordered
    }
}

pub(crate) fn resolve(
    manifests: &BTreeMap<String, ModuleManifest>,
) -> Result<ActivationPlan, KernelError> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = manifests
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect();

    let mut providers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, manifest) in manifests {
        for capability in &manifest.provides {
            providers.entry(capability.clone()).or_default().push(name.clone());
        }
    }

    for (name, manifest) in manifests {
        for dependency in &manifest.dependencies {
            match manifests.get(&dependency.name) {
                Some(found) => {
                    if !dependency.version.matches(&found.version) {
                        return Err(KernelError::IncompatibleDependency {
                            module: name.clone(),
                            dependency: dependency.name.clone(),
                            required: dependency.version.to_string(),
                            found: found.version.to_string(),
                        });
                    }
                    edges.get_mut(name).expect("registered node").insert(dependency.name.clone());
                }
                None if dependency.optional => {}
                None => {
                    return Err(KernelError::MissingDependency {
                        module: name.clone(),
                        dependency: dependency.name.clone(),
                    });
                }
            }
        }

        for capability in &manifest.requires_capabilities {
            match providers.get(capability) {
                None => return Err(KernelError::MissingCapability {
                    module: name.clone(),
                    capability: capability.clone(),
                }),
                Some(found) if found.len() > 1 => {
                    return Err(KernelError::AmbiguousCapability {
                        capability: capability.clone(),
                        providers: found.join(", "),
                    });
                }
                Some(found) => {
                    let provider = &found[0];
                    if provider != name {
                        edges.get_mut(name).expect("registered node").insert(provider.clone());
                    }
                }
            }
        }
    }

    // Kahn's algorithm with ordered collections makes the result reproducible.
    let mut dependents: BTreeMap<String, BTreeSet<String>> = manifests
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect();
    let mut indegree: BTreeMap<String, usize> = edges
        .iter()
        .map(|(name, dependencies)| (name.clone(), dependencies.len()))
        .collect();
    for (module, dependencies) in &edges {
        for dependency in dependencies {
            dependents.get_mut(dependency).expect("registered node").insert(module.clone());
        }
    }

    let mut ready: BTreeSet<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(name, _)| name.clone())
        .collect();
    let mut ordered = Vec::with_capacity(manifests.len());

    while let Some(next) = ready.pop_first() {
        ordered.push(next.clone());
        for dependent in dependents.get(&next).expect("registered node") {
            let degree = indegree.get_mut(dependent).expect("registered node");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent.clone());
            }
        }
    }

    if ordered.len() != manifests.len() {
        let cycle = indegree
            .iter()
            .filter(|(_, degree)| **degree > 0)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(KernelError::DependencyCycle(cycle));
    }

    Ok(ActivationPlan { ordered })
}

/// Check whether a module version satisfies a semantic-version requirement.
pub fn version_matches(version: &Version, requirement: &semver::VersionReq) -> bool {
    requirement.matches(version)
}

# moonwitness-module

The first MoonWitness module-kernel crate. It is a small, statically linked module host that is independent of the existing ORM crate.

## Current MVP

- Typed module descriptors with semantic versions and TOML parsing.
- Manifest validation and duplicate registration checks.
- Required and optional module dependencies.
- Semantic-version compatibility checks.
- Capability provider resolution with missing/ambiguous-provider diagnostics.
- Deterministic dependency-first activation plans.
- A static Rust `Module` contract and immutable registry.
- Prepare/activate/deactivate lifecycle hooks.
- Reverse-order shutdown and rollback after activation failure, including cleanup of a partially activated module.
- Failed cleanup remains visible in the host's active set and can be retried with a later shutdown call.

## Example manifest

```toml
name = "mw.audit"
version = "0.1.0"
api_version = "1.0.0"
provides = ["audit.events"]

[[dependencies]]
name = "mw.core"
version = "^1.0"
optional = false
```

## Design boundaries

This crate intentionally does not load arbitrary native dynamic libraries, execute database migrations, mutate ORM entities, or provide a service container. Those concerns require separate security, compatibility, and transaction designs. Modules are statically linked Rust values in this MVP.

## Use in the workspace

```toml
[dependencies]
moonwitness-module = { path = "../moonwitness-module" }
```

The crate is an early kernel foundation, not yet a production-complete plugin platform. See [the architecture blueprint](../docs/ARCHITECTURE.md) and [implementation blueprint](../docs/BLUEPRINT.md).

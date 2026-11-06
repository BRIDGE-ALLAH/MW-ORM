# moonwitness-module

The experimental MoonWitness module kernel provides typed, statically linked
module composition for Rust applications.

## Current capabilities

- TOML module manifests with semantic versions and dependency declarations.
- Required and optional module dependency validation.
- Capability provider validation and deterministic dependency-first planning.
- Static synchronous and asynchronous module registries with prepare/activate/deactivate lifecycle.
- Best-effort activation rollback and reverse-order shutdown.
- Module contribution phase that freezes services, hooks, and event subscribers into one immutable extension registry.
- Typed, immutable service registry.
- Deterministically ordered named hooks with fail-fast dispatch.
- Typed in-process events with deterministic subscriber order.
- Module-scoped migration descriptors, SHA-256 checksums, history validation,
  and deterministic pending migration plans.

## Minimal module

```rust
use moonwitness_module::{LifecycleError, Module, ModuleContext, ModuleManifest};

struct CoreModule {
    manifest: ModuleManifest,
}

impl Module for CoreModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn prepare(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        Ok(())
    }
}
```

## Extension contracts

Hooks are named, ordered callbacks. A hook handler receives a mutable
`HookContext`; handlers run by ascending priority, then module and handler id.
Dispatch stops on the first error.

Events are Rust-type-based and delivered synchronously. Subscriber order is
stable by module and handler id. The event bus is in-process only: it is not a
durable queue and makes no retry or exactly-once delivery guarantee.

Services are registered once per concrete Rust type and exposed through an
immutable registry of shared `Arc` instances.

## Migration planning

`MigrationDescriptor::new` calculates a SHA-256 checksum from migration source
bytes. `MigrationCatalog` rejects duplicate module-scoped identifiers or
versions, validates the checksums of applied history records, and produces a
deterministic pending plan.

This crate does **not** execute SQL, open database transactions, acquire
database locks, or write migration history. A database adapter must implement
those operational guarantees before this can be described as a migration
runner. Destructive rollback is never implied.

## Current limitations

- Synchronous and async lifecycle hosts are available; both use static Rust module values.
- Module preparation and activation are not connected to a database transaction.
- Migration support is planning and integrity metadata only, not execution.
- Modules contribute services, hooks, and event handlers during registry construction; the host exposes the frozen registry through `ModuleContext`.
- There is no dynamic library loading, hot reload, tenant isolation, or
  production security certification.
- The kernel is experimental and its public API may change.

The standalone ORM remains independent of this optional crate.

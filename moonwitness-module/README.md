# moonwitness-module

The experimental MoonWitness module kernel provides typed, statically linked
module composition for Rust applications.

## Current capabilities

- TOML module manifests with semantic versions, dependency declarations, and kernel API compatibility checks.
- Required and optional module dependency validation.
- Capability provider validation and deterministic dependency-first planning.
- Static synchronous and asynchronous module registries with prepare/activate/deactivate lifecycle.
- Best-effort activation rollback and reverse-order shutdown.
- Module contribution phase that freezes services, hooks, event subscribers, and owner-validated migration descriptors into one immutable extension registry. Hooks, events, and migrations cannot be registered under another module's identity.
- Typed, immutable service registry.
- Deterministically ordered named hooks with fail-fast dispatch.
- Typed in-process events with deterministic subscriber order.
- Module-scoped migration descriptors, SHA-256 checksums, history validation,
  and deterministic pending migration plans.
- Async migration coordinator contract with namespace locking, sequential application, and lock cleanup.

## Quick start

Register statically linked modules, validate their dependency graph, then start
the host. The same registry and lifecycle model is available asynchronously
through `AsyncModule`, `AsyncModuleRegistryBuilder`, and `AsyncModuleHost`.

```rust
use semver::Version;
use moonwitness_module::{
    LifecycleError, Module, ModuleContext, ModuleHost, ModuleManifest,
    ModuleRegistryBuilder,
};

struct CoreModule {
    manifest: ModuleManifest,
}

impl Module for CoreModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn activate(&self, _context: &ModuleContext) -> Result<(), LifecycleError> {
        println!("MoonWitness core is active");
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = ModuleRegistryBuilder::new();
    registry.register(CoreModule {
        manifest: ModuleManifest {
            name: "mw.core".into(),
            version: Version::new(0, 1, 0),
            api_version: Version::new(1, 0, 0),
            dependencies: Vec::new(),
            provides: vec!["core.runtime".into()],
            requires_capabilities: Vec::new(),
        },
    })?;

    let mut host = ModuleHost::new(registry.build()?, ModuleContext::default());
    host.start()?;
    host.stop()?;
    Ok(())
}
```

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

`MigrationDescriptor::new` retains immutable source bytes and calculates a SHA-256 checksum from those exact bytes. `MigrationCatalog` rejects duplicate module-scoped identifiers or versions, validates applied migration versions and checksums, and produces a deterministic pending plan. The async `MigrationRunner` passes the application namespace and SQL source to a backend, coordinates lock acquisition/release, validates history, and applies pending migrations in order.

The kernel does **not** ship a production SQLx/SeaORM backend. A database adapter must implement real database locks, SQL execution, durable history, and transaction semantics before module-owned migrations can run against an application database. Destructive rollback is never implied.

## Current limitations

- Synchronous and async lifecycle hosts are available; both use static Rust module values.
- Module preparation and activation are not connected to a database transaction.
- Migration coordination is generic; a production database backend with real transaction/locking integration is still required.
- Modules contribute services, hooks, and event handlers during registry construction; the host exposes the frozen registry through `ModuleContext`.
- There is no dynamic library loading, hot reload, tenant isolation, or
  production security certification.
- The kernel currently supports module API version `1.0.0`; manifests targeting a different major or a newer minor version are rejected. The kernel is experimental and its public API may change.

The standalone ORM remains independent by default. Enable the root package's optional `module-runtime` feature to access this crate as `sea_orm::modules`.

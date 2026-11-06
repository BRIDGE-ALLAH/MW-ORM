# MoonWitness ORM — Architecture Blueprint

> Status: proposed target architecture. This document is a design contract for staged implementation, not a description of already-shipped features.

## Implementation status

The first experimental `moonwitness-module` crate now provides TOML-backed typed manifests, a static module registry, deterministic dependency/capability resolution, and synchronous prepare/activate/deactivate lifecycle handling with best-effort rollback. The experimental kernel now also contains standalone typed service, hook, and in-process event registries plus checksum-validated migration planning metadata. Database migration execution, async lifecycle, unified module contribution registration, ORM integration, and production hardening remain unimplemented. See [the crate README](../moonwitness-module/README.md).

## 1. Product definition

MoonWitness is a **modular relational engine and ORM platform for Rust**.

Its architectural inspiration is the module/add-on composition model found in platforms such as Odoo: capabilities are delivered as modules, modules declare dependencies, and applications compose them into a coherent system.

MoonWitness is not an ERP suite. It provides data and module infrastructure so Rust applications can build their own domains.

### System context: MW-BAAS

MW-BAAS is a planned higher-level consumer of MoonWitness ORM. MoonWitness owns the reusable relational engine and generic module infrastructure; MW-BAAS owns BAAS product behavior and application-facing services. This separation keeps the ORM useful outside MW-BAAS and prevents product-specific concerns from leaking into the relational core. Integration should use versioned contracts and independently testable adapters.

The platform is divided into three major planes:

1. **ORM Core** — database and relational primitives.
2. **Module Kernel** — module metadata, dependency graph, registry, lifecycle, and extension contracts.
3. **Add-on Ecosystem** — optional capabilities, integrations, and application/domain modules.

## 2. High-level architecture

```text
Application Host
  |
  +-- Application / Domain Modules
  |     +-- identity
  |     +-- billing
  |     +-- inventory
  |     +-- workflow
  |
  +-- Optional Add-ons
  |     +-- audit
  |     +-- soft-delete
  |     +-- tenant-scope
  |     +-- outbox
  |     +-- search
  |     +-- framework adapters
  |
  +-- Module Kernel
  |     +-- Manifest Reader & Validator
  |     +-- Dependency Resolver (DAG)
  |     +-- Module Registry
  |     +-- Lifecycle Coordinator
  |     +-- Hook / Event Dispatcher
  |     +-- Extension Contract Registry
  |     +-- Migration Coordinator
  |
  +-- ORM Core
        +-- Entity Metadata
        +-- Relations
        +-- Query Engine
        +-- Persistence / ActiveModel
        +-- Transactions
        +-- Schema & Migration Primitives
        +-- Diagnostics / Tracing
  |
Database Adapter Layer
  +-- SQLx integration
  +-- SeaQuery integration
  +-- PostgreSQL / MySQL / SQLite drivers
```

### Dependency direction

Dependencies must point inward:

- Application modules may depend on add-ons and the module kernel.
- Add-ons may depend on the kernel contracts and ORM core.
- The kernel may depend on stable ORM core interfaces.
- ORM core must not depend on application modules or optional add-ons.

This prevents the core from becoming a dependency container for every feature.

## 3. Layer responsibilities

### 3.1 ORM Core

Owns the fundamental data model and database interaction:

- entity and column metadata;
- primary and foreign keys;
- one-to-one, one-to-many, many-to-many, and self-referential relations;
- query construction and execution;
- loading strategies;
- insert, update, delete, and nested persistence;
- transaction boundaries;
- schema and migration primitives;
- database adapter abstractions;
- query and connection diagnostics.

Core should remain usable without the module runtime. A developer who only needs an ORM must not be forced to adopt the full module platform.

### 3.2 Module Kernel

The kernel coordinates modules but should not contain their business logic.

Responsibilities:

- read and validate module manifests;
- identify module versions and compatibility ranges;
- resolve required and optional dependencies;
- detect missing dependencies and cycles;
- construct a deterministic module load order;
- build a registry of entities, services, hooks, events, and migrations;
- coordinate install, upgrade, activation, deactivation, and removal;
- expose diagnostics explaining why a module is enabled or rejected.

The first implementation should be an in-process, statically linked Rust module system. Dynamic library loading is explicitly deferred.

### 3.3 Add-ons

Add-ons are independently versioned capabilities. Examples for future exploration:

| Add-on | Possible responsibility |
|---|---|
| audit | Record selected data changes and actors |
| soft-delete | Standardize logical deletion and restore workflows |
| tenant-scope | Tenant-aware query constraints |
| outbox | Transactional outbox primitives |
| search | Search backend integration |
| cache | Cache adapters and invalidation contracts |
| observability | Metrics, tracing, and diagnostics exporters |
| framework-axum | Axum integration |
| framework-actix | Actix Web integration |

These are candidates only. Each add-on must have a clear API, threat model where relevant, tests, and explicit compatibility declaration.

## 4. Module contract

A module should declare:

- stable module identifier;
- semantic version;
- MoonWitness compatibility range;
- required and optional dependencies;
- capabilities it provides;
- capabilities it requires;
- migrations it owns;
- entities and services it registers;
- hooks and events it subscribes to.

### Proposed manifest

```toml
[module]
name = "billing"
version = "0.1.0"
description = "Billing domain module"
api = "0.1"

[compatibility]
moonwitness = ">=0.1, <0.2"

[dependencies]
required = ["identity >=0.1, <1.0"]
optional = ["audit >=0.1, <1.0"]

[capabilities]
provides = ["billing.invoices", "billing.payments"]
requires = ["identity.accounts"]
```

The manifest schema is provisional. Do not stabilize it until resolver and registry prototypes have been tested.

## 5. Module lifecycle

The target lifecycle is explicit and observable:

1. **Discover** — locate statically registered modules and read manifests.
2. **Validate** — validate identifiers, versions, compatibility, and required capabilities.
3. **Resolve** — construct a dependency DAG and reject cycles or missing requirements.
4. **Plan** — compute deterministic activation and migration order.
5. **Prepare** — construct registries and validate extension contracts without side effects.
6. **Migrate** — run module-owned migrations with recorded versions and controlled transactions.
7. **Activate** — invoke module initialization and publish the active registry.
8. **Operate** — dispatch events and invoke hooks through registered contracts.
9. **Upgrade / Deactivate / Remove** — use explicit plans, safeguards, and module-specific cleanup policies.

Failure handling must define which steps can be rolled back. Database migrations are not universally reversible; the platform must never promise automatic rollback where a migration is destructive or irreversible.

## 6. Dependency resolution

Required dependencies form a directed graph.

- Reject missing required dependencies.
- Detect and report dependency cycles.
- Validate semantic version ranges.
- Resolve optional dependencies only when compatible and available.
- Produce deterministic ordering independent of filesystem traversal order.
- Record the resolved graph for diagnostics and reproducible deployments.

The resolver should be pure where possible: same manifest set and policy should produce the same plan.

## 7. Extension model

Prefer explicit extension contracts over arbitrary monkey-patching.

### Registration

Modules register typed contributions during preparation:

- entity metadata;
- service implementations;
- named extension points;
- lifecycle hooks;
- event handlers;
- migration providers.

### Hooks

Hooks are synchronous or asynchronous contracts with defined ordering and error behavior. Avoid implicit global mutable state.

### Events

Events communicate facts that have already happened or lifecycle transitions. Event delivery semantics must be documented (in-process only initially; durable delivery belongs to a future outbox/integration layer).

### Model extensions

Do not permit two modules to silently redefine the same field or relation. Model extension should use explicit extension metadata, conflict detection, and deterministic composition rules. The first release may support registration of separate entities only; field-level extension can be deferred until its semantics are proven.

## 8. Registry

The registry is the resolved runtime view of the application:

- active module metadata;
- entity and relation metadata;
- service bindings;
- extension points;
- event subscriptions;
- migration state;
- capability providers.

The registry should be immutable after activation wherever practical. If hot reload is ever introduced, it must replace the registry atomically and define how in-flight requests and database connections behave.

## 9. Migration ownership and data safety

Each module owns a namespaced migration history. Migration identifiers must be globally unambiguous within an application.

Requirements:

- deterministic migration ordering;
- checksums to detect modified historical migrations;
- explicit forward and optional rollback operations;
- transaction support where the database supports transactional DDL;
- lock or coordination strategy to prevent concurrent migration runners;
- dry-run / plan output;
- backups and operator confirmation for destructive changes;
- no automatic module removal that silently drops user data.

Module uninstall should default to preserving data unless an explicit destructive policy is selected.

## 10. Security and tenant isolation

Security-sensitive features must be designed as first-class concerns, not merely add-ons that silently rewrite queries.

For tenant scoping:

- make tenant context explicit;
- ensure query constraints apply to reads and writes;
- cover joins, subqueries, raw SQL escape hatches, background jobs, and migrations;
- provide tests for cross-tenant access;
- document bypass APIs and restrict them to deliberate privileged contexts.

An add-on cannot claim complete tenant isolation if raw SQL or alternate query paths can bypass its policy.

## 11. Rust implementation strategy

Start with statically linked modules and typed traits. This is the safest and most idiomatic first step in Rust.

Illustrative shape only:

```rust
pub trait Module: Send + Sync {
    fn manifest(&self) -> ModuleManifest;

    fn prepare(
        &self,
        registry: &mut ModuleRegistryBuilder,
    ) -> Result<(), ModuleError>;

    async fn activate(
        &self,
        context: &ModuleContext,
    ) -> Result<(), ModuleError>;
}
```

The exact trait signatures, async strategy, object-safety requirements, and registry ownership need a prototype before becoming public API.

Do not begin with arbitrary `.so` / `.dll` loading. Rust does not provide a stable cross-version native ABI for general trait objects. If external dynamic modules become necessary, evaluate a versioned C ABI, WebAssembly component model, or subprocess boundary with explicit sandboxing and capability policy.

## 12. Suggested workspace evolution

Do not rename or move all existing source directories in one change. First map the current dependency graph and public API.

Potential future workspace:

```text
crates/
  moonwitness-core/
  moonwitness-macros/
  moonwitness-query/
  moonwitness-module/
  moonwitness-migration/
  moonwitness-codegen/
  moonwitness-arrow/

addons/
  audit/
  soft-delete/
  tenant-scope/
  outbox/

integrations/
  axum/
  actix/

examples/
  modular-app/
  custom-addon/
```

This is a target decomposition, not a statement that these crates already exist. Existing SeaORM-derived crates should be renamed only after package metadata, macro paths, feature flags, docs, tests, and licensing notices are mapped.

## 13. Architectural quality gates

A module-system milestone is not complete until it demonstrates:

- deterministic dependency resolution;
- useful cycle and missing-dependency diagnostics;
- duplicate identifier and extension conflict detection;
- repeatable module registration;
- migration ordering and checksum validation;
- safe failure behavior during activation;
- integration tests with at least two dependent modules;
- no regression in the standalone ORM use case;
- documentation of compatibility and lifecycle guarantees.

## 14. Non-goals and deferred decisions

- No ERP business suite in the ORM repository.
- No implicit global service locator as the only extension mechanism.
- No arbitrary runtime code execution in the initial kernel.
- No native dynamic library loading in the initial release.
- No automatic destructive data removal on uninstall.
- No stable manifest or public module API until prototype feedback is available.

## 15. Decision summary

The defining MoonWitness architecture is:

**A relational core + a typed module kernel + independently versioned add-ons.**

The ORM remains a useful standalone library. The module runtime is an optional higher layer. Add-ons extend capabilities through explicit contracts, dependency declarations, lifecycle rules, and migration ownership.

That is how MoonWitness can adopt the modular power of an Odoo-like system while remaining Rust-native, composable, testable, and maintainable.

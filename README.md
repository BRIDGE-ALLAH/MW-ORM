# 🌙 MoonWitness ORM

### Beyond Data. Witness Every Relation.
**A modular relational engine for Rust.**

MoonWitness ORM is an independently evolving Rust data layer built around a modular architecture: a stable relational core, a module runtime, and composable add-ons.

The long-term direction takes inspiration from the extensibility model of systems such as Odoo: applications are assembled from focused modules that declare dependencies, contribute models and services, participate in lifecycle operations, and extend other modules through explicit contracts.

MoonWitness is **not an ERP system** and does not aim to reproduce Odoo's business applications. It applies the modular-platform idea to Rust data engineering.

> **Core first. Modules by contract. Add-ons without a tangled core.**

---

## The idea

Traditional ORM projects often grow by accumulating features inside one crate or by asking every application to build its own integration layer. MoonWitness aims for a different shape:

- a **Core** that owns database connectivity, entity metadata, queries, relations, transactions, and persistence;
- a **Module System** that discovers and validates modules, resolves dependencies, builds a registry, and coordinates lifecycle operations;
- **Add-ons** that contribute optional capabilities without forcing every application to depend on every feature;
- **Application Modules** that compose domain-specific models and services on top of the lower layers.

This separation is the central architectural commitment of MoonWitness.

## Architecture at a glance

```text
                    APPLICATION
                         |
              Application / Domain Modules
                         |
          +--------------+---------------+
          |              |               |
       Add-ons       Integrations     Custom Modules
          |              |               |
          +--------------+---------------+
                         |
                 MODULE RUNTIME
       Manifest · Registry · Dependencies · Lifecycle
       Hooks · Events · Extension Contracts · Migrations
                         |
                  ORM CORE
       Entities · Relations · Query · Persistence
       Transactions · Schema · Diagnostics
                         |
                DATABASE ADAPTERS
             SQLx · SeaQuery · Drivers
                         |
          PostgreSQL · MySQL · SQLite · ...
```

The diagram describes the **target architecture**, not a claim that every layer is already implemented.

Read the [Architecture Blueprint](docs/ARCHITECTURE.md) and [Implementation Blueprint](docs/BLUEPRINT.md) for the design and phased delivery plan.

## Design principles

1. **Modularity by contract** — modules declare what they provide and require.
2. **Explicit dependencies** — dependency resolution is deterministic and validated before startup.
3. **Stable core, optional capabilities** — add-ons extend the platform without bloating the core.
4. **Composition over hidden magic** — extensions and hooks are explicit, inspectable, and testable.
5. **Rust-native safety** — prefer compile-time composition and typed interfaces before dynamic loading.
6. **Predictable lifecycle** — installation, migration, activation, upgrade, and removal have defined semantics.
7. **Observable behavior** — module graph, migration history, query activity, and lifecycle failures should be diagnosable.
8. **Compatibility with discipline** — public API changes require tests, documentation, and migration guidance.

## The module and add-on model

A MoonWitness application is intended to be assembled from modules. A module may:

- declare a unique name, version, and compatibility range;
- declare required and optional module dependencies;
- register entities and relational metadata;
- register services and extension points;
- contribute migrations and seed data;
- subscribe to typed lifecycle events;
- expose capabilities to other modules through documented contracts.

An add-on is an optional module that extends a capability without requiring the core to absorb that feature. Examples of possible future add-ons include audit trails, soft deletion, tenant scoping, outbox patterns, search adapters, and framework integrations. These are **design candidates**, not current feature claims.

A future manifest may look like this:

```toml
# moonwitness.module.toml — proposed format
[module]
name = "billing"
version = "0.1.0"
description = "Billing domain module"

[dependencies]
required = ["identity >=0.1, <1.0"]
optional = ["audit >=0.1, <1.0"]

[capabilities]
provides = ["billing.invoices", "billing.payments"]
requires = ["identity.accounts"]
```

The manifest format is a proposal and may change before implementation.

## Technology foundation

MoonWitness begins its independent evolution from the existing SeaORM 2.0 codebase in this repository. The current workspace includes the main ORM crate and companion crates for macros, code generation, and Arrow integration. The existing implementation uses Rust, SeaQuery, SQLx-related adapters, and other established Rust ecosystem components.

The project will preserve useful upstream functionality while progressively developing its own module contracts, APIs, package identity, and tooling. The current source still contains upstream SeaORM package names and references; crate renaming and architectural extraction are planned work, not completed work.

## Current status

**Independent evolution / architecture definition.**

- [x] Direction selected: Independent Evolution
- [x] Modular platform architecture proposed
- [x] Core, module runtime, and add-on boundaries documented
- [ ] Repository-wide upstream and license audit
- [ ] Baseline CI and compatibility inventory
- [ ] Module manifest and registry prototype
- [ ] Dependency resolver and lifecycle prototype
- [ ] First reference add-on
- [ ] MoonWitness crate namespace transition

Until the relevant implementation and tests land, module runtime capabilities described in the blueprint should be treated as planned architecture.

## Roadmap

| Phase | Focus | Exit criteria |
|---|---|---|
| 0 | Foundation audit | Workspace, APIs, features, licenses, and CI mapped |
| 1 | Identity and contracts | Vision, architecture, module boundaries, contribution rules |
| 2 | Module kernel prototype | Manifest, registry, dependency graph, deterministic startup |
| 3 | Extension and lifecycle | Hooks, events, migrations, activation and upgrade rules |
| 4 | First add-ons | Reference modules with integration tests and compatibility policy |
| 5 | Independent ecosystem | CLI, codegen, adapters, documentation, release process |

See [BLUEPRINT.md](docs/BLUEPRINT.md) for detailed work packages and acceptance criteria.

## Non-goals

- Rebuilding an ERP suite or copying Odoo business modules.
- Turning every optional feature into a mandatory core dependency.
- Loading arbitrary native dynamic libraries before a safe compatibility and security model exists.
- Claiming production readiness for planned module features.
- Breaking upstream compatibility merely to change names.

## Contributing

The project welcomes discussion and contributions around Rust ORM design, modular architecture, database engineering, and developer tooling.

Before implementation begins, significant architectural changes should be discussed through an issue or design proposal. Contributions should include focused tests, documentation, and compatibility notes where applicable.

## Licensing and upstream

MoonWitness is evolving from the SeaORM codebase. Existing source files, notices, and third-party dependencies retain their applicable copyright and license obligations. The repository currently includes MIT and Apache-2.0 license texts; contributors and redistributors should review the relevant notices and third-party licenses.

MoonWitness acknowledges the work of the SeaQL / SeaORM project and its contributors. Independent branding does not erase upstream provenance or licensing requirements.

## The MoonWitness principle

**Model the domain. Connect the modules. Preserve the integrity.**

MoonWitness is being built to make relational data foundations composable: a dependable core, clear module contracts, and add-ons that evolve independently.

---

*MoonWitness ORM — Beyond Data. Witness Every Relation.*

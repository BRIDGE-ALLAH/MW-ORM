# MoonWitness ORM — Implementation Blueprint

> This is the delivery plan for the modular architecture. All module-system features below are proposals until implemented and tested.

## North-star outcome

A Rust application can compose a set of independently versioned modules and add-ons. MoonWitness validates their compatibility, resolves their dependency graph, registers their contributions, coordinates migrations, and starts the application in a deterministic and diagnosable way.

The standalone ORM remains independently usable.

## Workstream A — Repository and upstream foundation

### A1. Inventory
- Map workspace members and crate dependency graph.
- Inventory public traits, types, macros, feature flags, and re-exports.
- Record database backend and runtime combinations.
- Identify upstream-specific URLs, names, docs, badges, examples, and generated code.
- Record source-level copyright and third-party notices.

### A2. Baseline
- Run formatting, clippy, unit tests, integration tests, and feature-matrix CI.
- Capture current compile times and benchmark baselines where practical.
- Record known failures and environmental requirements.

### A3. Upstream policy
- Define how upstream changes are tracked and reviewed.
- Keep a provenance record for inherited and MoonWitness-authored changes.
- Define a security update process.
- Preserve applicable upstream licenses and notices.

**Exit criteria:** reproducible baseline report and dependency map.

## Workstream B — Product and package identity

- Adopt the product name MoonWitness ORM.
- Use the positioning: “A modular relational engine for Rust.”
- Keep repository name MW-ORM unless maintainers choose otherwise.
- Define crate namespace migration as a separate compatibility project.
- Do not change every package name before macro and downstream compatibility tests exist.
- Publish vision, architecture, roadmap, contribution, and security documents.

**Exit criteria:** consistent project-facing identity without misleading package or feature claims.

## Workstream C — Module kernel MVP

**Status: initial experimental implementation landed in `moonwitness-module`.** The current crate includes typed manifests, TOML parsing, required/optional dependencies, semver checks, capability provider resolution, deterministic activation plans, a static registry, synchronous lifecycle, typed service lookup, ordered hooks, typed in-process events, and SHA-256 migration descriptors with checksum-validated planning. It is a prototype, not a stable public API. Database migration execution and ORM-specific adapters are still not implemented. Both synchronous and asynchronous lifecycle hosts now collect module contributions into a single immutable extension registry.

Build the first kernel as an optional crate, tentatively `moonwitness-module`.

### C1. Module descriptor — initial implementation

Implemented as `ModuleManifest` and `ModuleDependency`; TOML parsing and basic validation are available. The manifest/API compatibility policy still needs formalization.
Initial fields:
- module identifier;
- semantic version;
- module API version;
- MoonWitness compatibility range;
- required and optional dependencies;
- provided and required capabilities.

Start with Rust-defined descriptors or a small validated manifest format. Avoid supporting multiple competing formats.

### C2. Registry builder — initial implementation
- Register modules by stable identifier.
- Reject duplicate identifiers.
- Register entities and service contracts.
- Detect duplicate contribution names.
- Freeze a validated registry before application activation.

### C3. Dependency resolver — initial implementation
- Build a directed graph.
- Validate required dependencies and version ranges.
- Detect cycles with actionable diagnostics.
- Resolve optional dependencies deterministically.
- Return an ordered activation plan.

### C4. Static module loading and lifecycle — initial implementation
- Modules are linked into the host binary.
- No arbitrary runtime code loading.
- Provide a simple application builder that receives module instances.

**Exit criteria:** integration test with a host and at least two dependent modules; stable deterministic plan; clear failure diagnostics.

## Workstream D — Lifecycle and migrations

**Partial implementation:** the kernel now creates namespaced migration descriptors, calculates SHA-256 checksums, detects duplicate ids/versions, rejects missing historical migrations and checksum drift, and creates deterministic pending plans. The generic async `MigrationRunner` now coordinates backend lock acquisition, history validation, ordered apply-and-record calls, and lock release. The actual database backend must still implement SQL execution, real locks, transactions, and persistent history.

Define lifecycle states and transitions:

`Discovered -> Validated -> Planned -> Prepared -> Migrated -> Active`

Failure and deactivation states must be explicit.

Implement:
- prepare phase without external side effects;
- migration plan generation;
- module-scoped migration identifiers;
- migration checksums;
- migration history;
- database locking or single-runner coordination;
- dry-run output;
- explicit destructive-operation confirmation;
- uninstall policy that preserves data by default.

**Exit criteria:** repeatable install/upgrade test, failed migration test, checksum mismatch test, and no silent data deletion.

## Workstream E — Extension contracts

**Initial implementation:** modules can contribute typed services, ordered hooks, and typed in-process event handlers during registry construction. Contributions are frozen in dependency order and exposed through the lifecycle context. Delivery is synchronous and in-process only.

Start with the least magical, most testable extension types:

1. service registration;
2. lifecycle hooks;
3. typed in-process events;
4. entity metadata registration.

Defer field-level model mutation until conflict semantics, schema implications, code generation, and compatibility are understood.

Every extension contract needs:
- owner and identifier;
- input/output types;
- ordering rules;
- error behavior;
- compatibility policy;
- observability.

**Exit criteria:** two add-ons extend one declared contract without hidden global state or order-dependent behavior.

## Workstream F — First reference add-ons

**Initial example delivered:** `moonwitness-addon-audit` demonstrates registering an audit store service and subscribing to typed record-change events. It is intentionally in-memory and does not automatically intercept ORM writes; durable storage and transaction-after-commit integration remain open.

Choose one low-risk add-on first. Recommended sequence:

1. **Audit metadata** — explicit audit context and event records; carefully define actor and data capture.
2. **Soft delete** — explicit query scope and restore semantics.
3. **Outbox primitives** — transactionally persisted event records.
4. **Tenant scope** — only after security review and bypass-path testing.

Do not start with tenant isolation as the first add-on: it affects every query path, raw SQL, joins, background jobs, and operational workflows.

**Exit criteria:** add-on has its own package, tests, documentation, compatibility declaration, and example host application.

## Workstream G — MW-BAAS integration boundary

- Define the exact dependency direction: MW-BAAS consumes MoonWitness; MoonWitness must not depend on MW-BAAS.
- Publish versioned contracts for module registration, lifecycle participation, migrations, and shared data capabilities.
- Build a small reference integration that proves MW-BAAS can compose MoonWitness modules without coupling BAAS product logic into the ORM core.
- Add compatibility tests across supported MoonWitness and MW-BAAS versions.

## Workstream H — Developer tooling

After kernel and module contract stabilize:
- module graph inspection command;
- dependency and compatibility validation;
- migration plan and status command;
- module scaffolding;
- entity/code generation integration;
- diagnostics export.

Do not build a large CLI before the underlying APIs stabilize.

## Suggested release milestones

| Milestone | Scope | Release posture |
|---|---|---|
| M0 | Audit and identity docs | Documentation milestone |
| M1 | Module descriptor and registry prototype | Experimental |
| M2 | Resolver and static module loading | Experimental |
| M3 | Lifecycle and migration coordinator | Experimental |
| M4 | Extension contracts and first add-on | Alpha |
| M5 | Compatibility, security, and operational hardening | Beta candidate |
| M6 | Stable module API and release policy | Stable only after quality gates |

No dates are assigned until the baseline audit establishes project capacity.

## Testing strategy

### Unit tests
- manifest validation;
- semantic version constraints;
- dependency graph ordering;
- cycle detection;
- duplicate registration;
- lifecycle state transitions;
- migration checksum validation.

### Integration tests
- application with no modules (standalone ORM);
- one module;
- multiple dependent modules;
- optional dependency absent/present;
- module activation failure;
- migration failure and retry;
- two modules contributing to one extension contract.

### Security tests
- capability misuse;
- tenant-scope bypass paths (when implemented);
- raw SQL behavior;
- migration privilege boundaries;
- malicious or malformed manifest input.

### Compatibility tests
- supported Rust toolchain;
- supported database backends;
- feature combinations;
- macro expansion;
- downstream crate compilation.

## Decision log

| Decision | Current position |
|---|---|
| Product shape | Modular ORM platform |
| Inspiration | Odoo-style module/add-on composition |
| ERP scope | Explicitly out of scope |
| Core | Standalone and lean |
| Module runtime | Optional higher layer |
| Initial loading | Statically linked Rust modules |
| Dynamic loading | Deferred |
| Manifest | Proposed, not stable |
| Model field monkey-patching | Deferred |
| Uninstall data policy | Preserve data by default |
| Crate renaming | Separate staged migration |

## Immediate next actions

1. Complete source and workspace audit.
2. Map current crate boundaries and public API.
3. Establish baseline CI results.
4. Extend the `moonwitness-module` prototype with async lifecycle and ORM-facing contracts outside the ORM core.
5. Review prototype ergonomics before committing to public traits.
6. Build one reference add-on and prove lifecycle/migration behavior.

**Definition of done:** the modular design is demonstrated by executable tests and a sample application, not merely by diagrams or README claims.

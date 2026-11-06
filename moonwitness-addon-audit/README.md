# moonwitness-addon-audit

A small reference add-on demonstrating module-owned service and event
contributions using `moonwitness-module`.

## Behavior

- Registers an `AuditStore` service.
- Subscribes to the typed `RecordChanged` event.
- Captures entity, record key, operation, and optional actor in an in-memory
  store.

## Important boundaries

This is a reference integration, not a production audit subsystem. The
application must publish `RecordChanged` only after the corresponding database
transaction commits. The store is in-memory and is lost on process restart.
The add-on does not intercept SeaORM writes, guarantee durable delivery, or
provide tamper-evident audit storage. A persistent adapter and transaction
integration are required before production use.

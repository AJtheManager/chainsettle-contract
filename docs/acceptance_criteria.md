# Per-Milestone Acceptance Criteria Hash

## Overview

Each milestone in a shipment can carry an optional **acceptance-criteria hash** — a
32-byte digest of the document that defines what "done" means for that milestone.
The hash is anchored at milestone creation time so that, in the event of a dispute,
arbiters have an immutable reference to the agreed-upon acceptance criteria.

## Storage

The hash is stored under its own storage key rather than being added to the
`ShipmentOptions` / `Shipment` structs (which are at their field limit). The key is
scoped per shipment and per milestone index:

```
DataKey::AcceptanceHash(shipment_id, milestone_index) -> BytesN<32>
```

Persistent entries have their TTL extended on write via
`extend_ttl(TTL_INITIAL_LEDGERS, TTL_MAX_LEDGERS)`, and the per-shipment key is
registered in `extend_shipment_related_keys_ttl` so it is kept alive alongside the
rest of the shipment's data.

## Behaviour

- **Optional at creation.** A milestone may be created with or without an
  acceptance-criteria hash. When omitted, no entry is written.
- **Readable.** `get_acceptance_criteria(shipment_id, milestone_index)` returns the
  stored hash (or `None` when none was set).
- **Immutable after creation.** Once written, the hash cannot be changed by ordinary
  milestone operations.
- **Amendable with consent.** The only way to change the hash after creation is
  through the amendment flow, which requires the consent of the relevant parties.

## Authorization

Every new function validates its inputs and enforces caller authorization with
`require_auth`. State-changing calls emit events and write to the shipment audit log
where applicable.

## API

### `get_acceptance_criteria(shipment_id, milestone_index) -> Option<BytesN<32>>`

Returns the acceptance-criteria hash recorded for the given milestone, or `None` if
none was set. Validates the shipment and milestone index and requires the caller to
be authorized.

## Testing

Unit tests live in a dedicated `test_acceptance_criteria.rs` module (registered in
`lib.rs`) and cover:

- the happy path (hash stored and read back),
- authorization failures,
- edge cases (missing hash, out-of-range milestone index, amendment consent).

## Notes

- Soroban limits contract function names and `Symbol::new` event topics to 32
  characters; all new names stay within that limit.
- `DataKeyExt4` has only a few free slots; once it is full a new `DataKeyExt5` enum
  should be introduced following the existing pattern.

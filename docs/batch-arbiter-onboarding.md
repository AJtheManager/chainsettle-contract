# Batch Arbiter Onboarding (#663)

`batch_add_arbiters_to_pool` adds a whole cohort of arbiters to the
admin-managed arbiter pool in one transaction, instead of one
`add_arbiter_to_pool` call per arbiter.

## Function

```rust
batch_add_arbiters_to_pool(admin: Address, arbiters: Vec<Address>)
```

## Authorization

`admin.require_auth()` is required, and `admin` must hold the
`ArbiterManager` role. The contract admin holds every role.

## Validation

The contract validates every address before it changes the pool:

| Condition | Panic |
|---|---|
| `arbiters` is empty | `batch is empty` |
| more than `MAX_BATCH_ADD_ARBITERS` (20) addresses | `batch too large` |
| an address appears twice in the batch | `duplicate arbiter in batch` |
| an address is already in the pool | `arbiter already in pool` |
| an address is blacklisted | `arbiter is blacklisted` |
| an address is slashed (#372) | `arbiter is slashed and must be reinstated before re-adding` |

The single-arbiter `add_arbiter_to_pool` silently ignores an address that is
already in the pool. The batch version rejects it instead, so a cohort that
overlaps the existing pool can't be half-applied without anyone noticing.

## Atomicity

Either all of the arbiters are added or none are. A single invalid address
reverts the whole batch, and the pool is left unchanged.

## Effects

- The arbiters are appended to the pool in the order given.
- One `batch_add_arbiters_to_pool` entry is written to the admin action log.
- Per arbiter, `("arbiter_pool_updated",)` is emitted with data
  `("added", arbiter)`, the same event as `add_arbiter_to_pool`.
- One summary `("arbiters_batch_added",)` event is emitted with data
  `(added_count: u32, pool_size: u32)`.

Pool membership is global, not per shipment, so this function writes no
shipment audit log entry.

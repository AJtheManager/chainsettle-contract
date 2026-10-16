# Batch Proof Submission (#666)

In parallel-mode shipments several milestones are often ready at the same
time. `batch_submit_proofs` lets the submitter send all of their proofs in one
transaction instead of one `submit_proof` call per milestone.

## Function

```rust
batch_submit_proofs(
    caller: Address,
    shipment_id: String,
    entries: Vec<(u32, String, Symbol)>, // (milestone_index, proof_hash, proof_type)
)
```

## Validation

`caller.require_auth()` runs once for the whole batch. Then the batch is
rejected if:

| Condition | Panic |
|---|---|
| `entries` is empty | `batch is empty` |
| more than `MAX_BATCH_PROOF_SUBMISSIONS` (20) entries | `batch too large` |
| the same `milestone_index` appears twice | `duplicate milestone index in batch` |

Each entry then goes through the same checks as `submit_proof`:

- the contract and the shipment are not paused, and the shipment is `Active`
- the milestone index is valid and the milestone is `Pending`
- `caller` is the designated proof submitter for that milestone (#544; by
  default the supplier, or logistics when no explicit submitters are set)
- the proof hash is within the configured length and prefix bounds (#405)
- `proof_type` is on the milestone's proof-type whitelist, if one is set
- in `Sequential` mode, the previous milestone is `Confirmed` or `Resolved`
- the per-milestone evidence cap has not been reached
- the dual-attestation rules (#545) are met, if enabled

Entries are applied in order. A batch can't skip ahead in sequential mode: a
batch of `[0, 1]` fails because milestone 0 is only `ProofSubmitted`, not
`Confirmed`.

## Atomicity

The batch is all-or-nothing. If any entry fails validation, the contract
panics and the whole transaction reverts, including the entries before it that
had already passed.

## Effects

Each entry has exactly the effects of `submit_proof`: it updates the milestone
status and proof hash, records the submission ledger, evidence count, proof
type and submitter, adds a `proof_submitted` audit entry, and emits the
`proof_submitted` / `proof_resubmitted` and canonical `MilestoneProofSubmitted`
events.

When every entry has been applied, the batch also:

- adds one `batch_proofs_submitted` entry to the shipment audit log
- emits `("batch_proofs_submitted", shipment_id)` with data
  `(milestone_indices: Vec<u32>, caller: Address)`

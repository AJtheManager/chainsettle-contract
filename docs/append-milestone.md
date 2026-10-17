# Appending a Milestone by Mutual Consent

Scope often grows while a shipment is in progress, for example an extra
inspection or an additional delivery. Before this feature, extra work meant
creating a new shipment. Now the buyer and supplier can agree to append a new
milestone to an active shipment, and the buyer funds it with a top-up.

## Functions

| Function | Caller | Description |
|---|---|---|
| `propose_new_milestone(caller, shipment_id, name, amount)` | Buyer or supplier | Propose a new final milestone worth `amount`. |
| `accept_new_milestone(caller, shipment_id)` | The other party | Accept, fund and append the milestone. |
| `cancel_new_milestone(caller, shipment_id)` | Buyer or supplier | Withdraw or reject the pending proposal. |
| `get_new_milestone_proposal(shipment_id) -> Option<NewMilestoneProposal>` | Anyone | The pending proposal, if any. |

```rust
pub struct NewMilestoneProposal {
    pub proposer: Address,
    pub name: String,
    pub amount: i128,
    /// Buyer whose deposit is already held for this proposal (None = funded on acceptance).
    pub funded_by: Option<Address>,
    /// Escrow token, kept so a deposit stays refundable.
    pub token: Address,
    pub proposed_at_ledger: u32,
}
```

## Flow

1. Either party calls `propose_new_milestone`. A shipment can have only one
   pending proposal at a time.
2. The counterparty calls `accept_new_milestone`. A buyer accepts a supplier's
   proposal, and the supplier accepts a buyer's. The proposer can't accept
   their own proposal, and a co-buyer can't accept another buyer's proposal.
3. On acceptance the milestone is appended as the last milestone, in
   `Pending` status, and `total_amount` grows by exactly `amount`.

### Funding

The buyer always pays for the new milestone:

- **Supplier proposes**: the accepting buyer transfers `amount` into escrow
  when they accept.
- **Buyer proposes**: the buyer's `amount` is transferred into the contract
  when they propose, because the supplier's acceptance can't move the buyer's
  tokens. It is held as a deposit and only becomes escrow
  (`total_amount`, TVL) on acceptance. If the proposal is cancelled or
  rejected, the deposit is refunded in full. `cancel_new_milestone` works even
  after the shipment is no longer active, so the deposit can always be
  recovered.

On acceptance, the same caps as `top_up_escrow` apply: the supplier exposure
cap, the token TVL cap, and the buyer's rolling spending limit. The spending
limit is checked when the buyer's tokens move, which is either at proposal or
at acceptance.

### Re-deriving the payout weights

Milestone payouts are `total_amount × weight`. Without a change, raising
`total_amount` would also inflate every existing milestone. On acceptance,
the contract re-derives the weights so each existing milestone keeps its
absolute value:

- Each existing milestone's current gross value `g_i` is computed from the old
  total and weights.
- New basis-point weights are stored in `MilestoneSplits` (these take
  precedence for payouts): `ceil(g_i × 10_000 / new_total)` for existing
  milestones, and the remainder up to 10 000 for the new milestone.
  Rounding up means an existing milestone is never worth less than before.
  Any rounding loss, at most about 0.01% of the new total per milestone, is
  absorbed by the new milestone.
- `payment_percent` is re-derived from those weights (`bps / 100`, with the
  remainder on the new milestone) so the percentages still sum to 100.

Example: a 1 000 000 shipment split 25/50/25 plus a new 250 000 milestone
becomes a 1 250 000 shipment split 20/40/20/20, and the existing milestones
are still worth 250 000 / 500 000 / 250 000.

Already-released milestones are re-weighted too. Their weight doesn't move
funds, but it keeps the weights summing to 10 000.

### Per-milestone configuration

The new milestone gets neutral defaults for per-milestone options:

- timestamp deadline `0` (none)
- expected quantity `0` (not quantity-based)
- designated proof submitter: the supplier, when explicit submitters are
  configured
- not inspected
- paid to the main supplier on consortium shipments

## Validation

| Condition | Panic |
|---|---|
| `amount <= 0` | `milestone amount must be positive` |
| empty `name` | `milestone name must not be empty` |
| amount too small to receive a non-zero weight | `milestone amount too small` |
| a proposal is already pending | `new milestone proposal already pending` |
| milestone count would exceed `MaxMilestoneCount` (checked on propose and accept) | `TooManyMilestones` |
| shipment not active (propose / accept) | `shipment is not active` |
| no pending proposal | `no pending new milestone proposal` |
| wrong party accepts | `milestone must be accepted by the counterparty` |
| caller not a buyer or the supplier | `unauthorized` |

Every function requires the caller's signature (`require_auth`).

## Events and audit log

| Event | Topics | Data |
|---|---|---|
| `new_milestone_proposed` | `(name, shipment_id)` | `(proposer, milestone name, amount)` |
| `new_milestone_accepted` | `(name, shipment_id)` | `(acceptor, new milestone index, amount, new total_amount)` |
| `new_milestone_cancelled` | `(name, shipment_id)` | `(caller, refunded deposit)` |

Shipment audit-log actions: `new_ms_proposed`, `milestone_appended` and
`new_ms_cancelled`. An audit entry is only written while the shipment record
still exists.

## Storage

| Key | Storage | Value |
|---|---|---|
| `DataKeyExt4::NewMilestoneProposal(String)` | persistent | `NewMilestoneProposal` |

The key is written with `extend_ttl(TTL_INITIAL_LEDGERS, TTL_MAX_LEDGERS)` and
is included in `extend_shipment_related_keys_ttl`.

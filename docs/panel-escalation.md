# Automatic Arbiter-Panel Escalation (#665)

High-value disputes deserve more scrutiny than a single arbiter provides. With
panel escalation enabled, a dispute above an admin-set value threshold is
automatically assigned to a multi-arbiter panel drawn from the arbiter pool.

## Admin configuration

```rust
set_panel_escalation(admin: Address, token: Address, threshold: i128, panel_size: u32)
clear_panel_escalation(admin: Address, token: Address)
get_panel_escalation(token: Address) -> Option<(i128, u32)>   // (threshold, panel_size)
is_dispute_panel_escalated(shipment_id: String, milestone_index: u32) -> bool
```

- `admin.require_auth()` is required, and `admin` must be the contract admin.
- `threshold` must be positive, and `panel_size` must be at least
  `MIN_ESCALATION_PANEL_SIZE` (3), the same minimum as `assign_dispute_panel`.
- The configuration is per escrow token, because amounts are only comparable
  within one token. Tokens without a configuration never escalate.
- Each change writes an admin action log entry and emits
  `("panel_escalation_set", token) → (threshold, panel_size)` or
  `("panel_escalation_cleared", token) → ()`.

## When a dispute escalates

When a dispute is raised, the contract compares the **disputed amount** with
the threshold for the shipment's token:

- for `raise_dispute`, the milestone's full value
- for `raise_partial_dispute`, only the contested share. The uncontested share
  has already been paid.

The panel is assigned only if the disputed amount is **strictly greater than**
the threshold. At or below the threshold, the dispute works exactly as before,
with the shipment's single arbiter. Supplier-raised quality-grade challenges
(#519) never escalate.

## Panel selection

Panel selection uses the same method as `assign_dispute_panel`: `panel_size`
arbiters are drawn pseudo-randomly from the pool. The draw:

- never picks the shipment's buyers or supplier
- skips arbiters who have marked themselves unavailable
- applies the #470 repetition guard. Recent arbiters for the supplier are
  avoided while the pool has alternatives.

The panel replaces the shipment's `ArbiterPanel`, any leftover votes for the
milestone are cleared, and the chosen arbiters are recorded in the supplier's
recent-arbiter history.

## Fallback when the pool is too small

If the pool cannot supply `panel_size` eligible arbiters, the dispute **falls
back to the shipment's single arbiter**. The dispute is still raised and
nothing reverts, so a thin pool can never block a buyer from disputing. The
fallback is visible on chain through:

- a `panel_escalation_fallback` entry in the shipment audit log
- the event `("panel_escalation_fallback", shipment_id)` with data
  `(milestone_index, disputed_amount, eligible_count, panel_size)`

## Resolving an escalated dispute

While a dispute is escalated, `is_dispute_panel_escalated` returns `true` and
the single arbiter is locked out. `resolve_dispute`, `batch_resolve_disputes`
and `resolve_dispute_split` panic with `dispute escalated to arbiter panel`.
Panel members vote with `cast_dispute_vote`, and a simple majority resolves the
dispute.

The escalation flag is cleared when:

- the panel reaches a majority
- the dispute is appealed, because an appeal is heard by the single arbiter
  that `appeal_dispute` reassigns from the pool
- a new dispute is raised on the same milestone. The flag is then set again
  only if the new dispute is also above the threshold.

## Events and audit log

On escalation:

- shipment audit log entry `dispute_panel_escalated`
- `("dispute_panel_escalated", shipment_id)` with data
  `(milestone_index, disputed_amount, threshold, panel: Vec<Address>)`
- `("dispute_panel_assigned", shipment_id)` with data
  `(milestone_index, panel)`, the same event as manual `assign_dispute_panel`

## Storage

| Key | Contents |
|---|---|
| `DataKeyExt4::PanelEscalation(token)` | `(threshold, panel_size)` |
| `DataKeyExt4::PanelEscalated(shipment_id, milestone_index)` | `true` while escalated |
| `DataKeyExt::ArbiterPanel(shipment_id)` | assigned panel (existing key) |

All three keys are persistent and have their TTL extended when written.
`PanelEscalated` is also refreshed by `extend_shipment_related_keys_ttl` and
moves with its milestone when milestones are merged.

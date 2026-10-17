# Percentage Split Dispute Resolution (#664)

Real disputes are rarely all-or-nothing. `resolve_dispute_split` lets the
arbiter divide the contested amount between the supplier and the buyer by
percentage, instead of picking a winner as `resolve_dispute` does.

## Function

```rust
resolve_dispute_split(
    arbiter: Address,
    shipment_id: String,
    milestone_index: u32,
    supplier_bps: u32, // supplier's share, 0..=10_000
)

get_dispute_split(shipment_id: String, milestone_index: u32) -> Option<u32>
```

## Authorization and validation

- `arbiter.require_auth()`, and `arbiter` must be the shipment's current
  arbiter.
- `supplier_bps` must be at most `10_000`. Both `0` and `10_000` are accepted.
- The shipment must be `Active` and the milestone `Disputed`.
- These disputes can't be split:
  - a dispute escalated to an arbiter panel (#665), which the panel decides
    through `cast_dispute_vote`
  - a supplier-raised quality-grade challenge (#519)
  - a milestone with an approved advance, because the supplier already holds
    part of the funds
- The `dispute` operation must not be paused.

## Payout amounts

The **contested amount** is:

- for a full dispute (`raise_dispute`), the milestone's full value
- for a partial dispute (`raise_partial_dispute`), the contested percentage of
  the milestone's value. The uncontested share was already paid out when the
  dispute was raised.

```
supplier_share = contested * supplier_bps / 10_000   (rounded down)
buyer_share    = contested - supplier_share
```

`supplier_share + buyer_share` always equals the contested amount exactly. Any
rounding remainder goes to the buyer.

The following come out of the gross shares:

| Item | Rule |
|---|---|
| Arbiter fee | Calculated on the whole contested amount (fee tiers apply). The supplier pays `arbiter_fee * supplier_bps / 10_000` and the buyer pays the rest. |
| Platform fee | Applies to `supplier_share` only, as on any supplier payout. Buyer refunds carry no platform fee. |
| Milestone payees | The supplier's net amount is distributed through the configured milestone payees, as in `resolve_dispute`. |

Loser-pays (`LoserPaysArbiterFee`) does not apply to a split, because there is
no single loser. Instead, both parties pay the arbiter fee in proportion to
their shares.

## Dispute bond

The buyer's dispute bond is returned in proportion to the share of the dispute
they won:

```
bond_to_buyer    = bond * (10_000 - supplier_bps) / 10_000
bond_to_supplier = bond - bond_to_buyer
```

## Milestone outcome

A split always ends with the milestone `Resolved`. Unlike a buyer win under
`resolve_dispute`, it never resets the milestone to `Pending` for resubmission.
If this was the last open milestone, the shipment completes as usual.

## Majority rule for bookkeeping

Some existing mechanisms assume a binary winner. For these, a split counts as
a **supplier win when `supplier_bps > 5000`** and as a **buyer win otherwise**.
An even 50/50 split counts as a buyer win, because the buyer's dispute was at
least half upheld. The rule applies to:

- `ArbiterStats.resolved_approved` / `resolved_rejected`
- the dispute-history outcome (#577)
- buyer reliability tracking
- appeal-fee settlement (an appeal "succeeds" if the appeal ruling favours the
  appellant)
- overturn detection (#372 / #531). An appeal overturns the original arbiter
  if it flips the favoured side, for example a 30% split overturned by an 80%
  split, or by a binary `resolve_dispute` that favours the supplier.

Supplier reputation is not changed beyond the dispute already counted when
the dispute was raised.

## Appeals and finality

The existing appeal flow (`appeal_dispute`) reopens a dispute's decision
without reversing a payout that was already made. If a split paid out at once
and was then appealed, the contested amount would be paid a second time. To
prevent that, a split **holds the funds in escrow while it can still be
appealed**:

- If an appeal window is configured (`set_appeal_window_ledgers`) and the
  dispute has not been appealed yet, or a resolution finality delay (#393) is
  configured, the milestone is parked in `ResolvedPendingFinality`. Nothing is
  paid. The park lasts `max(finality_delay, appeal_window + 1)` ledgers. The
  `+ 1` stops a payout from landing on the last ledger on which an appeal is
  still accepted.
- **Within the window**, either party may call `appeal_dispute`. The milestone
  goes back to `Disputed` with the funds still in escrow, and the reassigned
  appeal arbiter decides again with either `resolve_dispute` or
  `resolve_dispute_split`. A partial dispute keeps its contested percentage.
- **After the window**, anyone may call `finalize_dispute_resolution`. It pays
  out the stored split, and the milestone becomes `Resolved`.
- A split given **on appeal** is final. It pays out immediately unless a
  finality delay is configured.

When neither an appeal window nor a finality delay is configured, the split
pays out immediately.

## Events and audit log

| Event | Topics | Data |
|---|---|---|
| `dispute_split_resolved` | `(name, shipment_id)` | `(milestone_index, supplier_bps, supplier_share, buyer_share, is_partial, parked)` |
| `dispute_split_settled` | `(name, shipment_id)` | `(milestone_index, supplier_share, buyer_share, supplier_net, buyer_net, arbiter_fee, platform_fee)`, emitted when funds actually move |
| `split_resolution_finalized` | `(name, shipment_id)` | `(milestone_index, supplier_bps, contested)`, emitted by `finalize_dispute_resolution` for a parked split |
| canonical `DisputeResolved` | `("chainsettle", "dispute_resolved")` | `resolution = "split"` |

The shipment audit log records `dispute_split_resolved` and, after a parked
split pays out, `split_resolution_finalized`.

## Storage

| Key | Contents |
|---|---|
| `DataKeyExt4::DisputeSplitBps(shipment_id, milestone_index)` | `supplier_bps` of the latest split |
| `DataKeyExt4::PendingSplit(shipment_id, milestone_index)` | `(supplier_bps, contested)` while parked |

Both keys are persistent, have their TTL extended when written, are refreshed
by `extend_shipment_related_keys_ttl`, and move with their milestone when
milestones are merged.

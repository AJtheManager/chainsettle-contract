# Buyer Late-Confirmation Fee

## Overview

ChainSettle can charge the buyer a configurable fee when they confirm a
milestone long after the supplier submitted proof. The fee compensates the
supplier for the cost of waiting on a buyer who sits on a submitted proof.
Auto-confirmation still applies, but the fee gives buyers a direct incentive
to confirm promptly.

## Configuration

The fee is a per-shipment option, configured when the shipment is created:

- **Grace ledgers** — the number of ledgers after proof submission during
  which no fee is charged.
- **Bps per ledger** — the fee rate, in basis points (1 bps = 0.01%), charged
  for each ledger past the grace period.
- **Cap** — the maximum fee, in the shipment's escrow token, that can ever be
  charged.

If the option does not fit in the `ShipmentOptions` / `Shipment` struct field
limit, it is stored under its own storage key, the same way `CancelPolicy` is
handled.

## Behaviour

- **No fee within the grace period.** If the buyer confirms within the grace
  period after proof submission, no fee is charged.
- **Linear accrual with a cap.** Past the grace period the fee accrues
  linearly with the number of elapsed ledgers, at the configured bps per
  ledger, and never exceeds the configured cap.
- **Deducted from escrow, paid to the supplier.** At confirmation the fee is
  deducted from the buyer's remaining unreleased escrow and paid to the
  supplier.
- **Never exceeds remaining escrow.** The fee is bounded by the remaining
  unreleased escrow, so it can never overdraw the shipment.

## Preview

`preview_milestone_payout` reflects the late-confirmation fee, so callers can
see the fee-adjusted payout before confirming.

## Events and audit log

State-changing calls emit events and write to the shipment audit log where
applicable. New contract function names and `Symbol::new` event topics stay
within Soroban's 32-character limit.

## Authorization and validation

Every new function validates its inputs and enforces caller authorization via
`require_auth`.

## Storage and TTL

Persistent entries have their TTL extended when written
(`extend_ttl(TTL_INITIAL_LEDGERS, TTL_MAX_LEDGERS)`), and per-shipment keys are
added to `extend_shipment_related_keys_ttl`.

## Tests

Unit tests live in a dedicated `test_<feature>.rs` module registered in
`lib.rs`, covering the happy path, authorization failures and edge cases.

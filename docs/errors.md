# Error Catalogue

The chainsettle contract reports failures by panicking with a string message. This document lists every distinct panic message raised by the contract, the function that raises it, and how an integrator can avoid or resolve it.

Source of truth: `contracts/chainsettle/src/lib.rs`.

## Initialization

### `"already initialized"`

- **Raised by:** `initialize`
- **Cause:** `initialize` was called on a contract instance that has already been initialized.
- **Fix:** Only call `initialize` once per deployed contract. Check the initialization state before calling, and treat a second call as a no-op on the client side.

## Shipment lifecycle

### `"shipment not found"`

- **Raised by:** `get_shipment`, `update_shipment_status`, `raise_dispute`, `resolve_dispute`
- **Cause:** The referenced shipment ID does not exist in storage.
- **Fix:** Verify the shipment ID returned by `create_shipment` before using it. Do not query or mutate shipments that were never created.

### `"invalid status transition"`

- **Raised by:** `update_shipment_status`
- **Cause:** The requested status change is not allowed from the shipment's current status (for example, moving backwards in the lifecycle or skipping a required step).
- **Fix:** Read the current status with `get_shipment` and only request transitions permitted by the lifecycle state machine.

### `"unauthorized"`

- **Raised by:** `update_shipment_status`, `raise_dispute`, `resolve_dispute`
- **Cause:** The caller is not the address authorized to perform the action (for example, not the shipper, carrier, or arbiter for the shipment).
- **Fix:** Ensure the transaction is signed by the correct party. Confirm the expected actor for the shipment before submitting the call.

## Disputes

### `"dispute already raised"`

- **Raised by:** `raise_dispute`
- **Cause:** A dispute has already been opened for the shipment.
- **Fix:** Check the shipment's dispute state before calling `raise_dispute`. Only one dispute may be active per shipment.

### `"no active dispute"`

- **Raised by:** `resolve_dispute`
- **Cause:** `resolve_dispute` was called for a shipment that has no open dispute.
- **Fix:** Only call `resolve_dispute` after a successful `raise_dispute` for the same shipment.

### `"invalid resolution"`

- **Raised by:** `resolve_dispute`
- **Cause:** The supplied resolution value is not one of the accepted outcomes.
- **Fix:** Pass a resolution value from the supported set defined by the contract.

## Amounts and inputs

### `"invalid amount"`

- **Raised by:** `create_shipment`
- **Cause:** The shipment amount is zero or otherwise outside the accepted range.
- **Fix:** Provide a positive amount within the contract's supported bounds.

### `"invalid address"`

- **Raised by:** `create_shipment`
- **Cause:** One of the party addresses supplied is invalid (for example, the zero address).
- **Fix:** Supply valid, non-zero addresses for every party.

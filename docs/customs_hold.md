# Customs Hold

## Overview

A logistics provider can place a milestone in a `CustomsHold` state when a shipment is
held at customs. While a milestone is on hold, its deadline and late-penalty clock are
paused, so the supplier is not penalised for delays outside their control.

## Functions

### `start_customs_hold(env, shipment_id, milestone_index)`

- Callable only by the logistics role; enforces `require_auth`.
- Marks the milestone as on hold and records the ledger at which the hold started.
- Emits a hold-start event and writes to the shipment audit log.
- Fails if the milestone is already on hold or the shipment/milestone does not exist.

### `end_customs_hold(env, shipment_id, milestone_index)`

- Callable only by the logistics role; enforces `require_auth`.
- Ends the hold and shifts the milestone deadline forward by the total time spent on hold.
- Emits a hold-end event and writes to the shipment audit log.
- Fails if the milestone is not currently on hold.

## Maximum hold duration

The admin configures a maximum hold duration. A hold cannot exceed this maximum: once the
maximum elapses the hold ends automatically and the deadline is shifted by the elapsed
hold time. Attempts to start a hold longer than the configured maximum are rejected.

## Deadline and penalty behaviour

- Deadlines shift forward by the total time spent on hold.
- The late-penalty clock is paused for the duration of the hold.

## Events

- Hold start event when a hold begins.
- Hold end event when a hold ends (manually or automatically).

## Authorization

Both `start_customs_hold` and `end_customs_hold` validate their inputs and enforce caller
authorization via `require_auth`, restricted to the logistics role.

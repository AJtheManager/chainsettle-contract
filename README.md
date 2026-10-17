# ChainSettle — Contract

![CI](https://github.com/shakurJJ/chainsettle-contract/actions/workflows/ci.yml/badge.svg)
![codecov](https://codecov.io/gh/shakurJJ/chainsettle-contract/branch/main/graph/badge.svg)

**Milestone-based supply chain escrow on Stellar Soroban.** The buyer locks payment in escrow, and the contract releases it to the supplier as each delivery milestone is confirmed on-chain.

This is one of three ChainSettle repos:

| Repo | What |
| --- | --- |
| `chainsettle-contract` (this one) | Soroban smart contract (Rust) |
| `chainsetttle-frontend` | React + Freighter wallet UI |
| `chainsetttle-backend` | Node.js API, notifications, off-chain metadata |

## How it works

```
Buyer creates shipment      → funds locked in escrow
Supplier submits proof      → milestone 1 (e.g. dispatch, 25%)
Buyer confirms              → 25% released to supplier
Logistics submits proof     → milestone 2 (transit, 50%)
Buyer confirms              → 50% released
Supplier submits proof      → milestone 3 (delivery, 25%)
Buyer confirms              → final 25% released, shipment Completed

Buyer disputes a proof      → milestone frozen until the arbiter resolves it
```

One deployed contract serves any number of independent shipments.

### Roles

| Role | Can do |
| --- | --- |
| Buyer | Create and fund shipments, confirm milestones, raise disputes, cancel |
| Supplier | Submit proof, receive payouts |
| Logistics | Submit proof (e.g. in-transit milestones) |
| Arbiter | Resolve disputes |
| Admin | Configure fees, tokens, limits, pauses, roles and upgrades (set at `init`) |

Every state-changing call needs the acting party's signature (`require_auth`).

### Milestone states

```
Pending ─ submit_proof ─→ ProofSubmitted
  ├─ confirm_milestone ─→ Confirmed (paid)
  └─ raise_dispute ─→ Disputed
        ├─ resolve_dispute(approve)  ─→ Resolved (paid)
        ├─ resolve_dispute(reject)   ─→ Pending (resubmit)
        └─ resolve_dispute_timeout   ─→ Resolved (default outcome)
```

## Features

The core flow above is extended by optional, admin- or party-configured features, including:

- **Disputes:** partial disputes, evidence, mediation, arbiter pools and panels, appeals, arbiter slashing, finality delay
- **Payments:** advances, payee splits, retainage, warranties, quality grades, partial quantities, payout batching and currency preference
- **Risk controls:** supplier collateral and slashing, exposure and TVL caps, buyer spending limits, depeg guard, blacklist with appeals
- **Fees:** fee tiers, VIP waivers, fee holidays, clean-completion rebates
- **Admin:** granular roles, multisig, timelock, global and per-operation pause, upgrades with `migrate`
- **Queries:** shipment summaries, cost estimates, completion percentage, reputation counters, paginated lists
- **Events:** every state change publishes an event. See [`docs/events.md`](docs/events.md) for each topic, its payload shape and the functions that emit it

For the full API, see the doc comments on each `pub fn` in [`lib.rs`](contracts/chainsetttle/src/lib.rs). Topic notes live in [`docs/`](docs/): [events](docs/events.md), [tokens](docs/tokens.md) and [completion percentage](docs/completion-percentage.md).

## Project structure

```
contracts/chainsetttle/
├── src/lib.rs         contract, types and storage keys
├── src/storage.rs     storage helpers
├── src/test_*.rs      unit tests, one file per feature; shared fixtures in test_common.rs
├── tests/             snapshot tests (UPDATE_SNAPSHOTS=1 to regenerate)
└── Makefile           build / optimize / deploy targets
```

## Getting started

**Prerequisites:** Rust ≥ 1.84 with the `wasm32v1-none` target, and the [Stellar CLI](https://developers.stellar.org/docs/tools/cli).

```bash
rustup target add wasm32v1-none
brew install stellar-cli            # or: cargo install --locked stellar-cli --features opt
```

**Build and test:**

```bash
cargo test                          # all tests
cargo test <name> -- --nocapture    # one test, with output

cd contracts/chainsetttle
make build                          # → target/wasm32v1-none/release/chainsetttle.wasm
make optimize                       # smaller .wasm for deployment (64 KB limit)
```

**Deploy to testnet:**

```bash
stellar keys generate --global my-account --network testnet
stellar keys fund my-account --network testnet
export STELLAR_ACCOUNT=my-account

cd contracts/chainsetttle && make deploy-testnet    # prints the contract ID

stellar contract invoke --id <CONTRACT_ID> --source my-account --network testnet \
  -- init --admin <YOUR_ADDRESS>
```

> Deploy to mainnet only after thorough testing and, ideally, a security audit. Mainnet USDC SAC: `CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7EJKEF`

## Security notes

- **Escrow:** funds sit at the contract address and leave only through explicit payout and refund paths.
- **Validation:** milestone percentages must sum to 100. Amounts use integer math, so the last milestone absorbs rounding.
- **Storage TTL:** persistent entries get an extended TTL on write. Long-lived shipments can be kept alive by anyone calling `extend_shipment_ttl`.
- **Upgrades:** `upgrade` (single admin) or `propose_upgrade` / `approve_upgrade` (multisig), followed by `migrate`.

## Contributing

PRs welcome. Run `cargo fmt` and `cargo test` before submitting.

## License

MIT

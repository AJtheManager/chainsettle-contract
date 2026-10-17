# Time-Bound Role Grants

Granular admin roles (`Pauser`, `FeeManager`, `ComplianceOfficer`,
`ArbiterManager`) can be granted with an expiry ledger. This is useful for
temporary operators such as incident responders, whose access then lapses on
its own and doesn't depend on someone remembering to revoke it.

## Functions

| Function | Caller | Description |
|---|---|---|
| `grant_role(admin, role, address, expires_at_ledger: Option<u32>)` | Admin | Grant a role, optionally until a ledger. |
| `revoke_role(admin, role, address)` | Admin | Revoke a role and clear any expiry. |
| `has_role(role, address) -> bool` | Anyone | `false` once the grant has expired. |
| `get_role_expiry(role, address) -> Option<u32>` | Anyone | The expiry of an active or lapsed grant; `None` if there is no grant or it has no expiry. |

## Semantics

- A grant with `expires_at_ledger = Some(n)` is valid while the current ledger
  sequence is **below** `n`. From ledger `n` onwards, every role check treats
  the grant as absent: `has_role` returns `false` and role-gated functions panic
  with `unauthorized`.
- `expires_at_ledger = None` gives a permanent grant, the same behaviour as
  before this feature.
- `n` must be greater than the current ledger, otherwise the call panics with
  `role expiry must be in the future`.
- Re-granting overwrites the previous grant. A new `Some(n)` extends or
  shortens the expiry, and `None` makes the grant permanent again. An expired
  grant can be renewed the same way.
- `revoke_role` removes both the grant and its expiry.
- The primary contract admin always passes role checks; expiries don't apply
  to it.

## Authorization, events and audit

- `grant_role` requires the admin's signature (`require_auth`) and panics with
  `unauthorized` for any other caller.
- `grant_role` keeps its existing `role_granted` event and its
  `grant_role / role_granted` admin-log entry.
- When an expiry is supplied, it also emits `role_expiry_set` with topics
  `(role_expiry_set, admin)` and data `(role, address, expires_at_ledger)`.

## Storage

| Key | Storage | Value |
|---|---|---|
| `DataKeyExt3::UserRole(Role, Address)` | persistent | `bool` (existing) |
| `DataKeyExt4::RoleExpiry(Role, Address)` | persistent | `u32` expiry ledger |

Both are written with `extend_ttl(TTL_INITIAL_LEDGERS, TTL_MAX_LEDGERS)`.

## Migration note

`grant_role` gained a fourth argument. Existing callers should pass `None` to
keep their current behaviour:

```rust
client.grant_role(&admin, &Role::Pauser, &operator, &None);              // permanent
client.grant_role(&admin, &Role::Pauser, &responder, &Some(now + 17_280)); // ~1 day
```

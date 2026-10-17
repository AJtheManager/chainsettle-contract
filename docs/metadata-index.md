# Shipment Metadata Index

Buyers and suppliers can attach custom key/value metadata to a shipment with
`set_shipment_metadata` (e.g. `po_number = "PO-2024-118"`). The metadata index
lets integrators look a shipment up by one of those pairs, for example finding
the shipment that carries a given purchase-order number.

Indexing is **opt-in per key**. Only keys the admin has marked as indexed are
searchable; every other key is stored exactly as before and cannot be queried.

## Functions

| Function | Caller | Description |
|---|---|---|
| `set_metadata_key_indexed(admin, key, indexed)` | Admin | Turn indexing on or off for a metadata key. |
| `is_metadata_key_indexed(key) -> bool` | Anyone | Whether `key` is currently indexed. |
| `find_shipments_by_metadata(key, value) -> Vec<String>` | Anyone | Shipment IDs whose metadata `key` currently equals `value`. |
| `set_shipment_metadata(caller, shipment_id, key, value)` | Buyer / supplier | Set a value and update the index (existing function). |
| `remove_shipment_metadata(caller, shipment_id, key)` | Buyer / supplier | Delete a value and drop the shipment from the index. |

### `set_metadata_key_indexed`

- Requires the admin's signature (`require_auth`) and panics with
  `unauthorized` for any other address.
- Records a `set_meta_indexed` entry in the admin action log.
- Emits `metadata_key_indexed` with topics `(metadata_key_indexed, key)` and
  data `(admin, indexed)`.

### `find_shipments_by_metadata`

- Panics with `metadata key not indexed` when `key` is not indexed.
- Panics with `metadata value must not be empty` for an empty value, and with
  `metadata value too long to index` for values longer than
  `MAX_METADATA_INDEX_VALUE_LEN` (128 bytes).
- Returns IDs in the order they were indexed. Each candidate is checked against
  the shipment's live metadata before it is returned, so stale entries are
  never reported.

### Index maintenance

- **Set**: when the key is indexed, the shipment is added to the
  `(key, value)` bucket. Changing a value moves the shipment out of the old
  value's bucket and into the new one. Setting the same value twice does not
  create a duplicate entry. Values for indexed keys must be non-empty and at
  most `MAX_METADATA_INDEX_VALUE_LEN` bytes.
- **Remove**: the shipment is dropped from the bucket of the value it held.
- Removals from an old bucket happen even if the key is no longer indexed,
  so the index stays clean while indexing is switched off.
- A bucket holds at most `MAX_METADATA_INDEX_ENTRIES` (100) shipments. Once
  it is full, `set_shipment_metadata` for that pair panics with
  `metadata index full`.

Both `set_shipment_metadata` and `remove_shipment_metadata` now append an entry
to the shipment audit log (`metadata_set` / `metadata_removed`).
`remove_shipment_metadata` emits `shipment_metadata_removed` with topics
`(shipment_metadata_removed, shipment_id)` and data `(caller, key)`. It panics
with `metadata key not found` if the key is not set on the shipment.

## Limitations

- **No backfill.** Enabling indexing for a key does not scan existing
  shipments. Values set before the key was indexed become searchable once a
  party calls `set_shipment_metadata` again (re-setting the same value is
  enough).
- Values written while a key is not indexed are not added to the index.
  Disabling and re-enabling a key therefore only finds shipments whose value
  was set while indexing was on.
- Archiving a shipment keeps its metadata, so archived shipments are still
  returned by lookups.

## Storage

| Key | Storage | Value |
|---|---|---|
| `DataKeyExt4::MetadataKeyIndexed(Symbol)` | persistent | `bool` |
| `DataKeyExt4::MetadataIndex(Symbol, String)` | persistent | `Vec<String>` (shipment IDs) |

Both entries are written with `extend_ttl(TTL_INITIAL_LEDGERS, TTL_MAX_LEDGERS)`.

## Example

```rust
client.set_metadata_key_indexed(&admin, &Symbol::new(&env, "po_number"), &true);
client.set_shipment_metadata(&buyer, &shipment_id, &Symbol::new(&env, "po_number"),
    &String::from_str(&env, "PO-2024-118"));

let ids = client.find_shipments_by_metadata(
    &Symbol::new(&env, "po_number"),
    &String::from_str(&env, "PO-2024-118"),
);
```

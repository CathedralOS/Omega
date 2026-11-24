# Packet unit_031_gap_016: selected-instruction effect catalogs (area 12)

Status: in progress. This early receipt settles the first claim.

Scope: [record](../../records/unit_031_gap_016.md) for
[`units/gap_016.tsv`](../../units/gap_016.tsv), area 12-native-representations.
The ranges are `effects/catalog/identity.rs` 1-2336, `effects/catalog/validation.rs`
1-3601 and `effects/program.rs` 1-63 under
`source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions/src/`.
Baseline `24ad2700685e`; reviewed at `a0152d26`.

| # | Record section | Code path | Outcome | Evidence |
| --- | --- | --- | --- | --- |
| 1 | §1 Fail-open risks, encoded-memory tag 19 (`identity.rs:278-285`, `:440`) | `effects/memory.rs`, `catalog/identity.rs` `encode_machine_encoded_effects_identity` | Fixed ([#5185](https://github.com/CathedralOS/Omega/pull/5185)); duplicate of the area-12 ledger row "effect tag 19" in [07-13](../07-13-psi-and-native.md) | The encoder now calls `effects.memory.encode(bytes)`. `memory.rs` gives `ReadTableFieldV1` tag 19 and `CatalogFenceV1` tag 28, one wire table for identity and plan decode. `encoded_memory_effect_identity_bytes_are_pinned` (`identity.rs:1659`) pins `[28]` and `[19, ...]`. |

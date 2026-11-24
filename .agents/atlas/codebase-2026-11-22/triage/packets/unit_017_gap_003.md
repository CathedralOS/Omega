# Packet: unit_017_gap_003 (area 11, native stages 11 to 15)

This ledger gives a disposition for each claim in [record unit_017_gap_003](../../records/unit_017_gap_003.md) and for the area 11 digest lines that cite it. Review baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`; code citations are at current `main` (`a0152d26`) unless stated. `P` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding`.

Status: in progress (early receipt).

## Claims

### C1. AArch64 system-call and table-call validators take register reads and writes from the caller

- **Source:** record §1 "Fail-open risks" (bullets 1 and 2); five-things item 3; [digest 11 line 154](../../digests/11-native-stages-11-15.md?plain=1#L154).
- **Code path:** `P/isa-aarch64/src/selected_form_encoding.rs` `system_call::validate_aarch64_selected_system_call_form` and `table_call::validate_aarch64_selected_table_call_form`; `targets/target-aarch64/src/inspection.rs` `selected_system_call_effects` and `selected_table_call_effects`; `targets/register-model/src/constraint_catalog.rs` `row_matching`.
- **Contract:** the footprint's reads and writes must be the fixed operands of the catalog row the bytes realize.
- **Trace:** `selected_system_call_effects` returns `None` unless `catalog.row_matching(&aarch64_linux_system_call_keys(), views, accesses)` finds a row whose operand count, `fixed_view` and `access` equal the caller's lists one for one. `selected_table_call_effects` makes the same per-operand comparison against `normalized_foreign_constraint(record)`. A wrong access list therefore fails validation with `EncodedFormMismatch` before the footprint is built. `writes_nzcv: true` claims a flag clobber across a foreign call or `svc`, which over-approximates and is the safe direction. Fixture: `system_call::tests::system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes` replays every transport row.
- **Outcome:** intended (fail-closed). No fix.

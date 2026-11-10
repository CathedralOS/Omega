slice_0073

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0561 | type_references_and_wire.rs:2186-2217 (record also cites 2153-2154) | REACHABLE | Index-legal non-structured carrier accepts `from_atom` with matching `type_name` only; encoding not validated on that branch (read). |
| A0562 | type_references_and_wire.rs:2114-2116 | CLOSED | Pending range endpoints skip const-arg errors here; strict prepare rejects unresolved endpoints before check (read). |
| A0563 | type_references_and_wire.rs:3688-3698 | REACHABLE | `normalize_closed_const_expression_in_carrier` → `Ok(None)` becomes `Ok(())` with no identity check when carrier is ineligible (read). |
| A0564 | type_references_and_wire.rs:3462-3485 | CLOSED | Early returns for unlisted ops / bad index type are normalization-only; `validate_open_index_expression` errors unsupported shapes (read). |
| A0565 | type_references_and_wire.rs:6779-6801 | REACHABLE | Adjacent wire eras check dropped numbers only, not type agreement when field number survives (read). |
| A0566 | value_custody_core.rs:526-541,584 | REACHABLE | `carries_activation_claim` does not peel `Reference`/`Slice`/`DynamicTrait`, so inner activation storage is invisible (read). |
| A0567 | value_custody_core.rs:1521 | REACHABLE | `FixedArray` with non-literal length hits empty arm; no element claim frontier walk (read). |
| A0568 | value_custody_core.rs:1460-1462,1491-1495 | REACHABLE | Missing data definition returns without pushing claims; linear frontier can be empty for that type (read). |

### A0561

1. Read `validate_const_data_argument` in full at `type_references_and_wire.rs:2036-2280` (read); cited 2153-2154 are the non-`Named` diagnostic branch, not the atom path (read).
2. Search `\bvalidate_const_data_argument\b` under `omega-rust/`: **7 hits**, all in `type_references_and_wire.rs` (read); call sites at 173, 1733, 1778, 5289 plus re-exports (read). Did not read every generic-argument caller in full.
3. Upward: generic application validation → `validate` diagnostics gate program acceptance (inferred from `constraint_validation` use at 769 calling `validate_indexed_domain_arguments`, read 600-769 partially).
4. Input to accepting branch: const generic argument spelled as a canonical atom (`TypeReferenceNode::Named` with invalid symbol) where carrier is index-legal but `exact_structured_const_carrier_is_eligible` is false (`2521-2543`, read); `from_atom` succeeds (`const_value.rs:218-226`, read) but branch at 2200-2216 checks only `value.type_name` (2210-2215), not `validate_exact_const_identity_in_carrier` (2188-2197 used only on eligible carriers, read).
5. **Verdict: REACHABLE** — A const parameter expecting a small integer index type could receive an atom whose prefix/type name matches while the encoding bytes are not a decodable integer; validation would not report malformed encoding on this branch (read).

### A0562

1. Read `validate_const_data_argument` including `2111-2116` and enclosing function (read).
2. Search `\bpending_const_range_endpoints\b` under `omega-rust/`: **many hits**; validation crate uses it only at `type_references_and_wire.rs:141,2114` (read).
3. Callers of `validate_const_data_argument`: search **7 hits** (above). `validate_const_argument_expression_formation` also called from `machine_parameters_and_bindings.rs:7994` (read one caller, not full `check_program`).
4. Input to accepting branch: `TypeReferenceNode::ConstExpression(root)` with `root` still listed in `program.pending_const_range_endpoints` (2114-2115, read).
5. **Verdict: CLOSED** — On the default preparation path (`admits_pending_inputs: false`, `preparation.rs:1838`, read), `deferred_constants::require_evaluated_constants` (`preparation.rs:415-436`, read) errors on range endpoints still pending without a valid recipe before `normalize_open_index_identities` / checking; the early return defers diagnosis only while folding is in flight. I did not trace the `admits_pending_inputs: true` dependency-check path in full.

### A0563

1. Read `validate_indexed_domain_argument_pack` at `3636-3726` and `normalize_closed_const_expression_in_carrier` at `1097-1130` (read).
2. Search `\bnormalize_closed_const_expression_in_carrier\b` in `omega-rust/psi/compilation/validation`: **1 definition + uses in same file** (read).
3. Caller: `validate_indexed_domain_arguments` → domain constraint validation during checking (read 3187-3205).
4. Input: closed indexed domain application, `ConstExpression` argument, `closed_application` true, `normalize_closed_const_expression_in_carrier` returns `Ok(None)` because `!exact_structured_const_carrier_is_eligible` (1103-1104, read); `.and_then` at 3689-3697 still yields `Ok(())` (3688-3698, read).
5. **Verdict: REACHABLE** — Closed const expression in an indexed domain with a non-exact-structured index carrier can pass this completion hook without `validate_exact_const_identity_in_carrier` when normalization declines the carrier (read).

### A0564

1. Read `normalize_open_index_expression_operations` in full `3448-3516` and parent `normalize_open_index_expressions` `3266-3359` (read).
2. Search `\bnormalize_open_index_expression_operations\b`: **4 hits**, all `type_references_and_wire.rs` (read).
3. Also read `validate_open_index_expression` `3788-3897` (read).
4. Input to silent returns: non Add/Sub/Mul/Div binary (3462-3467), missing owner (3469-3470), or index type not `Named`/`Constrained` (3479-3484) (read).
5. **Verdict: CLOSED** — Normalization skips are not acceptance of invalid open-index algebra: `validate_open_index_expression` still runs on domain packs (3666-3674, read) and rejects unsupported expression forms at 3893-3896 (read). Wrong operators therefore get diagnostics on the validate path; normalization silence is harmless for program acceptance. I did not prove the same for every `owner == None` case without reading all downstream monomorphization consumers.

### A0565

1. Read `validate_adjacent_eras` in full `6771-6802` and caller `validate_wire_protocol` era loop `6579-6584` (read).
2. Search `\bvalidate_adjacent_eras\b`: **2 hits** (definition + call, read). `validate_wire_protocol` invoked from `preparation.rs:1955` (read).
3. No successor-field type comparison when `successor_field.is_some()` (6779-6801, read); `validate_scope_field_types` runs per era separately, not across eras (6567-6569, read).
4. Input: wire schema with two versions sharing a field number but different field type strings (inferred from `WireField` structure, partial read of field validation).
5. **Verdict: REACHABLE** — A protocol evolution that reuses a field number with an incompatible type in the next version would pass adjacent-era validation if the field is not dropped and not tombstoned (read).

### A0566

1. Read `carries_activation_claim` / `type_carries_activation_claim` in full `513-586` (read).
2. Search `\bcarries_activation_claim\b`: **8 hits** in `value_custody_core.rs`, plus re-export and `moves_activation_claim` at `execution_terminal_unit_state_graph.rs:1085` (read).
3. Read `moves_activation_claim` `1080-1087` (read); used in terminal unit state graph retention `1053-1056` (read).
4. Input: parameter or type reference wrapped as `Reference`, `Slice`, or `DynamicTrait` whose referee carries activation claim storage in a record field (read 584, 1522-1526 parallel).
5. **Verdict: REACHABLE** — A state parameter typed as a reference to a struct that contains activation-claim bytes would not be treated as moving activation custody, so graph retention could treat the transition differently than for a by-value struct with the same inner layout (read).

### A0567

1. Read `append_linear_claim_frontier` through both matches `1354-1527` (read).
2. Search `\bappend_linear_claim_frontier\b`: **8 hits**, all `value_custody_core.rs` (read).
3. Callers `linear_claim_frontier`, `type_carries_linear_obligation` (read 1313-1335); used widely in validation (inferred from grep sample).
4. Input: `FixedArray` whose `length` is not `FixedArrayLength::Literal` (e.g. const parameter) — first match arm `1384-1432` skipped; second match `1521` empty (read).
5. **Verdict: REACHABLE** — A linear element type inside an array whose length is still a const generic parameter gets no per-element claim frontier expansion at this layer, understating linear obligations (read). Literal-length arrays are handled at 1384-1432 (read).

### A0568

1. Read `append_linear_claim_frontier` and `find_data_definition` `1725-1734` (read).
2. Search `\bfind_data_definition\b` in `value_custody_core.rs`: definition + claim_frontier uses (read).
3. Production path: unresolved `Named`/`Generic` base where `find_data_definition` / `program.closed_data_definition` both fail → `return` without claims (1460-1462, 1491-1495, read).
4. Input: type reference that names data not present in `CheckingInput` tables at validation time (read).
5. **Verdict: REACHABLE** — If such a reference could reach claim-frontier computation, the program would behave as having no linear claims for that subtree (read). I did not exhaustively trace whether earlier typed-tree validation always rejects unresolved data names before this runs.

## What I did not read

- Full `check_program` / `program_validation::validate` coordinator and every prologue validator.
- Complete `exact_structured_const_carrier_is_eligible` recursive body beyond `2514-2543`.
- Downstream consumers of empty open-index `operations` vectors after `normalize_open_index_expressions`.
- `admits_pending_inputs: true` dependency-check admission path end-to-end.
- Corpus or runtime behavior; all conclusions are from static reads at workspace HEAD (not verified at commit `457adf4a11f`; line numbers matched current files).

SLICE COMPLETE

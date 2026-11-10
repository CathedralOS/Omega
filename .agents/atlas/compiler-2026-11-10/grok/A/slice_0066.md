# slice_0066

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0505 | omega-rust/psi/compilation/validation/src/operators.rs:1939-1943 | REACHABLE | Empty trait meanings plus a non-primitive operand `continue`s without recording any selection while the function still returns `Ok`. |
| A0506 | omega-rust/psi/compilation/validation/src/operators.rs:2075 | REACHABLE | A binary whose operand types are all `None` records no operator-use fact and assembly `continue`s on `Ok(None)`. |
| A0507 | omega-rust/psi/compilation/validation/src/places.rs:448-455 | REACHABLE | A longer static write path is forced non-overlapping with a shorter observed path before the general overlap walk runs. |
| A0508 | omega-rust/psi/compilation/validation/src/places.rs:708 | REACHABLE | Pairwise segment comparison treats unlike segment kinds as non-overlapping via `_ => false`. |
| A0509 | omega-rust/psi/compilation/validation/src/places.rs:6026-6029 | REACHABLE | Unhandled expression forms become unique `Expression` roots, so `place_relation` reports `Disjoint` across roots. |
| A0510 | omega-rust/psi/compilation/validation/src/places.rs:1131-1139 | REACHABLE | `first_unknown_nested_field` returns `None` when the member receiver type does not resolve, so assignment emits no missing-field diagnostic. |
| A0511 | omega-rust/psi/compilation/validation/src/places.rs:94-103 | REACHABLE | Borrow/member chains deeper than 128 steps stop resolving and writes are not attributed to the intended root binding. |
| A0512 | omega-rust/psi/compilation/validation/src/places.rs:5169-5194 | REACHABLE | `zeroed_structural_parts` walks only the first variant payload for enums, not later variants. |

## A0505

**Digest (read):** area_14 preamble abbreviates `ops` as `omega-rust/psi/compilation/validation/src/operators.rs` (`digests/area_14.md:22-23`). Group claim: `continue` when no trait meaning and an operand is non-primitive (`digests/area_14.md:274`).

**Cited site (read):** `operators.rs:1887-1967`, function `uses::collect_pre_specialization_operator_selections`. Lines `1939-1943` match this checkout.

On `([], [])` for bound/selected trait meanings, if any flattened operand type is not a primitive reference, the loop hits `continue` and never reaches the builtin/intrinsic fallback at `1944-1950`. The function finishes with `Ok(selections)` (`1963-1966`).

**Callers (read):** `rg 'collect_pre_specialization_operator_selections' omega-rust` → 4 symbol hits: definition and re-export in `operators.rs` / `lib.rs`, one production call in `preparation.rs:1371` inside `bind_pre_specialization_authored_selections`.

**Input that reaches the branch (read):** A machine-state binary with a spelled operator, empty `bound_trait_operator_meanings` and `selected_trait_operator_meanings`, at least one operand whose type reference is not a primitive, scanned in the contract-frame walk (`1908-1961`).

**Accept path (read):** `bind_pre_specialization_authored_selections` (`preparation.rs:1365-1391`) only binds occurrences present in the returned vector; it does not error on omissions. Separate operator checking via `build_operator_uses` (`operators.rs:1978-2086`) was not traced as a closing gate for every omission from this vector.

**Verdict:** REACHABLE — checking can proceed without a pre-specialization selection row for a non-primitive binary that has no trait meaning at this site.

## A0506

**Digest (read):** `ops:2075` — no fact when all operand types are `None` (`digests/area_14.md:275`).

**Cited site (read):** `operators.rs:2002-2087`, function `collect_expression_operator_use`, `ExpressionNode::Binary` arm. Line `2075` is `operand_types.iter().any(Option::is_some)`; when false, no `binary_operator_use_fact` and no `uses` row.

**Enclosing function (read):** `build_operator_uses` (`1978-2000`) invokes this walker for every checked value origin except nested-expression origins (`1985-1988`).

**Callers (read):** `rg 'build_operator_uses' omega-rust` → 4 hits: definition/re-export and `assembly_and_operators.rs:1453` (production assembly), plus `operators.rs:527` via `build_operator_facts`.

**Upstream accept (read):** `assembly_and_operators.rs:1498-1520` — for binaries added with `use_handle: None` (`1493-1494`), `selected_call_target` (`1950-2088`) falls through trait binding attempts, then at `2060-2061` returns `Ok(None)` when `operator_use` is absent; the loop `continue`s without pushing a diagnostic.

**Input (read):** A binary evaluated under a `CheckedValueOrigin` where `expression_type_reference_for_origin` returns `None` for both operands (`2072-2074`), yet the expression is still visited in assembly’s supplemental scan (`1477-1495`).

**Verdict:** REACHABLE — operator assembly can silently skip a binary that produced no operator-use fact because both operand types were unknown at fact collection time.

## A0507

**Digest (read):** `pl:448-455` — write path longer than observed does not overlap the case tag (`digests/area_14.md:276`).

**Cited site (read):** `places.rs:443-457`, `storage_path_relations::case_tag_segments_may_overlap`. Cited range matches.

When `written.len() > observed.len()` and every segment in `written` and `observed` has resolved identity (`448-453`), the function returns `false` and never calls `canonical_place_segments_may_overlap` (`456`).

**Callers (read):** `rg 'case_tag_segments_may_overlap' omega-rust` → 5 hits: definition; re-export in `flow_place.rs` / `flow.rs`; one call in `flow_exits.rs:768` inside case-content survival checking.

**Input (read):** Case-tag content observation with a short `observed` segment list and a write whose segment list is strictly longer but all-static (for example tag-level observation versus a write into a resolved field inside that variant), passed to the closure at `flow_exits.rs:767-769`.

**Verdict:** REACHABLE — flow can treat a deeper static write as non-overlapping with a shorter case-tag observation without running the general segment overlap logic.

## A0508

**Digest (read):** `pl:708` — `_ => false`: different segment kinds count as disjoint (`digests/area_14.md:277`).

**Cited site (read):** `places.rs:665-709`, `canonical_place_segment_pair_may_overlap`. Line `708` is the `_ => false` arm after pairing `FixedRange`, `FixedIndex`, `Index`, and `OpaqueRange` cases.

**Enclosing logic (read):** `canonical_place_segments_may_overlap` (`628-643`) zips prefixes; the first pair for which `canonical_place_segment_pair_may_overlap` is false makes the whole path pair non-overlapping.

**Callers (read):** `rg 'canonical_place_segments_may_overlap|place_relation\(' omega-rust` → production uses across validation `places.rs` / `domains_and_predicates.rs` and many stage-05 flow modules (37 path hits in `omega-rust`).

**Input (read):** Two place paths whose first differing zipped segments have different `PlaceSegment` variants (for example `Case` versus `Field` at the same index).

**Verdict:** REACHABLE — overlap analysis reports disjointness from kind mismatch alone, which can mark places as non-interfering when a stricter model would allow overlap.

## A0509

**Digest (read):** `pl:6026-6029`, `6585` — other expressions become their own root, so `Disjoint` from everything (`digests/area_14.md:278`).

**Cited sites (read):** `places.rs:5943-6030`, `canonical_projection::canonical_place_from_expression` — default arm `6026-6029` sets `PlaceRoot::Expression(expression)` with empty segments. `places.rs:6459-6586`, `contextual_canonical_place_from_expression` — at `6585`, unhandled forms delegate to `canonical_place_from_expression`, which can hit the same default arm.

**Downstream (read):** `place_relation` (`587-595`) returns `Disjoint` when `left.root != right.root`.

**Callers (read):** `rg 'canonical_place_from_expression' omega-rust/psi/compilation/validation` → 34+ uses in validation alone (loans, entitlements, program validation, domains, calls, etc.).

**Input (read):** Two distinct expression handles (for example separate call results or literals) that are not names, members, indexes, or borrow/cast-preserving forms; each becomes a distinct `Expression` root.

**Verdict:** REACHABLE — overlap and place-equality checks treat unrelated expression roots as always disjoint even when semantics might require relating them.

## A0510

**Digest (read):** `pl:1131-1139` with `pl:4058-4121` — unresolved receivers produce no “no field” diagnostic (`digests/area_14.md:279`).

**Cited sites (read):** `places.rs:1101-1139`, `validate_assignment_target_handle` calls `first_unknown_nested_field` at `1131-1138` without a `Member`/`Name` shape guard (contrast `calls.rs:2460-2469`, which guards reads).

`places.rs:4058-4121`, `first_unknown_nested_field`: for `ExpressionNode::Member`, `declared_place_type_raw(..., member.receiver)?` at `4070-4071` — `None` receiver type makes the whole function return `None` before field existence is tested.

**Callers (read):** `validate_assignment_target_handle` ← `program_validation.rs:1559` (assignment validation). `first_unknown_nested_field` ← `places.rs:1132`, `calls.rs:2349`, `calls.rs:2464` (3 call sites, 4 symbol hits).

**Input (read):** An assignment target that is a nested member chain whose receiver subexpression does not yield a declared place type (unresolved borrow/call/index intermediate), but still passes earlier `is_mutable_place_handle` (`1112-1117`).

**Verdict:** REACHABLE — assignment validation can pass without the missing-field error that would fire when the receiver type is known.

## A0511

**Digest (read):** `pl:94-103`, `220-230` — chains longer than 128 judged not to write (`digests/area_14.md:280`).

**Cited sites (read):** `places.rs:89-104`, `selects_collection_contents` — loop stops after `visited < 128`, then returns `false` for non-`Indexed` tail (`94-103`). `places.rs:215-230`, `borrow_root_symbol` — same 128 bound, returns `None` when the chain does not resolve to `ExpressionNode::Name` (`220-230`).

**Consumers (read):** `binding_is_written` (`125-147`) uses `borrow_root_symbol` on assignment targets (`137`). `binding_is_unchanged_value` and contract/call paths consume `binding_is_written` (`contract_entailment.rs` 5 hits, `calls.rs` 3 hits; `rg 'binding_is_written' omega-rust/psi/compilation/validation` → 11 hits).

**Input (read):** A state binding allegedly written via an assignment or mutable call receiver path requiring more than 128 consecutive `Borrow`/`Member`/`Indexed` hops before a name root.

**Verdict:** REACHABLE — the compiler stops walking and treats the binding as not written or without a resolvable root.

## A0512

**Digest (read):** `pl:5169-5194` — enum whose first variant is zero-representable and later variants are not would be judged zeroable (`digests/area_14.md:281`).

**Cited site (read):** `places.rs:5132-5199`, `zeroed_structural_parts`. For `TypeReferenceNode::Named`/`Generic` with variants, only `variants.next()` (`5169`) supplies the case whose payload fields are enqueued (`5189-5194`); other variants’ payloads are not walked.

**Callers (read):** `rg 'zeroed_structural_parts' omega-rust` → 8 hits: definition, re-export, `execution_scalar_computations.rs` (3 uses), `execution_terminal_unit_control_statement_sequence.rs:1377`, `build-time-evaluation/layouts.rs:6254`.

**Input (read):** A closed enum-typed local or operand where variant 0’s payload is all zeroable bits but a later variant’s payload contains non-zeroable structure; callers gate zero-init or uninitialized-value policy on `zeroed_structural_parts` returning `true`.

**Verdict:** REACHABLE — the function can answer `true` while a non-first variant would forbid treating the whole type as zeroable.

## What I did not read

- Full `scoped_operator_application` / `select_scoped_candidates` outcome paths for every non-primitive binary omitted from pre-specialization (`operators.rs:1227-1483`).
- Whether stage-04 `validate` or typed-tree typing always rejects binaries before `operand_types` are all `None` at `operators.rs:2075`.
- Complete stage-05 flow consumers of `case_tag_segments_may_overlap` beyond `flow_exits.rs:768`.
- Corpus samples demonstrating accept for any REACHABLE row.
- `declared_place_type` / `declared_indexed_projection_type` follow-up on assignment targets after `validate_assignment_target_handle` (`program_validation.rs:1581-1584`).
- All `place_relation` consumers that would reinterpret `Expression`-root disjointness.

SLICE COMPLETE

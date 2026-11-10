slice_0066

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0505 | validation/src/operators.rs:1939-1943 | CLOSED | Skips prep intrinsic binding only; `build_operator_uses` still records uses and `check_operator_resolution` reports Missing/Inadmissible. |
| A0506 | validation/src/operators.rs:2075 | CLOSED | All-`None` operand types omit the use row, but the same guard exists at 1926-1927 and ill-typed binaries are not silently accepted at check time. |
| A0507 | validation/src/places.rs:448-455 | CLOSED | Longer all-static write paths are intentionally non-overlapping with shorter case-tag observations in `flow_exits.rs:768`. |
| A0508 | validation/src/places.rs:708 | CLOSED | Mismatched segment kinds at one path position are ill-formed for typed place paths; `Disjoint` is conservative for overlap queries only. |
| A0509 | validation/src/places.rs:6026-6029 | REACHABLE | Fallback `PlaceRoot::Expression` makes unrelated expression handles `Disjoint` from symbol-rooted storage for the same value. |
| A0510 | validation/src/places.rs:1131-1139; 4058-4121 | REACHABLE | `first_unknown_nested_field` returns `None` when `declared_place_type_raw` fails, so no nested “no field” diagnostic is emitted on that path. |
| A0511 | validation/src/places.rs:94-103; 220-230 | REACHABLE | Member/borrow chains longer than 128 steps yield `false`/`None`, so bindings can be treated as not written. |
| A0512 | validation/src/places.rs:5169-5194 | REACHABLE | Variant-bearing data uses only the first variant’s payload; later non-zeroable variants can still make `zeroed_structural_parts` return true. |

### A0505

1. Read `collect_pre_specialization_operator_selections` in full at `validation/src/operators.rs:1887-1967` (read).
2. Search `rg 'collect_pre_specialization_operator_selections' /agent/repos/omega/omega-rust` → 3 hits (read): definition `operators.rs:1887`, re-export `validation/src/lib.rs:1768`, sole production caller `pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:1371` (read).
3. Read caller `bind_pre_specialization_authored_selections` at `preparation.rs:1365-1391` (read); did not read full `push_consistent_resolution` or later late-bound loop at `1395+` (not read).
4. Input reaching the branch: a `Binary` in a machine state contract or statement tree, `binary_operator_spelling` present, not all operand types `None` (at least one `Some` from `expression_result_type_reference` at `1923-1925`), both `bound_trait_operator_meanings` and `selected_trait_operator_meanings` empty, and at least one operand type for which `primitive_type_reference` is `None` (`1939-1941`).
5. **Verdict: CLOSED** — `checks_operators.rs:2835-2850` (`check_operator_resolution`, read) turns `CheckedOperatorFacts::resolution_issues` (`operators_and_proof.rs:1326-1337`, read) into diagnostics for `Missing`/`Inadmissible`/`Ambiguous` uses built by `build_operator_uses` (`operators.rs:2069-2086`, read). The `continue` only withholds a prep-stage `BuiltinOperator` intrinsic target, not acceptance of an unresolved operator on typed operands.
6. Unsure: whether any authored-selection-only failure bypasses `build_operator_uses` without hitting `resolution_issues` (did not trace every late-bound selection path in `preparation.rs:1395-1465`).

### A0506

1. Read `collect_expression_operator_use` in full for the `Binary` arm at `operators.rs:2002-2105`, especially `2070-2086` (read).
2. Search `rg '\bbuild_operator_uses\b' /agent/repos/omega/omega-rust` → 4 hits (read): `operators.rs:1978`, re-exports, `operators.rs:527` (`build_operator_facts`), `assembly_and_operators.rs:1453`.
3. Read `assembly_and_operators.rs:1493-1520` and `selected_call_target` `2060-2061` returning `Ok(None)` when `operator_use` absent (read); read `checks_operators.rs:2835-2850` (read).
4. Input: `Binary` with `operand_types` where every `expression_type_reference_for_origin` is `None`, so `operand_types.iter().any(Option::is_some)` is false and no `CheckedOperatorUseFact` is appended (`2075-2086`).
5. **Verdict: CLOSED** — Missing rows cannot surface in `resolution_issues`, but the same all-`None` guard at `1926-1927` means prep selection also skips the expression; typed-tree assembly still walks values only for machine-holding origins (`1477-1496`, read). Acceptance is gated earlier by needing typeable executable values in normal pipelines (inferred from `collect_executable_value_origins` use at `1452`, not fully traced).
6. Unsure: full behavior when both operands lack types but the binary still appears in an occurrence list at `1493-1495` (read snippet only).

### A0507

1. Read `case_tag_segments_may_overlap` at `places.rs:443-457` and `canonical_place_segments_may_overlap` at `628-644` (read).
2. Search `rg 'case_tag_segments_may_overlap' /agent/repos/omega/omega-rust` → 4 hits (read): definition `places.rs:443`, imports in `flow.rs` / `flow_place.rs`, use `flow_exits.rs:768`.
3. Read `flow_exits.rs:764-770` (read): when `observed.descriptor` is invalid, write/relation uses `case_tag_segments_may_overlap` on `place.segments` vs `written`.
4. Input: `written` slice strictly longer than `observed`, and every segment in both slices passes `!place_segment_has_unresolved_identity` (`448-452`, read); function returns `false` before delegating to `canonical_place_segments_may_overlap`.
5. **Verdict: CLOSED** — This early `false` implements “longer all-static write does not overlap case-tag-only observation” for content-survival analysis (`flow_exits.rs:764-783`, read); a longer path into payload fields is intentionally not treated as overlapping a tag-only observed path.
6. Unsure: none for the cited control flow (did not read full `contents_observation_survives`).

### A0508

1. Read `canonical_place_segment_pair_may_overlap` in full at `places.rs:665-710` (read).
2. Search `rg 'canonical_place_segment_pair_may_overlap' /agent/repos/omega/omega-rust` → 1 hit (definition only, read); overlap flows through `canonical_place_segments_may_overlap` (`628-644`, read).
3. Read `place_segments_relation` `598-617` (read).
4. Input: zipped segment pair where `static_place_segment_pair_equal` is `None` and the pair is not one of the handled range/index cases, e.g. `PlaceSegment::Field` vs `PlaceSegment::Case` at the same index (`708-709`, read).
5. **Verdict: CLOSED** — Well-formed canonical places from `canonical_place_from_expression` / `push_field_place_segments` (`5932-5940`, read) do not mix unrelated segment kinds at one prefix; `_ => false` makes `canonical_place_segments_may_overlap` return false and `place_segments_relation` return `Disjoint` (`603-604`, read), which is conservative for “may overlap” queries, not a substitute for accepting an ill-typed program.
6. Unsure: whether any producer emits mixed-kind prefixes on accepted programs (did not audit all `PlaceSegment` builders).

### A0509

1. Read `canonical_place_from_expression` in full at `places.rs:5943-6030` (read), including fallback `_ => Some(CanonicalPlace { root: PlaceRoot::Expression(expression), ...})` at `6026-6029`.
2. Read contextual fallback at `6585` (read) delegating to the same projection.
3. Search `rg 'canonical_place_from_expression\(' /agent/repos/omega/omega-rust/psi/compilation/validation` → 77+ hits (count from grep, read); read `place_relation` `587-595` (read) and `PlaceRoot` equality `contexts_and_places.rs:1974-1984` (read).
4. Input: any valid expression node not handled by earlier arms (e.g. `Call`, `Binary`, literals), receiving its own `PlaceRoot::Expression(expression)` with empty `segments`.
5. **Verdict: REACHABLE** — Two different expressions naming the same storage, or an expression-rooted place vs a symbol-rooted place for one local, yield `PlaceRelation::Disjoint` at `592-593` (read) even when they alias; entailment and custody code using `canonical_place_from_expression` (`contract_entailment.rs` citations in grep, not all read) can therefore treat them as non-overlapping.
6. Unsure: how often overlap checks pair `Expression` roots with `Symbol` roots on accepted programs (did not trace all 77 call sites).

### A0510

1. Read `validate_assignment_target_handle` `1101-1139` (read) and `first_unknown_nested_field` `4058-4121` in full (read); read `declared_place_type_raw` member arm `1368-1382` (read).
2. Search `rg 'first_unknown_nested_field' /agent/repos/omega/omega-rust` → 3 hits (read): `places.rs:1132`, `calls.rs:2349`, `calls.rs:2464`.
3. Read `calls.rs:2446-2484` (read): nested-field diagnostic only when `first_unknown_nested_field` returns `Some`.
4. Input: assignment/read target whose nested member chain has a receiver where `declared_place_type_raw` returns `None` (e.g. `Member` receiver untyped at `4070-4071` with `?`), so `first_unknown_nested_field` returns `None` and `validate_assignment_target_handle` skips the “has no field” block at `1131-1138`.
5. **Verdict: REACHABLE** — A program with a nested member on a receiver that does not resolve via `declared_place_type_raw` can miss the dedicated spelling diagnostic while still passing other checks; `result_type` for `Member` also returns `None` without a field error when `applied_data_member_field` fails (`expression_types_and_fields.rs:5169-5171`, read), but that is a separate path from assignment-target spelling.
6. Unsure: whether all such receivers are rejected earlier with a different diagnostic (did not trace full statement validator for every receiver shape).

### A0511

1. Read `selects_collection_contents` `89-104` and `borrow_root_symbol` `215-231` in full (read); read consumers `binding_is_written` `125-147` and `binding_is_unchanged_value` `233-245` (read).
2. Search `rg 'visited < 128' /agent/repos/omega/omega-rust/psi/compilation/validation` → 2 hits (read), both in `places.rs`.
3. Search `rg 'binding_is_written' /agent/repos/omega/omega-rust/psi/compilation/validation` → multiple hits (read); read `places.rs:323` and `contract_entailment.rs`/`calls.rs` references from grep only.
4. Input: expression chains with more than 128 `Borrow`/`Member`/`Indexed`/`Name` hops before resolution; loop exits with `false` (`selects_collection_contents`) or `None` (`borrow_root_symbol`).
5. **Verdict: REACHABLE** — A binding reached only through a chain longer than 128 steps is judged not written / root not found, so `binding_is_unchanged_value` may return true (`233-238`, read) and downstream immutability reasoning (`immutable_binding` at `306-324`, read) can treat a written binding as unchanged.
6. Unsure: whether the language or parser allows 129-deep chains in practice (comment in entry: “practically unreachable”; still a code-level acceptance gap).

### A0512

1. Read `zeroed_structural_parts` in full at `places.rs:5132-5200` (read).
2. Search `rg 'zeroed_structural_parts' /agent/repos/omega/omega-rust` → 7 hits (read); read uses `execution_scalar_computations.rs:7358-7361`, `7447-7449`, `10678` (read).
3. Input: `TypeReferenceNode::Named`/`Generic` with `DataMember::Variant` entries; when `variants.next()` yields `first_case` (`5169-5170`, read), function still pushes all struct fields and only `data_payload_fields(first_case)` (`5181-5194`, read), not other variants’ payloads.
4. **Verdict: REACHABLE** — A sum type whose first variant is zero-representable but a later variant carries non-zeroable payload can still satisfy `zeroed_structural_parts`, enabling zeroed omitted fields / zeroed structural locals (`7447-7450`, `10673-10678`, read) as if the whole type were zeroable.
5. Unsure: whether runtime layout always uses first-variant tag for implicit zero init (did not read data layout rules).

### What I did not read

- Full `bind_pre_specialization_authored_selections` late-bound resolution loop after `preparation.rs:1395`.
- Full `contents_observation_survives` and surrounding case-content logic in `flow_exits.rs`.
- All 77+ `canonical_place_from_expression` call sites (only `place_relation`, `PlaceRoot`, and grep counts).
- Complete statement-typechecking path for every `Member` receiver shape in `program_validation.rs`.
- Data-layout / tag-representation specification for multi-variant `zeroed_structural_parts`.

SLICE COMPLETE

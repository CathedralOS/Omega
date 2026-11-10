slice_0063

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0481 | lsl:3557-3559 | CLOSED | Callers already hold a `DataDefinition`; the early return only fires if `data_definition_by_symbol` fails on that same symbol. |
| A0482 | lsl:2761-2778 | REACHABLE | `primitive_type_reference` failing exits before suffix comparison, so width suffixes on literals at non-primitive destinations are not diagnosed here. |
| A0483 | lsl:350 / lsl:714-726 | REACHABLE | `land_float_value_for_type` returns without landing when the declared type is not F32/F64 (read at 714-726). |
| A0484 | lsl:5259-5261 | REACHABLE | `array_window_shape` returning `None` ends validation with no diagnostic. |
| A0485 | mcc:607-634 | REACHABLE | Empty boundary edges with a valid concrete target and `Ok(None)` from `exact_direct_boundary_requirement` drops the call from boundary coordinates silently. |
| A0486 | mcc:366-400 | REACHABLE | `approved_boundary_traits` approves boundary traits on naming/uniqueness only; no provider table is consulted before `validate_boundary_calls` filters coordinates. |
| A0487 | mcc:123-215 | CLOSED | Unselected generic-data template machines are skipped by design; only `method_is_selected` machines are bound-checked at the end. |
| A0488 | mcc:1931-1933 | REACHABLE | Invalid dependency symbols are skipped, so `context_is_closed` can return true while dependencies remain unexamined. |

### A0481

1. Read `validate_literal_facts` in full at lsl:3541-3660 (read); early return at lsl:3557-3559 when `program.data_definition_by_symbol(owner)` is `None`.
2. Search `\bvalidate_literal_facts\b` in `omega-rust/`: **3 hits**, all in `literals_and_struct_literals.rs` (definition plus two call sites at lsl:3421 and lsl:3483) (read).
3. Read `validate_literal_default_domain` (lsl:3406-3437) and `validate_literal_case_constraints` (lsl:3439-3499) in full (read); both pass `data_definition.symbol` as `owner`.
4. Read construction path at lsl:5871-5898 (read): `selected_construction_definition` must return `Some(data_definition)` before `validate_literal_default_domain` runs; failure already pushes diagnostics at lsl:5874-5875.
5. Read `selected_construction_definition` at lsl:4771-4782 (read): returns `&DataDefinition` from `unique_data_definition_by_symbol` or constructor lookup, not a stale handle.
6. Read `data_definition_by_symbol` at `declaration_tables_and_index.rs:1842-1848` (read): maps symbol to the same table slice via `first_position`.
7. Did not read the rest of `validate_literal_facts` after lsl:3660 (membership/refutation arms only partially read).

**Input to accepting branch:** `owner` symbol for which the function already received a live `&DataDefinition` from the same `CheckingInput`, but `data_definition_by_symbol(owner)` returns `None` (inferred: table inconsistency or invalid symbol only).

**Verdict: CLOSED.** Every production caller holds `data_definition` from `selected_construction_definition` or the same struct passed through case validation; lookup by `data_definition.symbol` should succeed on any coherent typed program. The branch is defensive; skipping `requires` fact work requires inconsistent tables that earlier construction checks already reject.

**Unsure:** Whether any future caller could invoke `validate_literal_facts` with an `owner` not tied to a retained definition (none today).

### A0482

1. Read `validate_suffix_landing` in full at lsl:2711-2814 (read).
2. Search `\bvalidate_suffix_landing\b` in `omega-rust/`: **11 hits** in 3 files — `literals_and_struct_literals.rs` (9), `expression_types_and_fields.rs` (1), `calls.rs` (1) (read).
3. Read `validate_suffix_landings` at lsl:2816-2909 (partial: prologue scan of struct fields and statements) (read).
4. Read `program_validation.rs:380` prologue hook (read) — runs during program validation before acceptance.
5. Read `expression_types_and_fields.rs:1419-1427` (read) — anonymous comparison path calls `validate_suffix_landing` then `argument_matches_type_reference_handle`.
6. Did not read full `scalar_observations::primitive_delivery_matches` or entire `argument_matches_type_reference_using`.

**Input to accepting branch:** A suffixed integer or float literal whose declared destination unwraps to a non-primitive type (`unwrapped_type_reference` or `primitive_type_reference` is `None` at lsl:2761-2765 and lsl:2776-2778), so the function returns before suffix width checks at lsl:2780-2812.

**Verdict: REACHABLE.** A program can place a width-suffixed numeric literal into a named non-primitive or otherwise non-scalar destination (for example a data type field); this validator exits without emitting a suffix mismatch diagnostic. `validate_suffix_magnitudes` (lsl:42-85) only checks fit to the suffix type, not agreement with the destination.

**Unsure:** Whether every such program is always rejected later by assignment typing in `expression_types_and_fields.rs` (not fully traced for all statement kinds).

### A0483

1. Read `land_float_literal_destinations` at lsl:350-566 (read); pair collection then `land_float_value_for_type` at lsl:560-561.
2. Read `land_float_value_for_type` at lsl:632-727 (read); non-float `Named` arms return at lsl:718; `Slice`/`Generic`/etc. are no-ops at lsl:722-726.
3. Search `\bland_float_literal_destinations\b` in `omega-rust/`: **4 hits** — definition, re-export `lib.rs`, call `preparation.rs:2061` (read).
4. Did not read full pair-collection loops over machines/operators beyond lsl:442-528.

**Input to accepting branch:** Float literal expression paired with a destination type that is not F32/F64 primitive (or structural cases that recurse to such a destination).

**Verdict: REACHABLE.** Preparation at `04_typed-trees-to-typed-trees/preparation.rs:2061` can leave float literals un-landed when destinations are integers, nominal types, slices, or generics; no diagnostic is recorded in `land_float_value_for_type`.

**Unsure:** Downstream typed validation always fails programs that depend on landed float format for those sites (not exhaustively read).

### A0484

1. Read `validate_array_window_elements` at lsl:5234-5337 (read).
2. Read `array_window_shape` at lsl:5174-5231 (read) for conditions producing `None`.
3. Search `\bvalidate_array_window_elements\b` in `omega-rust/`: **3 hits** — definition, re-export, call `program_validation.rs:1605` (read).
4. Read assignment hook at `program_validation.rs:1602-1614` (read) — runs on assignment targets in machine statement validation.
5. Did not read full `normalize_immutable_integer_bound_to_usize` or bound lookup failure modes.

**Input to accepting branch:** Assignment with an indexed target that has a range (`lsl:5247-5252` passed), but `array_window_shape` returns `None` at lsl:5259-5261 (e.g. collection type, length, or bound normalization fails per lsl:5181-5200).

**Verdict: REACHABLE.** Array-window element and width checks are skipped with no diagnostic when shape cannot be built; a program with an ill-typed window replacement and unresolvable bounds could pass this validator.

**Unsure:** Overlap with `validate_array_literal_elements` at `program_validation.rs:1616-1627` for the same assignment (read call sites only).

**Note:** Silent return at lsl:5247-5252 when the index has no range node is intentional (not a window); entry’s cited 5247-5262 includes that guard, but the ACCEPT concern is the `shape` `None` path at lsl:5259-5261.

### A0485

1. Read `checked_boundary_call_coordinates` loop at mcc:577-634 (read); branches at mcc:607-609 and mcc:617-633.
2. Read `exact_direct_boundary_requirement` at mcc:699-729 (full) (read).
3. Search `\bexact_direct_boundary_requirement\b` in `omega-rust/`: **2 hits** in `machine_calls_core.rs` (read).
4. Read `validate_boundary_calls` at mcc:348-364 (read) — sole consumer of coordinates via `checking.rs:1394`.
5. Read `append_call_boundary_edges` at `flow.rs:5853-5882` (partial) for when edges stay empty (read).
6. Did not read remainder of boundary-edge validation loop after mcc:636.

**Input to accepting branch:** (a) Flow call with invalid `target_symbol` and empty `boundary_edges` — `continue` at mcc:607-609; (b) valid target, empty edges, `exact_direct_boundary_requirement` → `Ok(None)` at mcc:630 — no coordinate, no error.

**Verdict: REACHABLE.** A checked-flow call row with lost or missing boundary edges and a concrete implementation target is not added to `coordinates`; `validate_boundary_calls` never sees it, so boundary approval is not applied to that call. The digest’s “concrete implementation” case matches (b).

**Unsure:** Whether typed-phase `calls.rs` boundary machinery always rejects the same call independently (not traced end-to-end).

### A0486

1. Read `approved_boundary_traits` at mcc:366-400 (full) (read) — filters `is_boundary` traits with unique symbol/name in module and package.
2. Read `validate_boundary_calls` at mcc:348-364 (read) — rejects only calls whose `boundary_trait_symbol` is not in `approved`.
3. Search `\bapproved_boundary_traits\b` in `omega-rust/`: **2 hits** (def + use) (read).
4. Read `is_boundary_provider` at `value_custody_core.rs:10476-10481` (read) — separate helper, not used in `approved_boundary_traits`.
5. Did not read provider admission in `terminal-installation` or full `boundary_provider_qualification_obligations`.

**Input to accepting branch:** Boundary trait symbols that pass uniqueness (`has_exact_identity` at mcc:391-397) enter `approved`; any boundary call coordinate bearing that trait passes `validate_boundary_calls` regardless of whether a provider exists.

**Verdict: REACHABLE.** The diagnostic text at mcc:357 references “approved provider”, but approval at mcc:366-400 never reads a provider table; uniquely named boundary traits are treated as approved capabilities.

**Unsure:** Whether `checks_contracts_exits.rs:4815+` always blocks programs that call boundary capabilities without providers (only read call site).

### A0487

1. Read `validate_selected_attached_method_bounds` at mcc:120-303 (full) (read); fixpoint at mcc:157-218 skips machines where generic template is valid but `method_is_selected` is false (mcc:160-164).
2. Final check only for machines with template valid and selected at mcc:221-224 (read).
3. Search `\bvalidate_selected_attached_method_bounds\b` in `omega-rust/`: **3 hits** — def, re-export, `checking.rs:1039` (read).
4. Read `method_is_selected` at mcc:305-315 (read).
5. Did not prove completeness of the `selected` set against all invocation paths.

**Input to accepting branch:** Machine with `generic_data_template.is_valid()` that never enters the `selected` set during the fixpoint (no symbol/state reachability from constants, proof-output calls, or selected-machine traversal).

**Verdict: CLOSED.** Such machines are never bound-checked here; they are unreachable from the selection closure used for this validator. Selected machines get full bound validation at mcc:271-299.

**Unsure:** Dynamic or untracked selection paths that could call an unselected generic-data method without inserting its symbol into `selected`.

### A0488

1. Read `context_is_closed` at mcc:1919-1953 (full) (read); invalid symbols skipped at mcc:1931-1932 before kind inspection.
2. Read `static_interrupt_context` at mcc:1606-1666 (read); uses `context_is_closed` at mcc:1650; admission when `closed` is true at mcc:1657-1665.
3. Search `\bcontext_is_closed\b` in `omega-rust/`: **2 hits** in `machine_calls_core.rs` (read).
4. Read `collect_type_symbol_dependencies` at `references.rs:1182-1190` (read) — supplies dependency symbols.
5. Did not read all callers of `static_interrupt_context` in `calls.rs` / `borrow_and_reflection.rs` in full.

**Input to accepting branch:** Type referent whose dependency list includes invalid `SymbolHandle` values; those entries are ignored, so open parameters or locals referenced only through invalid handles do not force `false`.

**Verdict: REACHABLE.** `context_is_closed` may return true while dependencies are incomplete; `static_interrupt_context` can admit an installed context (`Ok(Some(...))` at mcc:1660-1665) that should remain inadmissible if invalid symbols concealed open context.

**Unsure:** Whether invalid type dependency symbols can appear on well-formed typed programs reaching validation (pipeline invariant not verified).

### What I did not read

- Full `validate_literal_facts` after membership handling (~lsl:3662+).
- Full `scalar_observations::primitive_delivery_matches` and all `argument_matches_type_reference_using` branches.
- Full `checked_boundary_call_coordinates` after mcc:636 and all boundary-edge error paths.
- Full `calls.rs` boundary-call validation parallel to `validate_boundary_calls`.
- Full `boundary_provider_qualification_obligations` and contract-exit refutation logic.
- Corpus or samples demonstrating reachability for any REACHABLE verdict (read-only slice; no build/run).

SLICE COMPLETE

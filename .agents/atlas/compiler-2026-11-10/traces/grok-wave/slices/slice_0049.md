slice_0049

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0373 | omega-rust/psi/compilation/validation/src/calls.rs:33194-33203 | CLOSED | Empty complete transfer is gated on builtin/asm, preserves-reference-slots, or static-intrinsic classification only. |
| A0374 | omega-rust/psi/compilation/validation/src/calls.rs:38739 | REACHABLE | `bind_formal_type` returns true on shape mismatch while `transition_signature_type_bindings` can still succeed if every type parameter is already bound. |
| A0375 | omega-rust/psi/compilation/validation/src/calls.rs:38952-38957 | REACHABLE | `CallArgument`/`IndexOperand` bypass return-type and isolation checks before call-site validation runs. |
| A0376 | omega-rust/psi/compilation/validation/src/calls.rs:39238 | CLOSED | Empty `Some(Vec::new())` is intentional proved-no-write for designated compiler/runtime operations; callers treat `None` as failure. |
| A0377 | omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs:1097-1099 | CLOSED | `immutable_image` rows are defined to contribute no backing places; consumers treat that as the profile semantics. |
| A0378 | omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs:1630-1634 | REACHABLE | Failed or ambiguous member mapping falls back to the authored segment and projection continues instead of failing. |
| A0379 | omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs:2244-2254 | CLOSED | Empty-path fallback applies only to reference-view captures after descriptor resolution fails; otherwise `Unavailable`. |
| A0380 | omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs:5468 | CLOSED | Name fallback matches the same parameter subject already fixed by symbol at normalization; it does not admit a different parameter root. |

### A0373

1. Read `invocation_reference_effects` in full at `calls.rs:33180-33312` (read): early returns `SlotRelationTransfer::complete(Vec::new())` for asm/`IntegerEmbed` builtins (`33194-33203`), `call_preserves_reference_slots` (`33266-33267`, defined `machine_calls_core.rs:2707-2721`), and non-`BuildProviderSelection` static intrinsics (`33286-33292`).
2. Search `rg 'invocation_reference_effects'` under `omega-rust/`: **2 hits**, both in `calls.rs` (definition `33180`, sole caller `invocation_effects` `33151`).
3. Read `invocation_effects` (`33137-33177`), `call_effects` (`33104-33135`), and install site `calls.rs:31538-31565` (read): `None` from reference effects aborts with `?` at `33164`; `None` from `call_effects` becomes `SlotRelationTransfer::unknown` at `31548-31559`, not success.
4. Input on accepting branches: a call whose `target` is a classified asm builtin, `IntegerEmbed`, a call that `call_preserves_reference_slots` marks observe-only, or a static intrinsic other than `BuildProviderSelection`.
5. **Verdict: CLOSED.** Reason: every empty-complete path is keyed on `builtin_function_for_symbol`, `call_preserves_reference_slots`, or `static_call_intrinsic` (`calls.rs:33194-33292`, `machine_calls_core.rs:2707-2721`); unclassified calls get `None` and unknown transfer at `31548-31559`, not silent acceptance. Entry description matches the read code.

### A0374

1. Read `bind_formal_type` in full at `calls.rs:38684-38788` (read): on generic actual shape mismatch returns `true` at `38738-38739`; base symbol mismatch `38741-38742`; `FixedArray`/`Slice` non-match `38770`, `38784`; default `_ => true` at `38787`.
2. Search `rg '\bbind_formal_type\b'` under `omega-rust/`: **8 hits**, all in `calls.rs` (definition plus internal recursion and two enforcement sites `20909`, `38664`).
3. Read `transition_signature_type_bindings` (`38636-38681`) and `applied_call_write_signature` (`38551-38579`) (read): `TransitionNamed` uses bindings from `transition_signature_type_bindings`; failure is `None` at `38671` only when `bind_formal_type` returns `false`, not when it returns `true` on mismatch; final guard requires every type parameter symbol appear in `bindings` (`38674-38680`). Read `boundary_written_argument_paths` (`8159-8214`) (read): uses `substituted_head` with those bindings for write-path planning at `8373-8398`.
4. Input: a `TransitionNamed` requirement/boundary call whose one argument’s actual type shape does not match the formal parameter type, while every type parameter symbol is already present in `bindings` from other arguments.
5. **Verdict: REACHABLE.** Reason: a transition call can supply a non-matching actual for one parameter (for example formal generic type versus unrelated actual shape at `38738-38739`), still get `true`, pass the “all parameters bound” scan if type parameters were bound elsewhere, and return `Some(bindings)` at `38676-38678` so write-frame planning proceeds. `validate_referent_projection` at `20919` is only on the transparent-origin path, not this transition binding path. Unsure: whether an earlier typing stage always rejects the mismatched actual before validation; not traced past `CheckingInput` into typed-trees.

### A0375

1. Read `value_call_result_is_admitted` in full at `calls.rs:38946-38978` and `ValuePosition` helpers `38820-38847` (read).
2. Search `rg '\bvalue_call_result_is_admitted\b'` under `omega-rust/`: **3 hits** in `calls.rs` (import/use `9604`, check `9846-9849`, definition `38946`).
3. Read `complete_expression_tree` (`9757-9873`) and caller `parameter_relative_expression_preserves_transparent_result` (`9709-9754`) plus indexed use `18212-18224` (read): for `ExpressionNode::Call`, admission at `9846` must succeed; `CallArgument`/`IndexOperand` return `true` immediately at `38952-38956` without reading callee return type or `CallResultRequirement`.
4. Input: an effectful call expression visited as `ValuePosition::IndexOperand` (or `CallArgument`) during transparent-result / write-frame tree walks.
5. **Verdict: REACHABLE.** Reason: a callee whose result is a shared reference or not caller-isolated can pass `value_call_result_is_admitted` when used as an index operand, because `IndexOperand` skips the checks at `38970-38977` that apply to `ComputedOperand`/`MemberReceiver`. That can let `parameter_relative_expression_preserves_transparent_result` continue (`18212-18224`) where a stricter return-type check would reject. Entry matches read code.

### A0376

1. Read `known_call_written_paths_for_parts` branches at `calls.rs:39227-39259` inside function body starting `39196` (read); did not read full 39196–end.
2. Read immediate caller closure at `calls.rs:15634-15690` (read): `written.is_empty()` returns `Some(written)` as proved empty at `15679-15680`; `None` from the `?` chain is failure at `15692-15699`.
3. Search `rg 'known_call_written_paths_for_parts'` under `omega-rust/`: **14 hits**, all in `calls.rs`.
4. Input: calls classified as GDT `Load`, activation/accessor empty-write targets, or flag-retaining asm builtins.
5. **Verdict: CLOSED.** Reason: empty vector is explicit “no caller writes” for compiler-known operations (`39237-39238`, `39247-39250`, `39252-39259`); incomplete or unrelated origins still yield `None` (`15682-15683`). Entry accurate; comment at `15676-15678` matches behavior.

### A0377

1. Read `resolve_profile` at `completed_storage_and_conservation.rs:1069-1124` and branch at `1013-1014` in an enclosing `places` resolution loop (read).
2. Read consumer `profile_backing` → `resolve` path at `931-934`, `1050-1066` (read).
3. Search: no separate symbol; cited lines match at commit line numbers (read `1097-1099`, `1013-1014`).
4. Input: operand retention profile rows with `immutable_image` set true from flow facts.
5. **Verdict: CLOSED.** Reason: `immutable_image` intentionally yields `ProfileBacking::Known(Vec::new())` at `1097-1098` and is skipped without backing at `1013-1014`; this is the defined “no backing places” semantics for immutable images, not a silent skip of a missing diagnostic. Entry matches.

### A0378

1. Read `project_captured_storage_place` in full at `completed_storage_and_conservation.rs:1610-1640` (read).
2. Search `rg 'project_captured_storage_place'` under `omega-rust/`: **6 hits** (`completed_storage_and_conservation.rs` definition plus internal uses `2106`, `2339`, `2607`; pipeline `flow_expression.rs:2408`, `flow_call_phases.rs:568`). Did not read pipeline callers.
3. Read one internal caller context at `2106-2117` (read): failed projection yields `Selection::Unavailable`, but wrong-segment projection may still succeed.
4. Input: field or case `PlaceSegment` where `canonical_place_type_reference` / `corresponding_place_projection` does not yield a unique mapped segment.
5. **Verdict: REACHABLE.** Reason: `unwrap_or(*segment)` at `1634` keeps the unmapped authored segment and continues the loop; a storage snapshot walk can then resolve storage along a path that member correspondence did not validate. Unsure: whether downstream snapshot logic always fails closed on the wrong place; pipeline callers not read.

### A0379

1. Read `Resolver::select` from `2013` through `2270` (read), focusing on `2244-2253`.
2. Prior guard at `2238-2239` returns `Unavailable` when `profile_descriptor` fails with empty path inside the retention block (read).
3. Input: reference-view operand (`reference_shell` Some), empty projection path, no usable retention descriptor/alternatives (block `2230-2242` skipped), after earlier paths did not select.
4. **Verdict: CLOSED.** Reason: `2244-2253` is a deliberate last resort returning the capture itself as a `StoredCapture` source, not a compile-success bypass; other failure modes return `Selection::Unavailable` (`2189`, `2239`). Entry description matches read control flow.

### A0380

1. Read `structural_place_matches` at `5459-5471` and caller `contracts_establish_domain` `5412-5456` (read).
2. Read subject construction `5017-5054` (read): parameter roots require `root_symbol.is_valid() && parameter.symbol == root_symbol` before a subject is built.
3. Search `rg '\bstructural_place_matches\b'` under `omega-rust/`: **2 hits** in `completed_storage_and_conservation.rs`.
4. Input: contract membership fact whose collected root has matching parameter **name** but a **symbol** handle that does not equal the subject parameter’s symbol.
5. **Verdict: CLOSED.** Reason: the `name == root_name` disjunct at `5468` only supplements facts that spell the same parameter already chosen by symbol at `5039-5040`; it does not match a different parameter root because normalization already fixed `subject.root` to one parameter. Entry is literally true at `5468` but does not open a distinct acceptance path for the wrong parameter. Unsure: ill-formed IR with duplicate parameter names; not investigated.

### What I did not read

- Full body of `known_call_written_paths_for_parts` below `39260` and most of its 14 call sites except `15634-15690` and references in `complete_expression_tree`.
- `invocation_reference_effects` continuation after line `33312` and downstream `apply_transfer` / slot-relation validation beyond `install_statement_call_effects` (`27363-27377`).
- Pipeline uses of `project_captured_storage_place` in `flow_expression.rs` and `flow_call_phases.rs`.
- Whether typed-trees typing rejects programs before validation for A0374/A0375 cases.
- Tests under `omega-rust/` for these paths.

SLICE COMPLETE

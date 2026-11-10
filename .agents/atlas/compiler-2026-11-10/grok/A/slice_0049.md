# slice_0049

Area digest preamble (read): `digests/area_13.md` lines 1–9 — validation first half under `omega-rust/psi/compilation/validation/`; abbreviations `calls` = `src/calls.rs`, `cs` = `src/completed_storage_and_conservation.rs`; digest commit `457adf4a11f`, cited line ranges matched in current checkout (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0373 | calls:33194-33203 | REACHABLE | Builtin asm/`IntegerEmbed` and related early exits return complete empty slot-reference transfer without modeling effects. |
| A0374 | calls:38739 | REACHABLE | `bind_formal_type` returns `true` on generic/array/slice shape mismatch, so type-parameter binding can succeed anyway. |
| A0375 | calls:38952-38957 | REACHABLE | `CallArgument`/`IndexOperand` admit any call result before return-type checking runs. |
| A0376 | calls:39238 | REACHABLE | Descriptor Load, accessor/borrow helpers, and flag asm report an empty write set and `complete_expression_tree` accepts. |
| A0377 | cs:1097-1099 | REACHABLE | `immutable_image` retention yields no backing places (empty `Known` or `continue`). |
| A0378 | cs:1630-1634 | REACHABLE | Field/case projection uses the raw segment when type mapping is missing or ambiguous. |
| A0379 | cs:2244-2254 | REACHABLE | Empty-path view with unresolved descriptor falls back to the capture itself as the source. |
| A0380 | cs:5468 | REACHABLE | Contract fact matching accepts parameter root by spelling even when the symbol differs. |

## A0373

**Cited sites (read):** `omega-rust/psi/compilation/validation/src/calls.rs:33194-33203` — `invocation_reference_effects` returns `SlotRelationTransfer::complete(Vec::new())` for asm intrinsics and `IntegerEmbed`; `calls.rs:33266-33267` same when `call_preserves_reference_slots`; `calls.rs:33286-33293` same for selected static call intrinsics.

**Enclosing function (read):** `invocation_reference_effects` (`calls.rs:33180-33347+`) builds slot effects or refuses with `trace_refusal` and `None`.

**Caller search (read):** pattern `\binvocation_reference_effects\(` over `omega-rust` — 1 definition, 1 production call at `calls.rs:33151` inside `invocation_effects`.

**Upward trace (read):** `invocation_effects` (`calls.rs:33137-33177`) always wraps the transfer from `invocation_reference_effects`; `call_effects` (`calls.rs:33104-33135`) and statement walk `calls.rs:31844-31858` consume it. On `None`, walk uses `SlotRelationTransfer::unknown` (`calls.rs:31858-31868`); on early `Some(complete(empty))`, effects are treated as fully known with no slot mutations.

**Input:** Call target classified as asm builtin, `IntegerEmbed`, reference-slot-preserving static call, or non–BuildProvider static intrinsic, while the callee still updates reference slots the ledger should track.

**Smallest program (plain words):** A call through a builtin asm or embed entry that mutates a stored reference slot; validation records complete slot transfer with zero effects and may accept caller isolation the call violates.

## A0374

**Cited sites (read):** `calls.rs:38737-38739` — non-generic actual paired with generic formal returns `true`; `38741-38743` — mismatched generic base returns `true`; `38770` and `38784` — non-matching `FixedArray`/`Slice` actual returns `true`; `38787` — catch-all `_ => true`.

**Enclosing function (read):** `bind_formal_type` (`calls.rs:38684-38789`) recurses on structure; only some arms return `false` via `type_references_match` failure paths.

**Caller search (read):** pattern `\bbind_formal_type\(` over `omega-rust/psi/compilation/validation` — 1 definition (`38684`), production uses at `38664`, `20909`, and 4 recursive sites (`38726`, `38755`, `38763`, `38777`); count 7 lines in `calls.rs`.

**Upward trace (read):** `transition_signature_type_bindings` (`calls.rs:38636-38682`) rejects only when `bind_formal_type` returns `false` (`38664-38671`), then requires every type parameter appear in `bindings` (`38674-38680`). Used from `applied_call_write_signature` for transition calls (`38568-38574`). Separate path at `20909-20916` binds exclusive-reference projection.

**Input:** Transition or reference argument where one formal parameter type shape does not match the actual (wrong generic instantiation, array vs scalar, etc.) but other arguments still populate all type-parameter symbols in `bindings`.

**Smallest program (plain words):** A generic transition call whose one argument type does not match the formal generic shape while remaining arguments still bind every type variable; the checker treats binding as successful and may accept the call.

## A0375

**Cited site (read):** `calls.rs:38952-38957` — `value_call_result_is_admitted` returns `true` immediately for `ValuePosition::CallArgument(_)` and `IndexOperand` without inspecting the callee return type.

**Enclosing function (read):** `value_call_result_is_admitted` (`calls.rs:38946-38979`); other positions require non-reference or caller-isolated return (`38970-38977`).

**Caller search (read):** pattern `value_call_result_is_admitted\(` over `omega-rust` — 1 definition, 1 call at `calls.rs:9846` inside `complete_expression_tree`.

**Upward trace (read):** `complete_expression_tree` (`calls.rs:9757-9997`) returns `false` when admission fails (`9846-9849`). `parameter_relative_expression_preserves_transparent_result` (`calls.rs:9709-9754`) and `parameter_relative_index_expression_preserves_transparent_result` (`calls.rs:9681`) use it; indexed-place check at `calls.rs:18212-18223` uses `ValuePosition::IndexOperand`. Call-argument children are queued with `ValuePosition::CallArgument(expected_type)` (`9919-9924`).

**Input:** Nested call used as another call’s argument or as a collection index, where the inner callee’s return type is a reference or not caller-isolated.

**Smallest program (plain words):** An effectful call nested in a call argument or index position whose return is a reference type; admission returns true without reading the return type and transparent-result preservation may accept.

## A0376

**Cited sites (read):** `calls.rs:39237-39239` — GDT `Load` returns `Some(Vec::new())`; `39247-39250` — activation accessor or access borrow empty frame; `39252-39259` — flag-retaining asm intrinsic empty paths.

**Enclosing function (read):** `known_call_written_paths_for_parts` (`calls.rs:39196-39280+`) else chains further ladders (`39262+`).

**Caller search (read):** pattern `known_call_written_paths_for_parts\(` over `omega-rust/psi/compilation/validation/src/calls.rs` — 1 definition plus production calls including `9933`, `15634`, `16275`, `27147`, `37594`, `39115`, `39366`, `39689` (read listing; 9 call sites excluding definition).

**Upward trace (read):** In `complete_expression_tree`, `CallFrame` handling requires `known_call_written_paths_for_parts(...).or_else(...).or_else(...)` not be `None` (`9933-9993`); `Some(empty vec)` satisfies that and the function returns `true` (`9996-9996`).

**Input:** Call classified as GDT load, activation accessor, access-borrow, or flag-preserving asm while the expression tree walk requires write-frame transparency.

**Smallest program (plain words):** A call through one of those builtin/accessor targets that still writes caller-visible storage; the write-frame walk records no paths and may accept the enclosing transparent expression.

## A0377

**Cited sites (read):** `cs.rs:1097-1099` (`completed_storage_and_conservation.rs`) — `profile_backing` on `immutable_image` returns `Known(Vec::new())`; `1013-1014` — in `places`, `immutable_image` hits `continue` and contributes no places.

**Enclosing functions (read):** `profile_backing` (`1050-1143`); `places` on backing `Resolver` (`892-1041`).

**Caller search (read):** pattern `profile_backing\(` in `completed_storage_and_conservation.rs` — 1 definition, 5 internal calls (`931`, `951`, `1006`, `1026`, `1081`).

**Upward trace (read):** `places` feeds `reference_backing_places` (`842-857`) and reference-capture resolution loops (`996-1035`). Empty or skipped backing reduces reported storage suppliers with no diagnostic at these sites.

**Input:** Operand retention row marked `immutable_image` where completed-storage or content validation still needs backing places for a referent.

**Smallest program (plain words):** A frozen/immutable-image operand retention in the flow facts; backing resolution returns no places and downstream conservation may treat storage as absent incorrectly.

## A0378

**Cited site (read):** `cs.rs:1617-1634` — for field/case segments, `corresponding_place_projection` mapping that is not exactly one segment falls through to `unwrap_or(*segment)`.

**Enclosing function (read):** `project_captured_storage_place` (`1610-1641`).

**Caller search (read):** pattern `project_captured_storage_place\(` over `omega-rust/psi/compilation/validation` — 1 definition, 3 production calls in same file (`2106`, `2339`, `2607`) plus re-export `2877`.

**Upward trace (read):** Used when projecting captured storage along authored paths; wrong segment choice propagates into `project_captured_view_place` (`1638`) and storage-fact queries.

**Input:** Projection through a field or case variant where type-to-place mapping is missing, ambiguous, or returns multiple segments.

**Smallest program (plain words):** A captured storage path through a field whose type correspondence does not map uniquely; the checker projects using the unmapped segment and may accept an invalid place.

## A0379

**Cited site (read):** `cs.rs:2230-2253` — for reference-shell view with empty `path`, after `profile_descriptor` fails or is inconclusive, returns `Selection::Sources` pointing at the capture itself with empty projection.

**Enclosing function (read):** `Resolver::select` (`2013-2272+`).

**Caller search (read):** pattern `resolver.select\(|self.select\(` in `completed_storage_and_conservation.rs` — multiple internal recursion sites; public entry `reference_sources_at_path` (`1879-1887`) wraps `stored_selection(resolver.select(value, path))`.

**Upward trace (read):** Earlier branches (`2191-2241`) prefer descriptor and reference-leaf data; empty-path fallback runs when those do not resolve. No diagnostic at return.

**Input:** Reference view operand with empty path whose retention descriptor does not resolve but is not rejected.

**Smallest program (plain words):** A borrow/view capture with no path suffix and an unresolved descriptor profile; storage selection treats the capture as its own source and may accept an incorrect referent.

## A0380

**Cited site (read):** `cs.rs:5467-5468` — `structural_place_matches` sets `root_matches` for parameters as `(*symbol == root_symbol && symbol.is_valid()) || name == root_name`.

**Enclosing function (read):** `structural_place_matches` (`5459-5472`); used from `contracts_establish_domain` (`5450-5454`).

**Caller search (read):** pattern `structural_place_matches\(` — single use site at `5453`; `contracts_establish_domain` called from content projection normalization (`5080-5086`).

**Upward trace (read):** When type lacks domain qualification, `contracts_establish_domain` can still satisfy projection via membership facts (`5079-5086`); name match alone suffices when symbols differ.

**Input:** Callable contract membership fact naming a parameter root by the same spelling as an unrelated parameter symbol.

**Smallest program (plain words):** Two distinct parameters sharing a name spelling in different scopes; a contract fact on one parameter’s name matches the other’s structural place and may establish a domain the type does not carry.

## What I did not read

- Full bodies of `call_preserves_reference_slots`, `static_call_intrinsic`, `activation_accessor_has_empty_write_frame`, and `access_borrow_has_empty_write_frame` beyond their use in `known_call_written_paths_for_parts`.
- Downstream consumers of `SlotRelationTransfer` after `invocation_effects` beyond the statement walk snippet at `calls.rs:31844-31868`.
- Stage checked-tree construction that sets `immutable_image` on operand retentions.
- Corpus or sample programs proving each branch is production-reachable (would move some rows from REACHABLE to CLOSED or UNDECIDED).

SLICE COMPLETE

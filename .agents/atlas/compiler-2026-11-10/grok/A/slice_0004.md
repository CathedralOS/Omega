# slice_0004

Digest preamble read: `digests/area_08.md` lines 1–12 (checkout note at `457adf4a11f`; path shorthand = basename under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` except stage-04 files). Area 05 borrow-check cluster described at `digests/area_08.md:47–51`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0025 | checks_borrows.rs:439 | REACHABLE | `DeclarationLifetimeFrontier::Incomplete` makes `frontier_requires_application` return false, so states/signatures are not deferred and escape skips const by-ref parameter roots at :555. |
| A0026 | checks_borrows.rs:1447 | REACHABLE | Empty `may_write` list returns without pruning `static_persistent_paths`, so wrong empty frames keep every entry path. |
| A0027 | checks_borrows.rs:5393 | REACHABLE | Ambiguous staged completion with no copied borrow-carrying operand `continue`s before operand capture, so that call’s staged borrow conflict path is omitted. |
| A0028 | checks_borrows.rs:5502 | UNDECIDED | Early `return` skips all statement borrow checks when typed machine/state lookup fails though a borrow row exists. |
| A0029 | checks_borrows.rs:11434 | CLOSED | Each `FlowStateFact` is built with the matching `StateBorrowFact` (`flow.rs:4841`), so `borrow_state?` is absent only if lookup desyncs, not for normal checked programs. |
| A0030 | checks_borrows_calls.rs:1079 | REACHABLE | Later-operand exclusive overlap check is skipped when the receiver place is a `Symbol` root and the receiver expression is not a `Borrow` node. |
| A0031 | checks_borrows_overlap.rs:2828 | UNDECIDED | Unhandled segment-kind pairs fall through to `_ => false` (non-overlap); did not trace whether mixed kinds can appear at one zip index. |
| A0032 | checks_borrows_overlap.rs:2989 | UNDECIDED | `independent_input_permissions` forces `non_interfering` with no comment or upstream proof at the site. |

## A0025

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_borrows.rs:439`, `:446`, `:555`. Line numbers match the digest at this checkout (not relocated).

**Enclosing functions (read in full):**
- `frontier_requires_application` (`checks_borrows.rs:430–452`)
- `defer_signature_frontier` (`checks_borrows.rs:398–428`)
- `check_view_return_elision` (`checks_borrows.rs:258–355`)
- `check_view_return_escape` in `mod escape` (`checks_borrows.rs:516–599` through the `Symbol` root branch)

**Callers (search `\bfrontier_requires_application\b` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src`, count mode): 2 production call sites (read): `checks_borrows.rs:286` (state loop in `check_view_return_elision`), `checks_borrows.rs:405` (`defer_signature_frontier`).

**Upstream (read):** `check_view_return_elision` is called from `check_flow_call_borrows` (`checks_borrows.rs:11423`). `check_flow_call_borrows` is called from `check_checked_facts_recording` (`checks.rs:12059`). Failures become diagnostics via `crate::checks::finish` (`checks_borrows.rs:11504`).

**Branch behavior (read):**
- At `:438–439`, result frontier `Incomplete` ⇒ `frontier_requires_application` returns `false` (not “defer”).
- At `:446`, any parameter frontier `Incomplete` ⇒ immediate `return false`.
- In `check_view_return_elision`, when `frontier_requires_application` is false, the state is not pushed to `deferred` and does not `continue` before `resolve_view_return_source` (`:286–309`).
- In `check_view_return_escape`, for `PlaceRoot::Symbol`, parameter handling requires `!parameter.is_const` and a reference shell (`:542–551`); otherwise execution hits `continue` at `:555` without a diagnostic. Module comment at `:508–512` (read) scopes “invocation-owned” reasoning to locals and by-value inputs, not const parameters.

**Input that reaches the branch (plain words):** A machine state or signature whose declared view/lifetime frontier is still structurally incomplete (classified `Incomplete` by `declaration_lifetime_frontier`, which I did not read in full), so it never enters the deferred applied-telescope queue (`templates::check_calls` at `:354`), and/or a returned view whose storage root is a const reference parameter, which the escape loop skips at `:555`.

**Verdict rationale:** Smallest rejection missing: incomplete-frontier bodies can be accepted without deferred application checking if `resolve_view_return_source` does not surface `Ambiguous`; separately, a return borrowing const-parameter storage may pass escape without the error path used for non-const by-ref parameters.

## A0026

**Cited site (read):** `checks_borrows.rs:1447–1449`.

**Enclosing function (read in full):** `retain_static_paths_across_call_frame` (`checks_borrows.rs:1432–1460`).

**Callers (search `\bretain_static_paths_across_call_frame\b` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src`, count mode): 2 call sites (read): `checks_borrows.rs:1040`, `:1056` inside `analyze_persistent_state`.

**Upstream (read):** `analyze_persistent_state` is used from `check_persistent_borrow_assignments` (`checks_borrows.rs:659+`, loop over machines/states). That runs in `check_flow_call_borrows` (`checks_borrows.rs:11425`).

**Branch behavior (read):** After resolving `written` from the closure, if `written.is_empty()` at `:1447–1448`, the function returns `false` and does not `retain`/`clear` paths—prior `static_persistent_paths` are unchanged. Non-empty `written` triggers overlap-based `paths.retain` (`:1450–1458`).

**Input that reaches the branch (plain words):** A state with machine persistent fields where a statement or call’s `may_write_paths` frame resolves to an empty list (no listed write paths), while in fact persistent storage is still invalidated or aliased by that invocation.

**Verdict rationale:** If the frame resolver under-approximates writes, every static persistent path is kept and downstream persistent-assignment checking (`check_persistent_borrow_assignments`) may accept assignments inconsistent with real writes.

## A0027

**Cited site (read):** `checks_borrows.rs:5393–5408` (digest end `:5408`; refusal diagnostic ends `:5405`).

**Enclosing function (read in full):** `staged_values::check` (`checks_borrows.rs:5342–5434`), including inner loop through `conflicts::check` at `:5423`.

**Callers (search `staged_values::check` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_borrows.rs`, count mode): 1 production call site (read): `checks_borrows.rs:5810` inside `check_statement_borrows`.

**Upstream (read):** `check_statement_borrows` (`checks_borrows.rs:5486+`) is invoked from `check_flow_call_borrows` (`checks_borrows.rs:11469`) only for pairs that already passed `borrow_state?` at `:11435`.

**Branch behavior (read):** For each flow call on the statement, the code pairs `occurrences` with invocation identity (`:5388–5392`). If there is not exactly one completion index (`:5393–5397`), it either pushes `refusal` when a copied operand is borrow-carrying (`:5394–5405`) or `continue` (`:5407`), skipping `captures::append` for that call. `conflicts::check` still runs for the statement (`:5423`) but only over leaves that were appended.

**Input that reaches the branch (plain words):** A statement whose ownership schedule gives zero or multiple completion indices for a call’s invocation identity, with operands that are not both owned-copy and borrow-carrying, while staged mutable/exclusive borrows on that call would conflict if captured.

**Verdict rationale:** The call’s operands never enter `leaves`, so staged copy/conflict checking for that call is bypassed unless the narrow copied-operand refusal triggers.

## A0028

**Cited site (read):** `checks_borrows.rs:5502–5509`.

**Enclosing function (read in full):** `check_statement_borrows` (`checks_borrows.rs:5486–5600+` through the early returns and statement loop start).

**Callers (search `\bcheck_statement_borrows\b` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src`, count mode): 1 production call site (read): `checks_borrows.rs:11469` (import at `:11379`).

**Upstream (read):** Same `check_flow_call_borrows` path as A0029; diagnostics aggregated at `:11500–11504`.

**Branch behavior (read):** If `find_state_in_machine` or `machine_by_symbol` returns `None`, the function returns immediately (`:5502–5509`) with no diagnostics—no statement, aggregate, or per-state borrow checks run for that invocation.

**Input that reaches the branch (plain words):** A `FlowStateFact` row whose `machine_symbol`/`state_symbol` pair does not resolve in the typed program table while borrow checking still runs with a `StateBorrowFact` for that pair (caller at `:11469` always passes `borrow_state` after `:11435`).

**Verdict rationale:** I did not read a proof that typed lookup can fail while borrow and flow rows still exist for the same symbols on successful compilation; nor did I read an alternate check that substitutes for the skipped body.

## A0029

**Cited site (read):** `checks_borrows.rs:11434–11435`; companion `checks_borrows_calls.rs:1497`.

**Enclosing functions (read in full):**
- `check_flow_call_borrows` (`checks_borrows.rs:11381–11507`), especially `state_flows` construction (`:11400–11411`) and `par_iter` `filter_map` (`:11432–11435`).
- `calls::check_compatibility` (`checks_borrows_calls.rs:1483–1580`), `filter_map` at `:1496–1497`.

**Callers (read):** `check_compatibility` from `check_flow_call_borrows:11413`. `check_flow_call_borrows` from `checks.rs:12059`.

**Coupling (read):** `build_flow_facts` appends each `FlowStateFact` with `borrow_state` taken from borrow facts for the same machine state (`flow.rs:4830–4867`, parameter `borrow_state` at `:4841`). `build_borrow_facts` appends one `StateBorrowFact` per `program.machine_states` on success (`borrow.rs:9458–9467`, `:8799–8808`).

**Branch behavior (read):** `borrow_state?` in both `filter_map` arms drops the entire per-state parallel work (statement borrows, aggregates, call compatibility) when the optional is `None`.

**Input that reaches the branch (plain words):** Only if `borrow_by_state.get(&(machine_symbol, state_symbol))` is `None` for a row in `facts.flow.control.states`.

**Verdict rationale:** For programs that reach borrow checking with successfully built borrow and flow facts, every flow state row is created alongside its borrow row in the same state walk, so the optional should be `Some` and the skip is not on the normal acceptance path. I did not read failure/retry paths in `published_facts.rs` that might desync tables without aborting the check pass.

## A0030

**Cited site (read):** `checks_borrows_calls.rs:1079–1085`.

**Enclosing function (read in full):** `check_exclusive_place_use` (`checks_borrows_calls.rs:1016–1120+` through the guarded block).

**Callers (search `\bcheck_exclusive_place_use\b` in `omega-rust`, count mode): 1 production call site (read): `checks_borrows_calls.rs:1669` from `check_build_selection_receiver` (`:1588+`).

**Upstream (read):** `check_build_selection_receiver` is called from `check_compatibility` (`checks_borrows_calls.rs:1548–1557`). Earlier guards still run receiver writability and entry-constraint loan interference (`:1041–1074`, read).

**Branch behavior (read):** The later-operand access collection and `captured_place_compatibility` block (`:1087–1119+`) runs only when `later_operand.is_valid()` and the receiver is a `Borrow` expression or `place_at(...)` is not a `PlaceRoot::Symbol` (`:1075–1085`). If the receiver is a symbol-rooted non-`Borrow` expression, the whole later-operand block is skipped.

**Input that reaches the branch (plain words):** A Build product exclusive operation whose receiver expression resolves to a local or parameter `Symbol` root (not wrapped in `Borrow`), with a valid later operand that would overlap active loans if checked.

**Verdict rationale:** Receiver-footprint and constraint checks still run, but the later-operand overlap path that the digest flags is explicitly gated off for symbol-root receivers.

## A0031

**Cited site (read):** `checks_borrows_overlap.rs:2828` (`_ => false` in `place_segment_pair_may_overlap`).

**Enclosing functions (read in full):**
- `place_segment_pair_may_overlap` (`checks_borrows_overlap.rs:2715–2830`)
- `place_segments_may_overlap_evaluated` (`:2688–2713`)
- `place_segments_compatibility` (`:2665–2686`)

**Callers (search `\bplace_segments_compatibility\b` in `checks_borrows_overlap.rs`, count mode): 4 definition/use sites in-file (read): `:2665` (def), `:2866`, `:2919`, `:2963`, `:2976` via `captured_place_compatibility` and `storage_path_compatibility`.

**Upstream (read):** `captured_place_compatibility` (`:2924–3015`) feeds borrow conflict judgments (`checks_borrows.rs` call sites per count in module). `static_place_segment_pair_equal` (`validation/src/places.rs:476–502`, read) returns `None` for unlike kinds (e.g. `Field` vs `Case`), delegating to the match that ends in `_ => false`.

**Branch behavior (read):** For segment pairs not handled earlier in the match (after `static_place_segment_pair_equal` is `None`), overlap is treated as false—paths are disjoint at that depth.

**Input that reaches the branch (plain words):** Two captured places whose segment sequences at some index pair unlike segment kinds that fall into the default arm (not `OpaqueRange`, range/index mixes already listed, etc.).

**Verdict rationale:** Digest marks this as inferred non-occurrence. I did not read borrow place formation far enough to confirm such pairs cannot be recorded at one depth; if they can, overlap would be understated.

## A0032

**Cited site (read):** `checks_borrows_overlap.rs:2989–2992`.

**Enclosing function (read in full):** `captured_place_compatibility` (`checks_borrows_overlap.rs:2924–3015`).

**Callers (search `\bcaptured_place_compatibility\b` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src`, count mode): 16 total matches across `checks_borrows.rs` (10), `checks_borrows_calls.rs` (3), `checks_borrows_overlap.rs` (3) (read from ripgrep count output).

**Branch behavior (read):** `independent_input_permissions` is true when roots are valid, no descriptor load on either side, at least one valid `incoming_reference`, and families differ or owner paths do not overlap (`:2989–2992`). It sets `non_interfering` true at `:3011–3013` even when `disjoint` is false and segment overlap may remain.

**Input that reaches the branch (plain words):** Two call-related captured places with distinct incoming-reference identities, no descriptor-load origin, where owner or segment geometry still overlaps but the flag treats them as non-interfering.

**Verdict rationale:** No stated invariant at the site explains why independent incoming references cannot alias the same storage; I did not trace incoming-reference construction or contract proofs that would close this.

## What I did not read

- `borrow/view_link` implementations of `declaration_lifetime_frontier`, `resolve_view_return_source`, and `resolve_applied_signature_view_return_source` beyond signatures referenced at `checks_borrows.rs:35`.
- `mod templates` / `check_calls` (`checks_borrows.rs:57+`) deferred-signature discharge.
- `CallFrameResolver` `may_write_paths` / `statement_value_may_write_paths` accuracy (frames behind closures at `checks_borrows.rs:1047–1062`).
- Full `conflicts::check` body (`checks_borrows.rs:5214+`) beyond the call at `:5423`.
- `omega-rust/psi/compilation/validation/src/places.rs` place-building paths that populate `PlaceSegment` sequences.
- Incoming-reference and descriptor-load origins in `borrow.rs` for A0032.
- `git show 457adf4a11f:` diff for line drift (citations matched at read on current tree).

SLICE COMPLETE

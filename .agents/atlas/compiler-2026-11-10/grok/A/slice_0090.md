# slice_0090

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0688 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9530-9542 | REACHABLE | `moving` returns `Ok` without frontier when root is not `Owned` or the operand path leaves inline storage. |
| A0689 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9578-9580 | REACHABLE | `observing` returns `Ok` when `structural_source_contract` access is not `Owned`, with no alternate ownership check in this function. |
| A0690 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9609-9622 | REACHABLE | `observing` stops at the first prefix that leaves inline storage and never runs `frontier` on deeper path segments. |
| A0691 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9730 | REACHABLE | `observations` `_ => Ok(())` skips `observing` for `StructuralLeafCopy` paths other than `Ordinary` handled above. |
| A0692 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9799-9816 | REACHABLE | `install_store` always removes the value's claims but reinserts them only when the destination window is owned inline storage. |
| A0693 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:9943-9947 | REACHABLE | Content-entry claims on non-parameter roots are stored with `input: None`, so `claim.input == Some(place)` filters miss them. |
| A0694 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:2340-2341 | CLOSED | Unit validation builds the view catalog with `preserves_boundary` fixed `false` and `Views` slot kind, so this disjunct does not gate acceptance. |
| A0695 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:2242-2272 | REACHABLE | Uncatalogued `CallStructural` can leave `may_escape` false when `closed` is true even though `record_escaped_invocation` still runs. |

## A0688

**Trace (read).** Opened `moving` at `current_ownership.rs:9525-9567`. After loading `structural_source_contract`, lines `9534-9541` return `Ok(())` when `root.access != StructuralAccess::Owned` or `projection_stays_in_inline_storage` is false for `argument.path`. Otherwise partial custody and `frontier` run.

**Callers.** Search `\bmoving\(` across `omega-rust`: count **3** (one definition, two production call sites). Call sites: `current_ownership.rs:7913` (call/boundary argument loop: only when `argument.access == StructuralAccess::Owned`) and `current_ownership.rs:8585` (`ReturnStructural` with synthetic `Owned` access on `source`).

**Input path.** CFG replay in `validate_current_ownership_cfg` reaches each node; call-like operations hit `7911-7916`, returns hit `8585`.

**Verdict.** **REACHABLE.** A machine can reach `moving` with an `Owned`-access operand whose source contract is not `Owned` (e.g. structural return from a borrowed root at `8585`), or with an owned inline operand whose path leaves inline storage, and `moving` accepts without `frontier`. No enclosing check in `moving` closes those branches (read `9534-9541`).

## A0689

**Trace (read).** Opened `observing` at `current_ownership.rs:9569-9628`. Lines `9578-9579` return `Ok(())` when `root.access != StructuralAccess::Owned` before partial-custody or prefix iteration.

**Callers.** Search `\bobserving\(` in `optimization-unit/semantics`: count **16** (one definition, fifteen internal calls including `current_ownership.rs:7915` for non-`Owned` call arguments).

**Input path.** Same replay as A0688; non-`Owned` operands at calls use `observing` at `7915`; many observation operations call it from `observations`.

**Verdict.** **REACHABLE.** Observation or call replay can supply a place whose contract access is not `Owned`; `observing` accepts immediately with no further ownership proof in this function (read `9578-9579`).

## A0690

**Trace (read).** In `observing`, loop `9604-9626` walks prefixes. When `projection_stays_in_inline_storage` is false at `9609-9614`, the function returns `Ok(())`. When `runtime_structural_path_tip` is `None` at `9616-9621`, it also returns `Ok(())`. Only when `selected.owns_linear_claim || length == path.len()` does it call `frontier` (`9623-9624`).

**Callers.** Same as A0689 (`observing`).

**Input path.** Operand path with an early inline prefix then an indirection below: iteration exits at `9614` before validating claim frontier on the full path or tail.

**Verdict.** **REACHABLE.** Indirected paths can pass `observing` without any `frontier` check on segments below the first non-inline prefix (read `9609-9614`).

## A0691

**Trace (read).** Opened `observations` at `current_ownership.rs:9630-9732`. `StructuralLeafCopy` with `StructuralCopyPath::Ordinary` calls `observing` at `9672-9676`. The fallback at `9730` is `_ => Ok(())`.

**Callers.** Search `\bobservations\(` in semantics: count **2** (definition plus `current_ownership.rs:7698` in CFG node replay, before `apply_operation`).

**Related (read).** `apply_operation` rejects non-`Ordinary` `StructuralLeafCopy` when the result carries a reference (`current_ownership.rs:6414-6423`). `unit_validation.rs:8283-8350` validates `StructuralLeafCopy` earlier in `validate_function` (`9647` precedes ownership at `9660`). Those checks are structural, not the same as `observing`/`frontier`.

**Input path.** A `StructuralLeafCopy` with `StructuralCopyPath::Case(_)` on a non-reference carrier reaches `7698`; `observations` matches no arm and returns `Ok` at `9730`.

**Verdict.** **REACHABLE.** For case-path leaf copies without reference results, ownership replay does not call `observing`, while the fall-through arm accepts the operation (read `9730`, `9672-9676`).

## A0692

**Trace (read).** Opened `install_store` at `current_ownership.rs:9754-9828`. `owned` is false when `destination.parameter().is_some()` (`9775-9780`). Loop `9798-9816` removes each claim tied to `value.place`; reinsertion at `9800-9815` runs only when `owned` is true.

**Callers.** Search `\binstall_store\(` in `omega-rust`: count **2** (definition; `current_ownership.rs:8427` for `StoreStructuralField`).

**Input path.** Store into a borrowed parameter place with a structural value that holds live claims: claims are stripped and not restored.

**Verdict.** **REACHABLE.** Borrowed-parameter stores can clear the source value's claims without reinsertion (read `9798-9816`, `9775-9780`).

## A0693

**Trace (read).** `reconstruct_entry_ownership` at `current_ownership.rs:9941-9947` inserts content-entry claims with `input: parameter.map(|_| claim.input.root)`; when `structural_parameter(claim.input.root)` is `None`, `input` is `None`.

**Consumers (read).** `frontier` filters `claim.input == Some(place)` at `9503-9506`. Many other scans use the same filter (e.g. `9503`, `9784`, `8063`). Some paths also use `function.content_entry_claim_declared(place)` (e.g. `9374`, `3421`), which keys on root place, not `LiveClaim.input`.

**Input path.** Entry ownership is built before CFG replay (`9856`); claims with `input: None` persist in `CurrentOwnership.claims`.

**Verdict.** **REACHABLE.** Checks that only match `claim.input == Some(place)` do not see those claims; whether every obligation is covered elsewhere was not fully traced for all consumers (read `9943-9947`, `9504`).

## A0694

**Trace (read).** In `descriptor_results.rs`, `BoundaryCall` handling sets `preserved` at `2315-2341`; line `2341` ORs `(slot_kind.tracks_references() && preserves_boundary(function, node))`. When `!preserved`, `step.may_escape = true` (`2342-2343`).

**Callers of custom `preserves_boundary` (read).** `optimized_reference_catalog_graph_with_boundary_preservation` at `descriptor_results.rs:1067-1086`. Repository call sites: `legalization_descriptor_inputs.rs:64-70` (closure calls `preserves_boundary` at `94-153`); `pipeline/02_abstract-operations-to-target-operations/src/references.rs:1536-1578` (inline closure). Default `optimized_catalog_graph` passes `|_, _| false` at `descriptor_results.rs:1042-1043`.

**Unit validation path (read).** `CallAccesses::new` uses `optimized_catalog_graph` (Views slot, implicit `false` preservation) at `current_ownership.rs:2865-2871`. `DescriptorSlotKind::Views` has `tracks_references() == false` (`descriptor_replay.rs:5734-5736`), so `2341`'s second disjunct is inactive for the view graph that feeds `validate_compatibility` (`CO:711-758`).

**Verdict.** **CLOSED** for `validate_psi_optimization_unit` acceptance: the only graph wired into ownership validation does not consult a caller-supplied `preserves_boundary` on the escaping branch (read `CO:2865-2871`, `DR:1042-1043`, `descriptor_replay.rs:5734-5736`). Custom closures affect reference-catalog builds in pipeline 02/03 only (read call sites above).

## A0695

**Trace (read).** `CallStructural` in `descriptor_results.rs:2158-2272`. When no exactly matching callee function exists, the `else` at `2242-2271` computes `closed` from unit/scalar result, empty `claim_transfers`, and `argument_storage_is_closed` on each non-shared argument (`2245-2260`). `step.may_escape |= !closed` (`2262`). `record_escaped_invocation` always runs (`2263-2271`).

**Input path.** Dynamic or missing-catalog callee with unit/scalar result and arguments classified as closed storage.

**Verdict.** **REACHABLE.** The digest's "non-escaping" reading matches `may_escape` staying false when `closed` is true, but `record_escaped_invocation` may still record unresolved content writes (`1185-1200`, `2263-2271`); whether `closed` is sound for all uncatalogued callees was not proved (read `2245-2262`).

## What I did not read

- Full body of every helper named in `apply_operation` arms for `StructuralLeafCopy` beyond `6414-6423`.
- Complete downstream use of `step.definitions` / `unresolved_contents_writes` after `record_escaped_invocation` for uncatalogued calls.
- Pipeline 02/03 legalization acceptance paths that consume reference catalogs built with custom `preserves_boundary` (only call sites at `legalization_descriptor_inputs.rs:64` and `references.rs:1536` were opened).
- Git pin `457adf4a11f`; line numbers are from the current checkout and matched the cited symbols on read.

SLICE COMPLETE

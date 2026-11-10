# Area 09: open “accepts when it does not know” entries

Read-only trace of the classed-open bullets in `.agents/atlas/compiler-2026-11-10/digests/area_09.md`. Citations are the digest’s lines at commit `457adf4a11f`. `git diff --stat 457adf4a11f HEAD` for `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` is empty, so those lines still match the checkout. Nothing was built or run.

Short names are the digest’s: ESC `execution_scalar_computations.rs`, ETU `execution_terminal_unit.rs`, CALLS `execution_terminal_unit_calls.rs`, CLEAN `execution_terminal_unit_cleanup.rs`, CTRL `execution_terminal_unit_control.rs`, SG `execution_terminal_unit_state_graph.rs`, SSS `execution_terminal_unit_structural_scalar_store.rs`, VS `values_scalar.rs`. Paths are under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` unless another crate is named.

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | ESC:8329-8330, 8348-8349 | REACHABLE | Carrier match accepts the same generic name or symbol and never compares type arguments. |
| 2 | ESC:7119-7129 | CLOSED | The only consumer peels every remaining reference shell before it looks up fields. |
| 3 | ESC:10538-10543, 10590, 10597 | CLOSED | A non-primitive parameter is already excluded, and a completion refusal is stored and refused later. |
| 4 | ESC:6697-6733 | UNDECIDED | A linear reference becomes an empty cleanup span; the linear-obligation checker was not read. |
| 5 | ETU:1212-1240 | REACHABLE | Seventeen plain reference copies stop at the sixteenth local, in the planner and in the checker. |
| 6 | ETU:2359-2364 | CLOSED | Those three routes are not calls to a checked body; a body call is the other arm. |
| 7 | ETU:3374-3380, ETU:500-501, SSS:1006-1009 | UNDECIDED | A failed alias lookup keeps the authored place; whether an erased mutable alias reaches that and is still accepted was not settled. |
| 8 | ETU:6403 | UNDECIDED | Six value kinds record no transfer; only the quotient-adaptation case is closed by an earlier multiplicity test. |
| 9 | ETU:5116-5164 | UNDECIDED | Several operation kinds add no transfer here; the caller covers some of them and not others, and those others were not followed. |

## 1. ESC:8329-8330, 8348-8349 — REACHABLE

Digest: types match on an equal base name when the symbols differ. That sentence matches the code. It leaves out a second accept on the same lines: equal symbols match even when the generic arguments differ.

Read in full: `structural_carriers_match` (ESC:8297-8353). It unwraps both types, returns true when normalized identities are equal (ESC:8308-8311), and otherwise returns true for two generic nodes when the base symbols are valid and equal or the base names are equal and non-empty (ESC:8329-8330), and the same for a generic paired with a named type (ESC:8348-8349). Arguments are not read. Any other shape returns false.

Search: `structural_carriers_match(` under `omega-rust`. Four hits: the definition and three calls, all in ESC. Count of calls: 3.

- ESC:7514, inside `structural_case_value` (read from ESC:7503). A false result returns `None`. A true result keeps planning the case.
- ESC:9274, inside `structural_value` (read from ESC:8945 through ESC:9380). For a call, `returned` is that call’s own result type (ESC:9267-9273) and `expected` is the type the caller asked for. False returns `None`. True also requires `call_result_types.admits` on the returned type only (ESC:9275-9278, function read at ESC:1576-1604), which asks whether that type can be shaped, not whether it equals `expected`. The published node stores `expected` (ESC:9380, `publish_structural_value` read at ESC:9383-9400).
- ESC:9299, same function, on a case construction. False returns `None`.

`structural_value` is called with a use-site type, not always with the expression’s own type. Read two of those call sites: an expression statement passes the state’s result type (ESC:10636-10669) and a transition value does the same (ESC:11003-11016). Assignment uses the stored type (ESC:10632-10634).

No later check on this pair compares generic arguments. `declared_carriers_match` does compare them (validation `places.rs:4943-4975`, read), but that function is not called on this pair. The destination check in `checks_operators.rs` returns as soon as the destination is not a float (checks_operators.rs:251-255, read). The identity test at CTRL:6029-6031 compares the structural root’s stored type with the destination; the stored type is the destination, so the test passes. Stage 03 lands destinations onto constructors, strings, arrays, and matches, and its `needs_destination` does not include calls (pipeline `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs:1444-1452`, read).

Smallest program that should be rejected and is accepted: a state whose result is one instantiation of a generic data type, whose body is a call whose result is a different instantiation of that same generic. The base symbol matches, the arguments are ignored, and the plan is recorded as the destination type. The same branch also accepts two different data types that share only a simple name.

Not read: the other callers of `structural_value` beyond the two statement sites and `quotient_call_value` (ESC:4954-5059, read; that function compares identities itself and does not use this name match). Stage 06’s call emitter was not read.

## 2. ESC:7119-7129 — CLOSED

Digest: after eight reference shells the walk continues with the still-wrapped reference. The loop does stop and pass that reference on. The digest stops one call too early.

Read in full: `descriptor_fields` (ESC:7111-7131). The counter stops the loop once it has passed 8, so at most nine shells are removed, and the function then calls `data_application_value_fields` on whatever remains.

Search: `descriptor_fields(` under `omega-rust`. Three hits, all in ESC: the definition and two calls, `closed_descriptor_value` (ESC:7066) and `descriptor_parameter_symbol` (ESC:7096). Both functions were read (ESC:7054-7105). Each uses the returned list only in a `.find(...)?`. An empty list makes them return `None`, and `structural_field` (read ESC:6936-7044) then keeps the ordinary field read.

Read in full: `data_application_value_fields` (`typed-trees/src/declaration_tables_and_index.rs:3013-3058`). Its first act is `peeled_type_reference` (type_system.rs:3708-3720, read), which strips every reference shell and every constraint shell with no cap, and only then matches a named or generic node. Anything else returns an empty vector. Both callers of `descriptor_fields` go through that function. A remainder that is still a reference is peeled there; it is not treated as the data type.

`reference_shell` (validation `value_custody_core.rs:8688-8710`, read) peels carrier shells and then one reference node. That can stop on a named alias the later peel does not open. The field lookup then returns an empty vector, and the callers return `None`. That is not a false descriptor literal.

## 3. ESC:10538-10543, 10590, 10597 — CLOSED

Digest: a state with a non-primitive scalar parameter is skipped whole, and `record_completion`’s result is discarded, leaving the state with no computation plans. The first claim does not match the code. The second describes a discarded `Option`, not a discarded row.

Read: the state loop in `build_checked_value_computation_plans` (ESC:10528-10604) and `occupies_scalar_position` (VS:4324-4331). The filter keeps a parameter only when it is not erased and `primitive_type_reference` returns a type. The following `collect::<Option<Vec<_>>>()` asks that same question again. A non-primitive parameter never enters the map, so it cannot take the `continue` at ESC:10542-10543. Search `occupies_scalar_position(` : the ESC call is the import at ESC:22, which is this function.

Read in full: `record_completion` (ESC:1881-1906). A non-match returns `None` before any row is written. A match always pushes a `CheckedCompletionRoot`, including when the root carries a refusal, and the `Option` is only whether the refusal is absent. The `let _` at ESC:10590 and ESC:10597 drops that `Option` and leaves the row.

Search `CheckedCompletionRoot` under `omega-rust`: the row is built in ESC and consumed in CTRL and SG. Read `unit_completions::build` (SG:9327-9356): a refusal returns `None` (SG:9347-9350), and a missing row fails the lookup at SG:9346. Read the caller at SG:8296-8317: `None` does not return success; it falls through. Read `machine_lowering.rs:1521-1551` in stage 06: a machine with no scalar entry and no unit plan is `LoweringError::InvalidUnitMachinePlan`.

The described skip is not how a non-primitive parameter is handled, and discarding the completion `Option` does not delete a refusal that was stored.

Not read: the planners that run after the fallthrough at SG:8318. They are not needed for the claim that was classed open, which was the skip and the discarded result.

## 4. ESC:6697-6733 — UNDECIDED

Digest: an empty cleanup span on any failed precondition, so a fresh reference whose retirement lookup fails skips normal cleanup. The empty span is real. “Any failed precondition” is wider than the retirement function’s `None`.

Read in full: `observation_completion_actions` (ESC:6697-6733). It returns an empty span when the receiver is invalid, when the canonical place is not that expression with no segments, when the reference is not readable, and when `descriptor_retirement` returns `None`.

Search: `observation_completion_actions(` . Two hits: the definition and the call in `insert` (ESC:9760), which stores the span on the computation node (ESC:9761-9767).

Read in full: `descriptor_retirement` (CLEAN:1527-1553). No reference shell returns `None`. Unrestricted returns an empty vector. Affine returns one discard. Linear returns `None`. So the “lookup fails” case that becomes an empty span, after the readable-reference test has already passed, is linear multiplicity.

Read the installer at CLEAN:4422-4446: a node whose span is empty skips the observation-cleanup block and is still a planned computation. Stage 06 passes that span into `structural_field` (`06_checked-trees-to-lowered-psi/src/scalar_computations.rs:7435-7454`, read the call only).

Not determined: whether the linear-obligation checker treats this fresh reference as a place that must be consumed, and whether an empty `normal_cleanup_actions` span counts as that consumption. What would decide it is the collector that builds `LinearPlace` rows in `checks_multiplicity.rs` for an expression-rooted reference receiver. That collector was not read. Search `normal_cleanup_actions` under `omega-rust/psi/compilation` found only the node field, this planner, the installer above, and the stage 06 lowering call.

## 5. ETU:1212-1240 — REACHABLE

Digest: after 16 steps, or a failed lookup, alias resolution returns the partly rebased place, so 17 or more reference-copy aliases resolve to an intermediate local. The loop bound matches.

Read in full: `resolve_storage_place` (ETU:1202-1241) and `reference_route_source` (`flow_reference_places.rs:789-832`). Each pass follows one local whose initializer or whole-local assignment is the previous reference. After 16 passes the function returns the root and segments it has, with no error.

Search: `resolve_storage_place(` under `omega-rust`. Planner calls: ETU:998, ETU:1108, ETU:1592, CLEAN:2905. Count: 4. The checker has its own function of the same name, not a call of this one. Read the call at ETU:998-1006: the pair is stored as the canonical place and planning continues. Read the checker (checks_multiplicity_borrowed_windows.rs:1721-1761): the same 16-pass loop and the same return. It does not reject the seventeenth copy.

Smallest program: seventeen successive locals, each a plain copy of a reference that began as a parameter or other real storage, then a use of the last local that should be seen as a use of that original storage. Both the planner and the checker stop on the sixteenth local and treat it as the storage.

Not read in full: the calls at ETU:1108, ETU:1592, and CLEAN:2905. Each is a call that takes the returned pair; they were not opened past the call.

## 6. ETU:2359-2364 — CLOSED

Digest: dynamic, activation-storage, and descriptor-table call routes get no dependency edge and no check, so a body is kept even when the called implementation has no plan. The empty arm is real. The “called implementation” is not what those routes name.

Read in full: `admit_call_target` (ETU:2319-2366). A boundary target missing from the boundary list drops the caller (ETU:2330-2339). A module route to a unit, structural, or no-normal result requires that callee entry (ETU:2342-2347). A module route to a scalar result applies the scalar verdict (ETU:2348-2358). The three named routes do nothing (ETU:2359-2364).

Search: `admit_call_target(` in ETU. Two calls, ETU:2453 and ETU:2562, both on `OperationPlan::Call`. Those caller functions were not read in full. The arm is selected from the route already stored on the call, and the three routes are built only as follows, all read:

- Dynamic slot: CALLS:3727-3743, target taken from `ResolvedCallTarget::DynamicSlot`.
- Activation storage: CALLS:2270-2289, from `exact_toolchain_activation_storage_contract`.
- Descriptor table: CALLS:2404-2425, from `descriptor_table_operation`, which returns `None` unless the operation is one of the four descriptor operations.

A call whose callee is a checked body uses the module arm, which does call `require_entry`. The empty arm is not that case. The digest’s wrong program does not arrive here.

## 7. ETU:3374-3380 with ETU:500-501, and SSS:1006-1009 — UNDECIDED

Digest: a failed receiver-alias lookup becomes an empty alias list, an unresolved alias keeps the authored place, and an atomic place or a store through an erased mutable alias is planned against the alias local. The first two sentences match the code. The erased-alias sentence was not settled.

Read in full: `aliases` (ETU:3368-3403) and `formation` (ETU:3266-3360). `aliases` returns `None` only when the state has no flow row or no borrow row (ETU:3374-3377). An empty loan list returns `Some` of an empty vector (ETU:3378-3379). `formation` returns `None`, and the local is then left out of a `Some` list, for a mutable local, a type that is not an exclusive reference, a borrow whose access does not match, a path containing a runtime index, a storage-identity mismatch, a missing or duplicated loan, or a loan with no resource (ETU:3276-3342).

Search: `receiver_aliases::aliases(` plus the same-module call `aliases(program`. Three calls: ETU:500, SSS:1006, CALLS:6394. Count: 3.

Read the atomic caller (ETU:489-528): `unwrap_or_default` turns `None` into an empty list, and a local that is not in the list keeps its own symbol and segments (ETU:510). Read the store caller (SSS:1006-1014): the same `unwrap_or_default`, then `resolve(...).unwrap_or(place)`. Read `resolve` (ETU:3422-3445): if the owner is not in the list it keeps the place it was given and returns `None` when nothing changed, so the caller keeps the authored place.

Not determined: whether an erased mutable reference local is one of `formation`’s `None` results, and whether a store or atomic plan rooted at that local is rejected later because the local has no runtime home. What would decide it is `structural_access_for_type_reference` on an erased reference (called at ETU:3409, not read) and the store emitter’s handling of an erased local root. CALLS:6394 was not read. `first_state_borrow` was read (checked-trees `admissibility_and_borrow.rs:2069-2077`): it returns `None` when the state has no borrow row. Whether every state that has a reference alias also has that row was not traced.

## 8. ETU:6403 — UNDECIDED

Digest: value kinds not listed contribute no transfers, and a scalar, quotient-call, quotient-adaptation, zeroed-structural, or byte-literal view whose subtree moves an owned source records no transfer. The wildcard is real. The digest omits `NativeCodeLayout`, which falls through the same arm.

Read in full: `value_kind` (ETU:6130-6406) and the enum `CheckedStructuralValueKind` (checked-trees `values_and_views.rs:1099-1185`). The match returns `Some` with an empty transfer list for `NativeCodeLayout`, `QuotientCall`, `QuotientAdaptation`, `Scalar`, `ByteLiteralView`, and `ZeroedStructural` (ETU:6403-6405). It does walk calls, projections, records, cases, arrays, dispatches, references, places, and slice views.

Search: `value_kind(` . One definition. It is called only from `value` (ETU:5999), which was read (ETU:5982-6012).

The quotient-adaptation input is closed for this worry. Read `quotient_call_value` (ESC:4954-5018): the adaptation arm returns `None` unless the actual type is unrestricted (ESC:4993-4998). `leaf` records nothing when the source is unrestricted (ETU:5907-5909, read). An unrestricted source would have added no transfer even if this arm had walked it.

Not determined: a quotient-call argument that is an ordinary owned place. That path is the `else` at ESC:5012-5018, which checks identity and not multiplicity, and `QuotientCall` is one of the kinds `value_kind` does not walk. What would decide it is the caller that gathers transfers for a quotient call’s argument list. That caller was not found; only `value`’s own walk was read. The same gap remains for `Scalar` if a scalar computation moves an owned source that no other walk records.

## 9. ETU:5116-5164 — UNDECIDED

Digest: operation and binding kinds that are not listed contribute no transfers. No wrong program is named. The wildcard is real, and it is not the last word for every omitted kind.

Read in full: `operation_computations` (ETU:5103-5166) and the enum `CheckedUnitEffectOperationPlan` (checked-trees `flow_terminal_units.rs:1669-1744`). Before the match, a call’s scalar operands and structural arguments are walked (ETU:5108-5114). The match walks some local bindings, some store values, reference establishment, view subslice establishment, field moves, assignment-destination capture, atomic operands, and atomic fences. The trailing `_ => {}` (ETU:5163) is `ReleaseReference`, `EvaluateStructuralValue`, `ContinuationCleanup`, `Call` (its operands were already walked), `RegisterAccess`, `GlobalDescriptorTable`, and `Complete`. The inner `_ => {}` on bindings (ETU:5130) is the binding kinds other than scalar, primitive, scalar-array, and byte-sequence.

Search: `operation_computations(` . One call, ETU:5805. Read the caller `operation` (ETU:5799-5858). After the call it adds a move leaf for `MoveStructuralField` (ETU:5809-5820), a leaf for an installed value of a structural local or a store (ETU:5822-5836), and a full `value` walk for a `StructuralValue` local (ETU:5837-5848). Those three are therefore not accepted merely because `operation_computations` skipped them.

Not determined: `EvaluateStructuralValue`, `ReleaseReference`, `RegisterAccess`, `GlobalDescriptorTable`, `ContinuationCleanup`, and `Complete`. `operation` does not have an arm for them beyond the shared call-operand walk and a structural-result flag (ETU:5850-5856). What would decide it is whether any of those six can move an owned source, and whether another function records that move. Their emitters were not read.

## Counts

- REACHABLE: 2 (entries 1, 5)
- CLOSED: 3 (entries 2, 3, 6)
- UNDECIDED: 4 (entries 4, 7, 8, 9)
- Entries not reached: 82 of the 91 open bullets, plus the panic note at digest line 244.

## Entries not reached

Not opened. Listed in digest order, starting at the first bullet after entry 9:

- CALLS:5678-5691
- CALLS:6172-6174
- CLEAN:929-991
- CTRL:398-402
- CTRL:2546-2563; CTRL:1670
- CTRL:1881-1898
- SEQ:1350; SEQ:5502-5506; SEQ:7230-7232; SEQ:7208-7221 and 6770-6783; SEQ:9224-9228
- SG:2000-2002, 2566-2568, 3021-3023
- SG:3568-3570; SG:3571-3581
- SG:3895-3897, 3905-3909
- SG:8482-8487, 8704-8713, 9255-9282
- SG:7362-7372
- SSS:3061-3088; SSS:3051-3056
- TYPES:4786-4872, 1093, 4653-4659, 4760-4762
- F:742-748
- F:1273-1353
- F:1469, 1506; F:1524-1539
- F:1647-1678, 1618-1621, 1780-1784
- F:2741-2775
- F:3687-3689, 3729-3734
- F:4146-4165; F:4166-4174
- F:4432-4449
- F:4652-4656
- F:6609-6641
- F:6913-6920
- F:8110-8138; F:8896-8904
- F:9282
- F:9426-9482, 9732-9753
- F:10617-10619; F:10869-10874, 10964-10968; F:10759-10768; F:10799-10804
- F:11515
- FCP:836-857
- FCP:775-777; FCP:1394-1399; FCP:2068-2069, 2088
- FC:420-424
- FX:1433-1436
- FE:601-604, 720
- FM:1491-1500, 1512
- FM:4612
- FM:4593; FM:3879, 3897, 4047-4051; FM:1283-1289
- FO:1257-1269
- FOC:111-114
- FOW:67-69; FOW:869-874, 909-911
- FP:1012; FP:1241
- FRP:526-531, 858-860
- FRB:1271-1281, 1368-1391
- FSV:518-541
- FT:3599-3602
- FT:1085-1115
- PROD:809, 819
- PUB:8387-8586
- PUB:6796-6806
- PUB:10387-10391
- PUB:9732, 9780, 9822
- PUB:4465-4521
- PUB:2483-2485
- PUB:790-792
- PCC:5205-5207
- PCC:815-870
- PCC:750, 788-789
- PF:6637-6643, 5356-5377
- PF:412-455
- PF:7755, 7789-7791, 7835-7840
- PF:8155-8187
- PF:8290-8292, 8394-8399
- PF:9551-9569
- PEV:1014
- PEV:2211-2221
- PEV:1296-1299; PEV:1246-1251
- SH:1028-1030
- SH:2253-2292, 2648, 1967; SH:2181-2198
- SH:489-491; SH:2912-2923; SH:4685-4687
- VA:1016-1018
- VA:2164-2170
- VS:231-254
- VS:1328, 1394
- VS:2226-2234; VS:2032-2041, 2073-2093
- VS:4480-4489
- VS:4388, 4417; VS:5619-5622
- VS:10041-10060 with VS:5442-5443
- VS:8119-8135
- VS:6880-6882; VS:7552
- VS:10938-10941
- VS:5429 and the planners
- Panic note, not an accept: F:3757, 3592 (`Arena::get` not read)

## What I did not read

- Every open bullet from CALLS:5678 onward, including the enclosing functions and their callers.
- For the nine entries above, the callers named in each section as not read.
- The linear-obligation collector, `structural_access_for_type_reference`, the stage 06 structural-call emitter, and the transfer walk that would cover a quotient call’s arguments.
- Tests, samples, and the wiki.

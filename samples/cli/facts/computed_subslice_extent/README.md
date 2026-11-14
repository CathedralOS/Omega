# A computed array window keeps its length and original bytes

Given `can_window(items, low) == true`, the captured view `items[low + 0 .. low + 2]` has extent 2, and reading offset 1 observes original backing `items[low + 1]`.

The predicate checks that the backing has four bytes and `low <= 2`. Each positive forms the same two-byte window, measures its length, and reads offset one before checking runtime results. The array `[11, 23, 37, 53]` must yield `37` at `low = 1` and `53` at `low = 2`, including the allowed endpoint. The nine twins weaken only the predicate bound to `low <= 3`; the unchanged window can then exceed the backing and must refuse.

## Repair and owning requirement

This milestone advances **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md). Four identical target-prefixed bodies become one ordinary `ComputedWindowRead` implementation with four typed Build selections. Its public requirement retains the whole `can_window(items, low) == true` contract and an empty service ceiling (`reaches`). Private implementation facts cannot strengthen the caller’s contract.

The frozen baseline also exposed a compiler defect: every valid place rejected the pure predicate merely because its input was a shared slice. The classifier now distinguishes observing that input’s checked element sequence from treating a reference descriptor as a proof value. Only the outer shared input is interpreted this way. Nested references, linear elements, returned references, exclusive inputs, routed qualifications and other resource authority retain their existing fences. A second public-signature check now uses the same shared-versus-exclusive distinction, while preserving empty reach, invocation and crash ceilings and unconditional termination. Range formation and exact storage-version checks remain separate.

The compiler also has to distinguish a new slice view from moving an element out of borrowed storage. A computed range now follows the view route while still checking its collection and both endpoint expressions. This allows the generic library helper to return a view without adding a copy requirement to its element type. Actual indexed moves retain their ownership checks.

Calls keep the current shared contents separate from the slice descriptor, which stores the address and length. The public predicate can inspect those contents only when both identities are available; a descriptor with unavailable contents remains insufficient. When a helper returns a computed view, the compiler retains the actual input borrow even if it cannot represent the exact window coordinates. It compares that window with the recorded input using the same range-composition rule, while preserving the original input region, borrowing parents and suspension depth. That comparison grants no new access or pointer equality.

The match place still executes both choices at both low values. One arm writes an independent path marker before returning the unchanged computed window; the other forms it inline. Literal expected markers distinguish the paths. The helper’s returned lifetime names the array backing, and its checked postcondition retains length two.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [source](main.omg) | — | root entry subslice extent and offset read |
| local | [source](local/main.omg) | [source](invalid_local/main.omg) | the computed subslice view is bound in a local |
| field | [source](field/main.omg) | [source](invalid_field/main.omg) | the computed subslice view is stored in a record field |
| parameter | [source](parameter/main.omg) | [source](invalid_parameter/main.omg) | the computed subslice view arrives as an explicit parameter |
| call result | [source](call_result/main.omg) | [source](invalid_call_result/main.omg) | a helper call returns the computed subslice view with `result.len == 2` |
| match join | [source](match_join/main.omg) | [source](invalid_match_join/main.omg) | both match arms produce the computed subslice view |
| const name | [source](const_name/main.omg) | [source](invalid_const_name/main.omg) | named constants `ZERO` and `SPAN` supply the subslice endpoint offsets |
| other state | [source](other_state/main.omg) | [source](invalid_other_state/main.omg) | a successor machine receives the computed subslice view |
| other machine | [source](other_machine/main.omg) | [source](invalid_other_machine/main.omg) | another machine receives and inspects the computed subslice view |
| target body | [source](target_body/main.omg) | [source](invalid_target_body/main.omg) | one ordinary implementation selected by four Build rows constructs and reads the view |

The bundled slice API also still depended on retired top-level boundary operators and an empty data namespace. Its public `slice_index` and `slice_range` helpers now have ordinary checked bodies delegating to the existing private slice-view module. Array syntax keeps the catalog’s builtin geometry; imports select no provider or alternative operator. Named callers migrate to the free helpers. Index copyability, range bounds, returned length and original backing lifetime remain explicit. The unused `SliceAccess` boundary requirements remain separate library work.

## Controls and pins

Every place keeps its `invalid/` twin and its separate `controls/insufficient_bound` project; each differs only in the predicate bound. The nine Main/Build pairs match their pass/fail pins. The original saved-predicate, stale-field and effectful-actual corpus controls remain in scope.

Additional [shared-input control](controls/shared_predicate_inputs/main.omg) retains the same computed range and read using a fixed-array predicate, and checks that a closed public requirement may take a shared slice in a specification. Source controls keep [nested reference elements](controls/nested_reference_elements/main.omg), [linear elements](controls/linear_elements/main.omg), [reference results](controls/reference_result/main.omg) and [exclusive predicate inputs](controls/exclusive_predicate_input/main.omg) outside denotational admission. A [closed public exclusive-input control](controls/exclusive_public_predicate_input/main.omg) independently retains the public access fence. Neighboring proof controls retain termination, normal-return, proposition-reference and quotient-qualification fences.

Every runnable positive binds all four hosted targets and declares `Expected exit: 0`. Ordinary core slice operations provide the window; no `std` dependency or data-owned helper is introduced.

```sh
omega run --both samples/cli/facts/computed_subslice_extent/target_body/main.omg
```

## Validation

Original source `f5202b89368c37bc851329684d9fdc2ab16fa0d1`, frozen against published base `2977bfb92a918ff4c3f83b36cb092c52484f0f84`, failed [baseline run 41631799762](https://github.com/CathedralOS/Omega/actions/runs/41631799762). All nine positives stopped at the shared-slice predicate content gate. Neither engine executed, and public projects were not reached. Most twins stopped at that earlier gate too, so those refusals do not establish their intended bound obligation. The baseline audit verifies all 153 frozen input hashes, eight downloaded file hashes, the exact producer tree and both optimized profiles.

Source `500b325c2d4541b3ea942601249069d0b31001f6`, [run 41635444183](https://github.com/CathedralOS/Omega/actions/runs/41635444183), cleared the original input-content gate but exposed the public-signature gate and missing imported slice providers. All four new content controls and neighboring proof controls rejected for their intended reasons. Neither engine ran, and public projects were not reached. Its eight raw download hashes and optimized producer profiles remain audited historical evidence.

Before main integration, source `56494d6586497bea8fe6ea3facc30fdc209d6f18` recorded eleven matching native and Terminal results in [run 41649933905](https://github.com/CathedralOS/Omega/actions/runs/41649933905). An additional [ownership-restoration fixture](../../../../tests/omega/pass/ownership/runtime_generic_borrowed_case_restore_exit/main.omg) still failed physical legalization. [Exact-source replay 41651168899](https://github.com/CathedralOS/Omega/actions/runs/41651168899) reproduced that failure at the earlier `45c23b543577c22b7a592d109a91fe371ad27832` source, before the range repair. It remains an unaddressed neighboring limitation, with no runtime pass claimed. Its failure prevented that run from reaching the public projects.

Integrating published reference-borrowing changes exposed a missing input-borrow association for returned computed views. The unchanged library composition then exposed a mismatch between its nested window coordinates and the saved input region. Both failures are retained at their actual source revisions; they are not counted as intended rejections.

Measured source `93ac4da811fe95eb23e55262d7bdb159ee72c99c` passed [run 41671815659](https://github.com/CathedralOS/Omega/actions/runs/41671815659): all 46 checking cases, all 12 actual native/Terminal exit-zero agreements, and all 34 public projects. The public results include eleven engine agreements and 23 intended rejections. The corpus results include all nine fact places, their nine bound-sensitive twins, the unchanged generic library composition and the neighboring authority controls. All 34 rejected corpus cases matched their authored diagnostics. [Run 41671880303](https://github.com/CathedralOS/Omega/actions/runs/41671880303) independently passed three additional checking-only library callers; no runtime result is claimed for them.

The audits bind the actual optimized producers to their exact source tree and binary digests, verify the downloaded evidence and all 110 authored input hashes, and preserve the unchanged rejection controls. The source archive is published with [PR #1681](https://github.com/CathedralOS/Omega/pull/1681#issuecomment-6609655201) and verifies to SHA-256 `c2226931d5868df090b2a18f80708e053e1c3dbee4acea244c0be2d1cb99b250`. Only owned Source and Linux BOTH observations are refreshed; every other recorded row stays unchanged. These results establish this recorded source, with final documentation and record changes reviewed separately.

Historical macOS results at `bcd7c58d3a26` and `a93651a39273` retain their original source stamps. Earlier corruption experiments were not replayed by this milestone and are not current evidence.

## Requirements established by this row

1. **Establishment.** Checked predicate meaning supplies length four and `low <= 2`. Exact endpoint arithmetic, ordered formation and inclusion in the same backing are proved before the view exists.
2. **Identity and lifetime.** The view retains its original backing and length two through all nine places. The offset read consumes that view’s own length; unrelated descriptors or stale snapshots cannot supply it.
3. **Composition.** Calls, returned views, fields, joins and state arrivals retain their original requirements. The selected public contract remains the whole predicate, rather than a weaker inferred interval.
4. **Soundness boundaries.** Shared input observation grants no write, move, loan or resource duplication. Predicate admission still needs pure, terminating, crash-free checked behavior or an exact closed public promise.
5. **Checked-to-Terminal Psi.** Range formation, descriptor length and byte read lower into one checked portable artifact and run in Terminal Psi.
6. **Provider settlement, layout and ABI.** Four Build rows select one checked implementation. Borrowed descriptors retain their original base and count; the byte result uses its ordinary calling lane.
7. **Instructions and activation storage.** Native execution reads the two distinct expected bytes and preserves the independently observed match paths.
8. **Encoding and native emission.** Actual Linux x86_64 native and Terminal execution agree on authored exit zero; intended invalids reject for their own obligations.

Other-host runtime qualification and the parent’s remaining source/library/product migrations and target-family retirement stay open. This row does not close the parent or every borrow-proof requirement.

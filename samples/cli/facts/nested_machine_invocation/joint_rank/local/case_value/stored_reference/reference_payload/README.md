# Reference-bearing case payload ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

The same seven/empty operation crosses two stored references:
`Remaining.child` selects `Payload::Pending`, whose `value` selects the original
`Leaf`. The private coordinate is `remaining.child.value.count`, bounded by 7.
Seven decrements take count 7 to 0 and the explicit counter 3 to 10; the empty
path preserves 10. Each caller observes its original leaf, stamp 413, payload
guard 827 and neighboring canary 641. No caller equality guard supplies the
callee's missing bound.

Run a positive with `omega run --both <main.omg>`; check a reject twin with
`omega --check <invalid/main.omg>`. The authored exit and storage checks are
the oracle.

## Nine places

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../../../field/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../field/case_value/stored_reference/reference_payload/invalid/main.omg) |
| parameter | [source](../../../../parameter/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../parameter/case_value/stored_reference/reference_payload/invalid/main.omg) |
| call_result | [source](../../../../call_result/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../call_result/case_value/stored_reference/reference_payload/invalid/main.omg) |
| match_join | [source](../../../../match_join/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../match_join/case_value/stored_reference/reference_payload/invalid/main.omg) |
| const_name | [source](../../../../const_name/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../const_name/case_value/stored_reference/reference_payload/invalid/main.omg) |
| other_state | [source](../../../../other_state/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../other_state/case_value/stored_reference/reference_payload/invalid/main.omg) |
| other_machine | [source](../../../../other_machine/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../other_machine/case_value/stored_reference/reference_payload/invalid/main.omg) |
| target_body | [source](../../../../target_body/case_value/stored_reference/reference_payload/main.omg) | [source](../../../../target_body/case_value/stored_reference/reference_payload/invalid/main.omg) |

`other_state` is the nested-machine place; `other_machine` is a separate
ordinary machine. Each twin removes only strict descent. Its required rejection
is `no exact private-coordinate assignment proves every edge and complete cycle`.
The 18 sources are pinned as
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_reference_payload_<place>`.

## Composition and invalidation

| Witness | Required observation |
| --- | --- |
| [Common reference field](compositions/common_reference/main.omg) | Same rank and counter; a common-field guard reference remains usable and changes 827 to 828. |
| [Overlapping common reference](controls/common_reference_overlap/main.omg) | Rejects borrowing `guard` while local borrow `payload` is active. |
| [Reseated payload reference](compositions/reseated_payload_reference/main.omg) | Saves the incoming leaf (count 5, stamp 919), counts on the replacement (7 to 0, stamp 413), then restores the original pointer before return. |
| [Replacement outside the bound](controls/reseated_payload_reference_bound/main.omg) | Replacement count 8 cannot satisfy `remaining.child.value.count <= 7`, even though the saved leaf had count 5. |
| [Overwritten result scalar](controls/stale_scalar/main.omg) | Writing 8 after the passing `input` call invalidates its public payload bound. |
| [Replaced result pointer](controls/stale_pointer/main.omg) | Installing another leaf with count 8 invalidates that same bound. |

The last two controls retain the original passing call-result producer unchanged;
only their intervening write differs. Both must reject the `invoke` requirement
`payload.value.count <= 7`. These six fixtures use the prefix
`nested_machine_reference_payload_` under `tests/omega/{pass,fail}/facts`.

## Requirements and limits

1. **Establishment:** entry contracts independently establish the selected case
   and scalar bound. Constructors retain both common and selected payload
   fields. Completed calls transport their checked public result guarantees;
   neither a future assumption nor a tag alone establishes a readable reference.
2. **Identity and lifetime:** every stored-reference crossing consumes its own
   current slot evidence. Frozen scalar reads retain backing and descriptor
   dependencies. The disjoint counter write preserves the bound; overlapping
   scalar writes and pointer reseats invalidate it. Saved pointers keep their
   own identity instead of becoming fresh loads from a changed slot.
3. **Composition:** all nine places substitute explicit actuals and preserve
   the same private rank, arrival requirements and complete-cycle descent.
   Public result bounds reach both stored and directly forwarded call results.
   Case and field identities retain their selected declaration scope.
4. **Soundness boundaries:** all nine strict-descent twins reject. Bound-8,
   stale-value, stale-pointer and overlapping-loan controls preserve independent
   arithmetic, access and ownership obligations. Unknown reference suppliers
   do not become empty effects or evidence.
5. **Psi 05–07:** the nine positives and two positive compositions execute in
   Terminal Psi. Reference-bearing payload observations preserve affine custody;
   the emitter copies only copyable carriers. Full declared field paths retain
   their case and reference crossings before live projection and restoration.
6. **Omega 00–02:** all positives bind `ProgramEntry` for the four hosted
   targets. Shared typed geometry supplies layouts, pointer crossings and
   calling-plan inputs. Provider/ABI execution is observed on macOS ARM64 only.
7. **Omega 03–06:** native execution realizes exact subtraction, the private
   cycle and live caller counter, and checks the same original storage and
   canaries after return. This evidence does not certify portable WCSU or a
   separate physical stack bound.
8. **Omega 07–09 and emission:** emitted macOS ARM64 images exit 0 and agree
   with Terminal Psi on the authored oracle. Windows x86-64, Linux x86-64 and
   Linux ARM64 have source/Build coverage here; native execution on those hosts
   remains unverified.

## Recorded evidence and remaining acceptance

The frozen baseline is `c81dd8920ead1e41eba1f5586f51ba08f87d0fd6`.
The unchanged stored-reference witness wrongly refused exact subtraction and
private-cycle admission. Candidate 13 is optimized source
`f5dff06e558b147b7542cc6e5b785ca1f3d39f36`, with CLI SHA-256
`df09f8f75e15cde00351e4ddc75f7dc0138c81aec19116a6fa113e0e00661b26`
and corpus-runner SHA-256
`d89f243e6e91e2046fb4ba145cd9b62763a57c4f27ac9a5266b87c661b3b6207`.
Its actual macOS ARM64 observations establish nine native/Terminal exit-0 pairs,
nine strict-descent rejections, the common-reference composition and its overlap
rejection, and the reseated-reference composition. The bound and stale-result
controls also have retained source refusals; qualification replays the pinned
sources. Raw records and producer receipts are retained under
`build/deep-reference-rank/`, including `candidate13-row.json`,
`candidate13-controls.json`, `candidate13-reseat.json`,
`result-read-review/source-observations.json` and `pinned-cases.json`.
The manifest binds each original source, pinned sample and fixture to exact
`main.omg` and `build.omg` digests. Historical observations retain their producer
identities.

A separate inherited producer-establishment gap remains open: the isolated
source SHA-256 `466f63bf776903851f8e495920faa0ebddbe20c0ae48031dde229c3b2b18bffa`
refuses its own `input` ensures after a write, identically on baseline c81 and
candidate 12. Attribution is retained in
`result-read-review/isolated_producer_only_explicit_discard/attribution.json`.
That refusal supplies no runtime or caller-transport acceptance. This row closes
only the demonstrated reference-payload path; the parent task, further
invocation/custody composition, product compiler and other-host physical
acceptance remain open.

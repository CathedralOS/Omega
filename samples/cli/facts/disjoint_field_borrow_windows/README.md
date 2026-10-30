# Disjoint field borrow windows

Mutating or borrowing `pair.left` preserves the bound `pair.right <= 75u64`
on its disjoint sibling, permitting exact subtraction `75u64 - pair.right`.
Each place copies that fact and operation. The call-result and match-join
places establish the bound through a producer's published return contract;
the nested-machine place transports an explicit exclusive argument.

Parent: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
Its remaining migration must preserve this existing fact while replacing its
last target-prefixed machine family with ordinary implementations and explicit
Build provider rows. The wider migration remains open.

The nine positive/reject pairs are:

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Constant | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Selected body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin changes the producing value/domain to admit `right == 76u64`, while
keeping the exact subtraction unchanged. The intended rejection is an exact
`u64` arithmetic obligation. The pairs are pinned under
`tests/omega/{pass,fail}/facts/disjoint_field_borrow_windows_through_<place>_*`.

On fixed main `6913f2247361423f07452c1c29620e8f51c081dc`, the original call-result
and match-join programs wrongly refuse that subtraction after `touch_left`.
The nested-machine program separately refuses the enclosing result contract.
The storage owner retired each enclosing observation without retaining its
unaffected fields; nested invocation entry also dropped an independently
observed incoming referent. The repair preserves exact typed child snapshots
and the producer's observation admission through invocation entry. A whole or
opaque write still retires overlapping facts. A retained reference descriptor
does not expose its old pointee.

Requirements and evidence:

- **Establishment:** constructors and checked producer guarantees establish the
  bound. The consuming subtraction receives it directly, without a caller guard
  that asserts the desired bound afterward.
- **Identity and lifetime:** child snapshots identify the same completed value
  and stable backing across the disjoint write. They create neither entry
  premises nor new read events. Changed fields and opaque effects retain their
  invalidation fence; already captured values remain detached snapshots.
- **Composition:** calls, both match arms, explicit nested arguments and return
  contracts preserve the fact. Additional public generic-record fixtures move
  the whole value after a nested sibling write and retain a newer field store;
  their runtime checks observe the changed field and untouched tag as well.
- **Soundness boundaries:** existing access, formation, ownership and loan
  admission remain required. The nine weakened-bound twins and additional
  written-field and stale-pointee controls reject their arithmetic obligation.
- **Checked to Terminal Psi:** all nine positives and the root execute with
  native exit 0 and Terminal exit 0 on macOS arm64. The row transports the fact
  through the ordinary checked/lowered representation and Terminal interpreter;
  it does not introduce a cleanup or restoration permission.
- **Provider settlement and layout:** the selected-body place uses ordinary
  `SelectedProbe` implementations and target-keyed Build rows. Architecture
  checks distinguish the selected implementations at runtime. Its retained
  Terminal container is identical for both macOS ISA realizations and executes
  unchanged with the five declared hosted Terminal installations.
- **Instructions and activation:** the nine corpus positives and two generic
  composition programs execute in both engines on macOS arm64; the selected
  body additionally executes natively under macOS x86-64. These observations
  establish the exercised call/storage/arithmetic path, without a new ABI lane
  or a claim about every frame mechanism.
- **Encoding and emission:** retained Mach-O images execute with the authored
  results. Two wrong-provider counterfactuals produce typed Terminal traps;
  they are not native-trap or foreign-host execution evidence.

The immutable optimized repair producer is
`99fdbe448327cdfc08ee549f86dd78ab01f02760`; later source/fixture and documentation
commits leave its Rust and bundled-library inputs unchanged. Commands, source
hashes, producer profiles, actual engine observations, products and independent
review are retained in the qualification record accompanying the PR. Run a
place with `omega run --both <place>/main.omg` and its twin with
`omega --check <place>/invalid/main.omg`.

Windows and Linux native execution are unverified here. Array-index
non-interference, opaque referents, arbitrary reference-bearing alias joins and
whole-parent migration acceptance are not closed by this row. The historical
README's claims of native success on all four hosts were not established by
this qualification.

# Indexed storage address identity

For runtime u64 index < 2 into live [u64; 2], compatible references to
items[index] expose the same nonzero addr and differ from the other live
element address.

Every place copies that sentence and performs the same
`address_of(&items[index])` query with the same strict `index < 2` bound; only
how the indexed storage reaches the query changes. Each placement runs
indices 0 and 1, and `match_join` runs both arms. Every positive compares
repeated exposure (`selected == again`), nonzero identity (`selected != 0`),
and the selected against the other live element (`selected == first`,
`selected != second`, and conversely). The oracle is exit 0.

## Places

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` (`local/controls/held_element/main.omg`) |
| field | `field/main.omg` | `field/invalid/main.omg` (`field/controls/held_element/main.omg`) |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` (`parameter/controls/held_element/main.omg`) |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` (`call_result/controls/held_element/main.omg`) |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` (`match_join/controls/held_element/main.omg`) |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` (`const_name/controls/held_element/main.omg`) |
| a nested machine | `other_state/main.omg` | `other_state/invalid/main.omg` (`other_state/controls/held_element/main.omg`) |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` (`other_machine/controls/held_element/main.omg`) |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` (`target_body/controls/held_element/main.omg`) |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` and declares
`Expected exit: 0`. Run one place directly:

```sh
omega --check samples/cli/facts/indexed_storage_address_identity/<place>/main.omg
omega run --both samples/cli/facts/indexed_storage_address_identity/<place>/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit;
agreement at zero is the place's runtime oracle. The target-body placement
retains one machine body per hosted target, so checking also validates the
foreign bodies on every host.

## Reject twins

Each `invalid/main.omg` twin (and `controls/held_element/main.omg`) keeps the same
program shape, bounds and initialization, and changes only the custody around
the query: an exclusive `&mut` reference to the selected element stays live
across it. Checking must reject the shared reference formed inside `address_of`
because `held` remains active. Every twin rejects with exit 1:

```
error: state `unknown` receives `items[index]` while local borrow `held` is
still active (borrowed at statement N; its last use is at statement M)
```

(the `target_body` twin reports the same refusal once per target body).
Check a twin with `omega --check <place>/invalid/main.omg` (or
`omega --check <place>/controls/held_element/main.omg`).

Two further controls hold the row honest outside the per-place twins.
`controls/missing_bound` rejects the query's runtime selector when only
`index <= 2` is known (`cannot prove index `index` is within length 2`), and
`controls/raw_address_to_borrow` rejects using numeric address data as a
reference with a different referent carrier (`argument `value` for state
`entry` expects `&u64``), since the spec grants no inverse
address-to-borrow conversion.

The row is pinned by
`tests/omega/{pass,fail}/facts/indexed_storage_address_identity_*` fixtures —
nine `*_exit` positives, nine `*_rejected` twins, and the
`indexed_storage_address_identity_missing_bound` control. Every positive
records native exit 0 with Terminal Psi agreement on the hosts that ran it;
`corpus_{both,native}_macos_arm64.txt` carry the nine macOS AArch64 rows.

Canonical authority:
[address exposure](../../../../wiki/spec/language/counts_and_addresses.md#address-exposure)
and
[interpreted address identities](../../../../wiki/spec/language/counts_and_addresses.md#interpreted-address-identities).

## Requirements and Evidence

- **Establishment:** The strict `requires index < 2` contract (or `index < ROOM_COUNT` with `const ROOM_COUNT: u64 = 2`) on runtime `u64` selector and live backing `[u64; 2]` array establish the valid element projection and reference before `address_of(&items[index])` executes. Proof obligations are satisfied at every call site without relying on any post-hoc caller equality guard (`transition x == ...`) or partial operand manufacturing.
- **Identity and lifetime:** Repeated exposure of `&items[index]` yields the exact same nonzero `addr` value (`selected == again`, `selected != 0`), distinct from the other live element address (`selected == first` and `selected != second` for index 0; `selected == second` and `selected != first` for index 1), while respecting storage version, distinct array element slots, and borrow lifetime.
- **Composition:** The indexed array storage and bound travel end to end across all nine fact places (`local`, record `field`, callee `parameter`, contracted `call_result`, `match_join` branches, named `const_name`, `other_state` transition, `other_machine` forwarding, and `target_body` foreign targets). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` (and `<place>/controls/held_element/main.omg`) differ strictly in the held `&mut` borrow conflict (`held` active across `address_of`), keeping the construct, bound, and operation identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and `<place>/controls/held_element/main.omg`) enforce loan exclusivity at compile time, rejecting with `state `unknown` receives `items[index]` while local borrow `held` is still active` (exit 1). Outside the per-place twins, `controls/missing_bound` rejects forming indexed references without strict bounds proof (`cannot prove index `index` is within length 2`), and `controls/raw_address_to_borrow` enforces the boundary prohibiting inverse conversion from raw numeric address data back to typed borrows (`argument `value` for state `entry` expects `&u64``).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source `address_of(&items[index])` lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) operations without loss, erasing proof-only obligation tags while preserving interpreted address identity semantics, materializing borrow restoration windows, and executing with exit 0 in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Fixed-array `[u64; 2]` layout geometry (`size_of = 16`, element stride 8, align 8) resolves element offsets, and target `CallPlan`/`StatePlan` pass array, reference, and index carriers cleanly across registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit indexed effective-address computation (`lea` with scaled index on `x86_64`, shifted add on `arm64`) and unsigned address comparisons, assigns disjoint stack-frame activation storage homes for the live `[u64; 2]` array so distinct elements occupy separate physical addresses, and inserts standard frame prologues and multi-return epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for `items[0]` and `items[1]`, basic blocks lay out with branch relaxation and offset reflow, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O executable images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## What this row establishes and what stays open

This row discharges the `address_of` storage-operand requirement that
"compatible views of the same storage location expose the same address while
that location remains in place" for a runtime-indexed referent: the selected
element of a live `[u64; 2]` is addressed through `&items[index]`, repeated
exposure of the same live location agrees, the address is nonzero (zero stays
the absence encoding), and distinct live elements do not collide — matching
the ADDRESS-EXPOSURE board acceptance clause "repeated exposure agrees and
distinct live locations differ" for readable primitive storage referents.
Establishment is covered by the strict `index < 2` contract carried through
every place; composition by the parameter, call result and other-machine
places, which substitute the exact actual storage into the callee; and
soundness of the borrow boundary by the `held_element` twins, which show the
query cannot establish a loan to storage an exclusive borrow still holds —
"the query cannot establish a loan to otherwise inaccessible storage."

Still open under ADDRESS-EXPOSURE: `address_of<Selected<Arguments>>()`
selected machine applications, aggregate (record/array-typed) referents,
write-only `&write` storage operands, proof-only entity refusal, and
exhausted-encoding refusal. `controls/raw_address_to_borrow` pins the "no
inverse address-to-borrow conversion" boundary only for this carrier shape.
Native execution rows for `windows_x86_64` and `linux_arm64` record on their
own hosts' goldens; the bindings are declared for all four.

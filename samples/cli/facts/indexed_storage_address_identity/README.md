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
| local | `local/main.omg` | `local/controls/held_element/main.omg` |
| field | `field/main.omg` | `field/controls/held_element/main.omg` |
| parameter | `parameter/main.omg` | `parameter/controls/held_element/main.omg` |
| call result | `call_result/main.omg` | `call_result/controls/held_element/main.omg` |
| match join | `match_join/main.omg` | `match_join/controls/held_element/main.omg` |
| const name | `const_name/main.omg` | `const_name/controls/held_element/main.omg` |
| another state | `other_state/main.omg` | `other_state/controls/held_element/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/controls/held_element/main.omg` |
| another target body | `target_body/main.omg` | `target_body/controls/held_element/main.omg` |

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

Each `held_element` twin keeps the same program shape, bounds and
initialization, and changes only the custody around the query: an exclusive
`&mut` reference to the selected element stays live across it. Checking must
reject the shared reference formed inside `address_of` because `held` remains
active. Every twin rejects with exit 1:

```
error: state `unknown` receives `items[index]` while local borrow `held` is
still active (borrowed at statement N; its last use is at statement M)
```

(the `target_body` twin reports the same refusal once per target body).
Check a twin with `omega --check <place>/controls/held_element/main.omg`.

Two further controls hold the row honest outside the per-place twins.
`controls/missing_bound` rejects the query's runtime selector when only
`index <= 2` is known (`cannot prove index `index` is within length 2`), and
`controls/raw_address_to_borrow` rejects using numeric address data as a
reference with a different referent carrier (`argument `value` for state
`entry` expects `&u64``), since the spec grants no inverse
address-to-borrow conversion.

The row is pinned by
`tests/omega/{pass,fail}/facts/indexed_storage_address_identity_*` fixtures —
nine `*_exit` positives, nine `*_held_element` twins, and the
`indexed_storage_address_identity_missing_bound` control. Every positive
records native exit 0 with Terminal Psi agreement on the hosts that ran it;
`corpus_{both,native}_macos_arm64.txt` carry the nine macOS AArch64 rows.

Canonical authority:
[address exposure](../../../../wiki/spec/language/counts_and_addresses.md#address-exposure)
and
[interpreted address identities](../../../../wiki/spec/language/counts_and_addresses.md#interpreted-address-identities).

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

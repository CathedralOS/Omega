# Selected native code layout

The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.

Every place copies that sentence and performs the same
`code_layout<selected<u64, 7>>()` query with the same specialization; only how
the returned `CodeLayout` reaches `inspect` changes. The query is checked
against the nonzero entry, a nonempty attributed region sequence, entry
attribution (`regions[0].base == layout.entry`) and a positive instruction
length. The root `main.omg` probe retains its original body and now declares
`Expected exit: 0`. Both `match_join` arms execute, and the const-name place
uses the constant only to select the same application — the metadata remains
runtime data, never a semantic constant.

## Places

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` and declares
`Expected exit: 0`. Run one place directly:

```sh
omega run --both samples/cli/facts/selected_native_code_layout/<place>/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit;
agreement at zero is the place's runtime oracle. The target-body placement
retains one machine body per hosted target, so checking also validates the
foreign bodies on every host. Pure Terminal interpretation declines this
native query rather than fabricating an address or byte region from an
interpreter ID: under `omega run --both` the interpreter leg reports
`DECLINED (UnsupportedCapability(NativeCodeLayout))` and the native leg
carries the runtime oracle.

## Reject twins

Each adjacent `invalid/` twin keeps the same program shape, specialization and
query, and changes only the receiving evaluation stage: the same witness is
consumed by `const OBSERVED: bool = witness()`. Late native metadata cannot
feed a same-product semantic constant, so checking must reject with exit 1:

```
late native code-layout metadata is unavailable to same-product semantic
constant, type, layout, or optimization evaluation
```

An unrelated parse, contract, or lowering error does not count. Check a twin
with `omega --check <place>/invalid/main.omg`.

The row is pinned by
`tests/omega/{pass,fail}/facts/selected_native_code_layout_*` fixtures — nine
`*_exit` positives and nine `*_rejected` twins. Every positive records native
exit 0 with Terminal Psi agreement on the hosts that ran it;
`corpus_{both,native}_macos_arm64.txt` carry the nine macOS AArch64 rows.

```sh
python3 tools/corpus_gate.py --filter facts/selected_native_code_layout_through_
python3 tools/corpus_gate.py --both --filter facts/selected_native_code_layout_through_
```

## Production path

The otherwise uncalled selected specialization must survive into native text.
Metadata comes from final selected instruction spans plus final frame spans,
including branch reflow. The image producer relocates entry and region bases;
no symbol-size subtraction, second compilation, address authority, or lifetime
extension is involved. Runtime copies use the ordinary record/slice ABI after
checking its actual field offsets and element stride.

The focused regressions also cover static type/const/machine/lifetime
forwarding, ordinary declaration shadowing, unavailable/proof-only/incomplete
selections, frame instructions, uncalled boundary-body realization and
portable family progress contracts.

Canonical authority, captured at `7875d3411f26556b6c6711494cd659032e274abe`:
`wiki/spec/language/counts_and_addresses.md`, blob
`348dfb1fa43ef749374a9ff46078b211443fcf6c`:

> `code_layout<SelectedMachine<Arguments>>()` describes the exact selected native
> realization. Like `address_of`, it is a compiler-known core query, not a keyword
> or an invocation of that machine. Its retained use creates the same executable
> demand.

> Regions cover the emitted instruction spans attributed to that realization,
> including its retained entry and any split instruction regions. They do not
> include unrelated intervening bytes, padding, data pools or transitively called
> machines merely because those lie nearby or are needed for execution.

> Lengths are finalized by native lowering, layout and any later size-changing
> realization; bases may remain link/load relocations. This is late artifact
> metadata, not hermetic evaluation of the program being produced. Neither its
> sizes nor its addresses may feed that same program's semantic constants, type
> arguments, layout policies or optimization decisions through a circular query.

## What this row establishes and what stays open

This row discharges the NATIVE-CODE-LAYOUT-QUERIES clause that "ordinary exact
single-fragment generic applications are covered" — a complete exact family
application `selected<u64, 7>` resolves at the existing physical selection
boundary and yields the selected member's final nonzero entry and exact
attributed regions, observable at runtime through every transport place.
Establishment is the retained `code_layout` query's executable demand
justifying realization of the otherwise uncalled specialization; composition
is covered by the parameter, call-result, other-state, other-machine and
match-join places substituting the exact `CodeLayout` value across call,
state and target boundaries; native realization and image emission is the
nonzero relocated entry plus attributed instruction regions surviving branch
reflow into the emitted image; and the soundness boundary is pinned by the
nine `invalid/` twins, which prove the same metadata rejects when consumed as
a same-product semantic constant.

Still open under NATIVE-CODE-LAYOUT-QUERIES: real split/shared native
production with one placed fragment and one frame record per application,
multiple-fragment refusal, missing or ambiguous native realization refusal,
and the direct `code_layout` query of a bodyless target-family head, which
remains unsupported and rejects explicitly per the canonical refusal rule —
"Unsupported or unavailable realization metadata rejects rather than
inventing a region." Native execution rows for `windows_x86_64` and
`linux_arm64` record on their own hosts' goldens.

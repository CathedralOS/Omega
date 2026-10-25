# Checked predicate membership on an array copy

Copying a four-u16 array carries its checked zero tail, so exact addition to
65535 stays 65535.

This is a prerequisite of `FILESYSTEM-PLATFORM-SERVICES`: the Windows raw
metadata provider terminates a wide-character buffer before passing its
qualified copy to a nested machine. It is not a Windows-specific language rule.
The exact requirement this row establishes: a completed initialization call
followed by a tail store establishes `values[3] == 0`, a copy of the carrier
retains that checked membership (`[u16; 4]::ZeroTail`), and the consuming exact
addition `65535u16 + captured[3]` is proved directly from the retained fact —
with no caller equality guard re-establishing it.

The same fact and consuming operation travel through `local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, and `target_body`. Each place has an `invalid/` twin whose
tail changes from zero to one. No caller guard re-establishes the fact before
the exact addition. Each positive binds all four hosted targets and expects
exit 0 from both native and Terminal execution.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the qualified copy is captured into the local `captured` before the exact addition |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the copy is stored in `Envelope.values`, a domain-qualified record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the copy arrives as the `consume(values: [u16; 4] in ZeroTail)` parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `produce()` returns `[u16; 4] in ZeroTail` and `consume` takes that call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the same qualified copy and the join retains `ZeroTail` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the tail store stores the named constant `TAIL`, and the copy keeps its membership |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine `accept(captured: [u16; 4] in ZeroTail)` receives the qualified copy |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `qualify(values) requires values[3] == 0` returns the qualified copy to `consume` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies captures the qualified copy and performs the exact addition |

## Execution

Every positive place binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares
`Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and
native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/predicate_array_copy/local/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/field/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/parameter/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/call_result/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/match_join/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/const_name/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/other_state/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/predicate_array_copy/target_body/main.omg
python3 tools/corpus_gate.py --filter facts/predicate_array_copy
```

Run a place with `omega run --both --keep <place>/main.omg`, then independently
consume its saved `program.oterm` using `omega inspect-terminal --artifact
<saved>/program.oterm`. The second command matters: execution alone did not
detect the canonical reader's stricter copy-membership rule.

### The oracle discriminates

Exit `0` is strictly discriminative. The one-tail twins cannot reach the exact
addition: the qualified capture no longer obtains, so `captured[3]` is not
provably zero, and `65535u16 + captured[3]` is refused as `may overflow u16`
(local) or refused earlier at the capture, the field construction, the
parameter, the return contract, the join, or the qualifying call. The positive
places demonstrate the same addition is *exactly* 65535 because the retained
membership proves the tail is zero — not because the operands happen to fit.

## Reject Twins

Each place has an `invalid/` reject twin whose tail changes only from
`values[3] = 0` to `values[3] = 1` (verified with `git diff`); the qualified
capture no longer obtains, and proof checking fails closed during compile-time
verification:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | ``exact arithmetic`` / ``local `answer` may overflow `u16` `` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | ``construction of `Envelope` field `values` is not proven in domain `[u16; N]::ZeroTail` `` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract for call consume from main: values in [u16; N]::ZeroTail` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | ``cannot prove default-domain field requirement for return from produce`` / ``values requires [u16; N]::ZeroTail`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | ``cannot prove initializer of `captured` in choose is in domain `[u16; N]::ZeroTail` `` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | ``cannot prove initializer of `captured` in main is in domain `[u16; N]::ZeroTail` `` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract for call accept from main: values in [u16; N]::ZeroTail` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract for call qualify from main: values[3] == 0` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract for call evaluate from main: values[3] == 0` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/predicate_array_copy_through_*`
fixtures: nine pass fixtures (checked, and built with `exit:0` plus Terminal
agreement on this host) and nine reject twins (rejected with their authored
fragments). The nine pass fixtures are byte-identical copies of their place
programs, and the nine fail fixtures are byte-identical copies of their twins,
so sample and fixture cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `FILESYSTEM-PLATFORM-SERVICES` (`TASKS.md`): complete the
  remaining portable platform-services behavior; this row is its array-copy
  membership prerequisite, not a Windows-specific special case.

| Requirement | Evidence |
| --- | --- |
| Establishment | A completed initialization call is followed by the exact tail store; the qualified capture must establish `ZeroTail`. |
| Identity and lifetime | The copy is a fresh unrestricted occurrence, not a qualification added retroactively to the initializer's contract. Separate fixtures check mutation before capture rejects and mutation after capture preserves the copy. |
| Composition | Nine transport places retain the same predicate and exact u16 addition. |
| Soundness boundaries | Each one-tail twin rejects; routed origin is not obtained from equal bytes. |
| Psi 05–07 | Checked memberships survive proof erasure, canonical serialization and separate consumption; Terminal execution returns the authored result. |
| Omega 00–02 | Hosted Build bindings select the entry; fixed-array geometry and qualified aggregate argument/result placement remain ordinary plans. |
| Omega 03–06 | Native array copies, calls and state transfers preserve payloads and independent storage; the exact addition is realized in u16. |
| Omega 07–09 and emission | The host image executes with exit 0 and agrees with Terminal execution; this does not establish native execution on an unavailable host. |

The regression fixtures mirror this row under
`tests/omega/{pass,fail}/facts/predicate_array_copy_through_*`.
Additional controls under `tests/omega/{pass,fail}/domains/copied_array_domain_*`
challenge source mutation and issuer provenance. These programs establish only
the copy/predicate mechanism, not the complete Windows filesystem provider or
its owner-deferred Windows native acceptance.
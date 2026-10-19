# Runtime window length

A runtime window length names the last byte of that window: for a window
`items[0..count]` with `count > 0`, indexing at `length - 1` reads the byte at
`count - 1`, provided `length` is proved to be that window's length.

Every place copies that fact and performs the same last-byte read on a
four-byte `[4, 7, 8, 9]` array with `count == 3`; the oracle is exit 0 only
when the byte is 8. Only the transport of the window length changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the length is copied into a local |
| field | `field/` | the length is stored in a record field |
| parameter | `parameter/` | the length arrives as an explicit parameter |
| call result | `call_result/` | a call result is the window length |
| match join | `match_join/` | both arms yield the window length |
| const name | `const_name/` | a named constant is the window length |
| another state | `other_state/` | a successor state receives the length |
| another machine | `other_machine/` | a forwarding machine supplies the length |
| another target body | `target_body/` | each of four target bodies uses the length |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/length/local/main.omg
omega run samples/cli/facts/length/field/main.omg
omega run samples/cli/facts/length/parameter/main.omg
omega run samples/cli/facts/length/call_result/main.omg
omega run samples/cli/facts/length/match_join/main.omg
omega run samples/cli/facts/length/const_name/main.omg
omega run samples/cli/facts/length/other_state/main.omg
omega run samples/cli/facts/length/other_machine/main.omg
omega run samples/cli/facts/length/target_body/main.omg
```

Each place has an `invalid/` twin whose transported length names the backing
view rather than the runtime window. The last-byte index is then no longer
proved: the twin must reject with `cannot prove index ...` (the const-name twin
reports `cannot prove index ... is within length 2`). Check a twin with
`omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/{pass,fail}/facts/length_through_*`
fixtures; every positive records native exit 0. The nine pass fixtures are
byte-identical copies of their place programs, so the sample and the fixture
cannot drift apart.

## Evidence

Owning task: `TASKS.md` compiler-advance capability work; the requirement this
row discharges is that a runtime window length is proved and carried from its
origin to the consuming last-byte read through every carrier, and that the read
executes natively with Terminal Psi agreement on all four hosted targets.

Measured on macos_arm64 at base `f98259e86dd2` with the opt-level 1 CLI:

- All nine places and every place `main.omg` bind `ProgramEntry` for
  `windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`, and
  declare `Expected exit: 0`. `omega run --both` reports `native exit 0` with
  `interp exit: 0` for each.
- `python3 tools/sample_native_gate.py --filter cli/facts/length` reports 9/9
  samples meeting their documented oracle and 9/9 intended-invalid controls
  matching their authored rejection.
- `python3 tools/corpus_gate.py --native --filter facts/length` reports nine
  `built ... exit:0` rows, recorded into
  `tests/omega/corpus_native_macos_arm64.txt`.
- The oracle discriminates: corrupting the array byte under the proved
  last-byte read (`[4, 7, 8, 9]` to `[4, 7, 1, 9]`) leaves the index proved but
  changes the read result, and all nine places then trap in both engines —
  `native signal 5` with `interp crash: Trap` agreeing. An oracle that cannot
  fail would not establish the read.

Requirement coverage:

- **Establishment:** the window length is established where it originates —
  `window.len` on the narrowed view, a `measure` call's `ensures result ==
  window.len`, or a `const` matched by the `requires` bound — never by a
  post-hoc caller equality guard.
- **Identity and lifetime:** each place names an exact length of an exact
  window; the `&items[0..count]` view is the same storage the read indexes, so
  no write invalidates it between proof and read.
- **Composition:** the consuming `window[length - 1]` is proved directly from
  the transported fact at each call, join and state entry; the positive and its
  `invalid/` twin differ only in the construct's own transport, verified by
  `diff` against each pair.
- **Soundness boundaries:** the twins show the proof is not transferable —
  substituting a version of the length that names the backing view loses the
  entailment, and the checker refuses rather than accepting a widened bound.
- **Terminal Psi and carrier lowering:** every place lowers to the same
  target-neutral last-byte read; the `match_join`, `other_state` and
  `other_machine` places exercise the fact across a join, a state boundary and
  a call boundary.
- **Native realization and execution agreement:** the nine recorded `exit:0`
  rows and the nine trap rows above are native observations, and Terminal Psi
  agrees on each.

Remaining open: the root `main.omg` is absent for this fact, so there is no
root place to bind; nothing else in this row is open.

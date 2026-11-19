# Induction call guarantee

An inductive call records its postcondition under the dispatch subject's
comparison. The caller reads that record.

Owning task: **OPERATOR-MACHINE-SUPPLY**. The requirement is citation and
induction call guarantees with their transitive assumptions. `gauss` terminates
by `n` and ensures `result * 2 == acc * 2 + n * (n + 1)`. The transition
subject `n > 0` is evaluated once; each arm assumes that comparison. The
recursive arm receives the same postcondition at `(n - 1, acc + n)` only after
that comparison shows the measure decreased and stayed non-negative. `use`
binds the call and records the postcondition at accumulator zero. `observe`
requires that record. `hold` requires the resulting `true`.

A checked lemma citation already imported its `ensures` after its `requires`.
This row is the induction call whose guard assumption is the captured subject.

Before the repair, the captured subject hid the arms, so `gauss` could not
prove its postcondition and the caller could not read it.

Each twin changes only the base arm from `acc` to `(acc + 1)`. The
postcondition is then unproved. The recursive arm is unchanged.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [field](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [parameter](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [call result](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [match join](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | [const name](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [other state](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [other machine](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| target body | [target body](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Reject fragments are `cannot prove ensures` and `gauss`.

On Linux x86_64, all nine positives report native exit 0 and interpreter exit 0
from `omega run --both`. All nine twins reject. Windows, macOS, and QEMU were
not run.

- **Establishment:** the base arm returns `acc`. The recursive arm uses the
  postcondition only after the captured `n > 0` comparison discharges descent.
- **Identity:** `n` and `acc` are the call's formals. The captured subject is
  that comparison, not a second value.
- **Composition:** `use` reads `gauss`. `observe` reads `use`. No post-hoc
  guard replaces those contracts.
- **Soundness:** a base arm of `acc + 1` leaves the postcondition unproved.
- **Psi 06–08 through Omega 07–09:** the nine positives agree on this host.
  The other hosted targets are bound and not executed here.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.

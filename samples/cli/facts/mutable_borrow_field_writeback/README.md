# Mutable borrow field writeback

Updating a record field through an exclusive mutable borrow (`&mut Counter`) and retiring the loan establishes the updated field bound (`counter.used <= 80u64`) while preserving unrelated fields, discharging exact subtraction `80u64 - counter.used`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Mechanism

A `&mut Counter` projection forms an exclusive loan over the *aggregate* `c`,
naming a reference to the original storage rather than a copy of the field
([structural access](../../../../wiki/spec/terminal-psi/structural_access.md)).
The store `r.used = 50u64` therefore writes through that reference into `c`'s
own storage. When the loan retires, the borrowed-storage restoration window
re-establishes the refined post-loan fact on the owner, so the aggregate later
reports `c.used == 50u64` while the never-touched `c.limit == 80u64` is
preserved. That restored field bound is what proves `80u64 - c.used` in range.

This is distinct from `shared_borrow_field_projection` (a `&` projection grants
no write authority), from `disjoint_field_borrow_windows` (two distinct fields
held live at once), and from whole-aggregate mutation (the write is projected
through the aggregate, not applied to a whole-referent replacement).

## Classification: coverage, not repair

Measured on this baseline, the root program and all nine places already run
green through both engines:

| Place | `omega run --both` |
| --- | --- |
| root | `native exit 0` / `interp exit: 0` |
| local | `native exit 0` / `interp exit: 0` |
| field | `native exit 0` / `interp exit: 0` |
| parameter | `native exit 0` / `interp exit: 0` |
| call result | `native exit 0` / `interp exit: 0` |
| match join | `native exit 0` / `interp exit: 0` |
| const name | `native exit 0` / `interp exit: 0` |
| a nested machine | `native exit 0` / `interp exit: 0` |
| another machine | `native exit 0` / `interp exit: 0` |
| another target body | `native exit 0` / `interp exit: 0` |

No place refused and none produced a wrong result, so this row is a **coverage
gap**: the places already ran and the missing evidence was the fixtures and
host-pinned goldens, not a compiler change. No compiler source was edited.

## Discrimination

Both halves of the construct were proved, and the classes are reported per probe.

**Runtime half — the written value is observable through the aggregate.**
Corrupting the consumer's observation (`transition c.used == 999u64` after the
loan retires, with the write still `r.used = 50u64`) reached runtime and trapped
in both engines: `native signal 5` / `interp crash: Trap (host presentation
agrees)`. The value written through the mutable view is exactly what the
consumer reads back from the aggregate.

**Static half — the construct's own contract refuses when violated.**

| Probe | Class | Diagnostic |
| --- | --- | --- |
| Write `81u64` into a field declared `where used <= 80u64` | static refusal, before native production (`--check` exit 1) | ``machine `main` state `entry` assignment stores a value not provably within its data's `where` facts`` |
| Construct `Counter { used: 95u64, … }` against `where used <= 80u64` | static refusal at the construction gate, independent of seeding (`ch12: construction is the gate`) | ``data `Counter` literal violates the default domain: a `where` fact evaluates FALSE at this construction`` |
| Two overlapping `&mut c` loans of the same aggregate | static refusal | ```statement 2 creates local borrow `r2` while local borrow `r1` is still active``` |
| Write `r.used = 50u64` through a *shared* `&Counter` | static refusal | `assignment requires mutable source access through every reference crossed by its destination` |

The obligation's kind here is a construction/`where`-clause fact plus an exact
subtraction range obligation, not a value boundary admitting a range, so no
runtime trap is reachable from the invalid controls: all four refuse statically,
and the nine twins refuse on the exact-arithmetic obligation. The overlapping
borrow refusal above is the compiler correctly rejecting a sibling's twin shape
(two loans of the same field), reported here as static rather than runtime.

## Fixtures

Nine pass fixtures and nine reject twins are pinned under
[tests/omega/pass/facts](../../../../tests/omega/pass/facts) and
[tests/omega/fail/facts](../../../../tests/omega/fail/facts) as
`mutable_borrow_field_writeback_through_<place>_exit` and
`mutable_borrow_field_writeback_through_<place>_unproved`. Each pass fixture
binds `ProgramEntry` for the four hosted targets and carries its own
`expected_native_exit.txt`; each twin breaks only the construct's own bound
(`used <= 81u64` and writing `81u64`) and authors its diagnostic in
`expected.txt`.

`python3 tools/corpus_gate.py --native --filter facts/mutable_borrow_field_writeback`
reports nine `built … exit:0` rows in
[tests/omega/corpus_native_macos_arm64.txt](../../../../tests/omega/corpus_native_macos_arm64.txt);
the checked leg reports nine `checked` and nine `rejected` rows, 0 rejected
without its expected fragment and 0 fail fixtures that did not reject.

## Places

The nine places carry the post-writeback record bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | mutable borrow writeback establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | post-writeback field value bound to a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | record containing counter mutated through nested borrow |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | counter passed as an explicit mutable reference parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | post-writeback record returned across call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | post-writeback record preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | post-writeback bound constrained against named constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | post-writeback record passed as an argument across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | post-writeback record forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | post-writeback record consumed across target-scoped machine bodies |

Run each place with `omega run --both <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `STRUCTURAL-ACCESS` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place exclusive mutable borrow writeback, loan retirement, and path-sensitive field bound restoration, verifying that mutating a record field through `&mut Counter` and ending the borrow restores the refined field bound (`counter.used <= 80u64`) while preserving unrelated fields (`counter.limit == 80u64`), statically discharging exact subtraction `80u64 - counter.used`.

Full 8-layer specification and lowering evidence:

- **Establishment:** An exclusive mutable borrow `let r: &mut Counter = &mut c;` is created over record `Counter`. Writing `r.used = 50u64;` through the borrow updates the referent field in-place. When the mutable loan `r` retires, the loan restoration window re-establishes the refined post-loan invariant on the owner `c`: `c.used <= 80u64` (governed by `Counter where used <= 80u64`) while unrelated field `c.limit == 80u64` remains preserved without interference. The restored field bound directly proves non-negativity for exact subtraction `80u64 - c.used` without requiring any post-hoc caller equality guard (`transition diff == 30u64 && c.limit == 80u64` verifies the authored output at runtime, rather than manufacturing the proof premise).
- **Identity and lifetime:** The referent identity is tied strictly to the root storage of `c`. The exclusive borrow `&mut Counter` temporarily suspends shared or independent access to `c` during the loan duration. Upon loan retirement, the referent identity and all unmodified fields (`limit`) retain their exact pre-loan values, and the modified field `used` reflects the updated writeback value across local variable binding, nested record field projection, parameter passing, machine call returns, match joins, constant bounds, state transitions, machine forwarding, and target-scoped bodies.
- **Composition:** The exclusive borrow writeback and post-loan restoration compose across all 9 standard places to statically discharge exact subtraction `80u64 - counter.used`:
  - `local`: loan is taken and retired, and updated field is bound to a local variable `used` before subtracting from `80u64`.
  - `field`: record `Counter` is stored inside outer record `Container.counter`, and mutated through `&mut cont.counter`.
  - `parameter`: mutable reference `&mut Counter` is passed as an explicit machine parameter `update_counter(&mut c)`.
  - `call_result`: helper machine `mutate_and_return()` performs borrow writeback and returns the updated `Counter`.
  - `match_join`: updated counters converge across divergent match branches.
  - `const_name`: subtraction bound is defined by compile-time named constant `const LIMIT: u64 = 80u64`.
  - `other_state`: updated `Counter` is passed as an argument to state `evaluate(c)` across a transition edge.
  - `other_machine`: updated `Counter` is forwarded through helper machine `forward(c)`.
  - `target_body`: mutable borrow writeback and subtraction are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the field bound to `used <= 81u64` and writing `81u64`, proving that exact subtraction `80u64 - counter.used` strictly fails closed when the post-writeback bound permits underflow.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow `u64``, confirming that the arithmetic proof engine soundly detects that when `used` can be 81, `80u64 - counter.used` may underflow `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Mutable projection loans and field writebacks lower into canonical reference-write and loan-retirement actions, preserving borrowed-storage restoration windows (`reborrow restored call uses`) without loss. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 16-byte contiguous record layout (two 8-byte `u64` fields at offsets 0 and 8, alignment 8), and `CallPlan`/`StatePlan` assign register and stack homes for passing pointers and records across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for indirect pointer stores (`mov [reg + offset], val`), 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements and field offsets resolve into concrete memory displacements. Basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host platforms.

Remaining open under STRUCTURAL-ACCESS: dynamic multi-field disjoint loans on aggregate slices, reborrow projection splits across nested variants, and conditional lifetime extension.

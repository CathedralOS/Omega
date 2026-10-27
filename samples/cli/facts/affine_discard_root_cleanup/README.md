# Affine Discard Root Cleanup

Fact: an unconsumed nominal resource whose drop contract requires valid == 1u64 executes deterministic affine scope-exit cleanup while preserving result <= 50u64 so 50u64 - result succeeds.

Owning Task: `RESOURCE-CLEANUP-MODEL`
Specification Reference: [Ownership](wiki/spec/terminal-psi/ownership.md), [Loans](wiki/spec/terminal-psi/loans.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md)

This fact row demonstrates that an unconsumed nominal resource with an affine drop contract (`machine Resource::drop(resource: &mut Resource) requires resource.valid == 1u64`) triggers deterministic scope-exit cleanup (`DiscardRoot` / `InvokeNominal`) across all nine standard places. The resource state satisfies the cleanup precondition (`resource.valid == 1u64`), allowing the cleanup action to be verified and statically discharged at compile time while writing `resource.valid` into caller-visible `Counter` state (`dropped.value == 1u64`) and preserving the computed value bound `result <= 50u64` so that exact subtraction `50u64 - result` succeeds and exits with code 0.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness creating Resource with valid == 1u64 and verifying scope exit drop | [`main.omg`](main.omg) | - |
| local | `local/` | the resource is held in a local variable and cleaned up at function exit | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the resource is wrapped in a nominal Holder record field and cleaned up via field drop | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the resource as an explicit parameter and cleans it up at return | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the resource which is received into a local and cleaned up | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the resource before scope exit cleanup | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant VALID_TAG initializes the resource tag before scope exit cleanup | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the resource to a successor machine before cleanup | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine receives the resource and performs deterministic drop | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the resource and cleans it up | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/affine_discard_root_cleanup/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/local/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/field/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/parameter/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/call_result/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/match_join/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/const_name/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/other_state/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/other_machine/main.omg
omega run --both samples/cli/facts/affine_discard_root_cleanup/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the resource valid tag is weakened to `0u64`, violating the drop contract precondition `resource.valid == 1u64`. Check each twin with:

```sh
omega --check samples/cli/facts/affine_discard_root_cleanup/local/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/field/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/parameter/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/call_result/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/match_join/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/const_name/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/other_state/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/other_machine/invalid/main.omg
omega --check samples/cli/facts/affine_discard_root_cleanup/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove automatic cleanup requires at selected edge ... does not establish Resource::drop clause \`resource.valid == 1\``

## Requirements and Evidence

- **Parent outcome:** `RESOURCE-CLEANUP-MODEL` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for deterministic affine scope-exit cleanup (`DiscardRoot`, `DiscardResidual`, and `InvokeNominal`) with automatic contract precondition checking and exact subtraction preservation across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Ownership](wiki/spec/terminal-psi/ownership.md#affine-cleanup), [Loans](wiki/spec/terminal-psi/loans.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Constructing `Resource { valid: 1u64, value: 30u64 }` establishes the precondition `resource.valid == 1u64` required by `machine Resource::drop(resource: &mut Resource) requires resource.valid == 1u64` and the scalar value bound `value <= 50u64`. When the unconsumed resource reaches scope exit, the compiler synthesizes a `DiscardRoot` / `InvokeNominal` cleanup call that statically discharges the `valid == 1u64` contract without requiring any runtime panics or caller equality guards (`transition x == 100`). Exact subtraction `50u64 - result` is proved directly from the preserved bound.
- **Identity and lifetime:** The nominal resource retains unique ownership throughout its scope. At scope exit or across terminal machine transfers, affine cleanup disposes the resource by borrowing `&mut Resource` for `Resource::drop` and retiring its storage without leaking unconsumed linear/affine obligations.
- **Composition:** Affine discard cleanup composes across all 9 standard places: local variable scope exit (`local`), nested record field drop (`field`), machine parameter cleanup upon return (`parameter`), function call return value binding (`call_result`), match arm convergence join (`match_join`), named constant initialization (`const_name`), state transition arrival and state scope exit (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in supplying `valid: 0u64`, violating the drop clause.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove automatic cleanup requires at selected edge ...: res does not establish Resource::drop clause \`resource.valid == 1\``. The compiler soundly refuses any program where an unconsumed affine resource cannot prove its drop precondition at every exit edge.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source drop declarations lower into target-neutral Terminal Psi (`PSITERM\0`) with explicit affine cleanup actions (`DiscardRoot`, `DiscardResidual`, and `InvokeNominal`). Erasure cleanly separates proof annotations while preserving `proof-term-tags`. Both Terminal Psi interpreter and native execution agree identically (`omega run --both`) on exit code 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning assigns 16 bytes for `Resource` (two 8-byte `u64` fields at 8-byte alignment) and 16 bytes for `Holder`. In `CallPlan` and `StatePlan`, resource references pass as mutable pointers in ABI registers (`rcx`/`rdi`/`x0`), and drop invocations follow standard calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions, emitting calls to `Resource::drop` before function return or state transition. Disjoint activation frames allocate spill and scratch slots for mutable references while composing worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow around synthesized drop epilogues, transitive machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## Discrimination: which obligation class this fact belongs to

This fact's obligation is a **cleanup/disposition obligation**, not a value/arithmetic
bound, a `where`-clause construction fact, or an index/range proof. The distinction
matters because it decides whether a runtime trap is reachable at all. Measured with
the built producer on this checkout:

- **Satisfied clause (control).** `Resource { valid: 1u64, value: 30u64 }` proves the
  drop clause at the selected exit edge; both engines report `exit 0`. The cleanup
  precondition is statically discharged, so no runtime check is emitted.
- **False call-site contract.** A caller supplying `exercise(0u64)` where the callee
  declares `requires tag == 1u64` refuses statically at the call gate:
  `machine main state entry value call to exercise violates required fact tag == 1:
  the instantiated fact is structurally false (0 == 1)`. No trap.
- **Range-admitting contract.** Binding the resource tag from a parameter whose
  contract admits a range (`requires tag <= 1u64`) — so constant folding cannot decide
  the tag and the range includes the violating value — **still refuses statically**:
  `cannot prove automatic cleanup requires at selected edge from exercise state entry
  after statement 1: res does not establish Resource::drop clause resource.valid == 1`.

That third probe is the load-bearing observation. For a value/arithmetic bound, a
boundary contract admitting a range defeats constant folding and typically yields a
runtime trap. A cleanup obligation does not behave that way: the disposition is decided
on the edge partition, and `check_cleanup_requirements` refuses the edge whenever the
clause is not implied, before any runtime realization exists. **No runtime trap is
reachable through this fact's obligation.** The nine reject twins therefore all refuse
statically; that is this obligation's class, not a missing probe.

The lever that changes the outcome is downstream of what the clause proves: the fact is
only observable when the disposition is *permitted*, which is why every positive place
makes the discarded root's disposition the thing the program depends on (`50u64 - result`
is proved from the bound the cleanup preserves).

## Fixture pins

The nine places and nine reject twins are pinned under
[`tests/omega/pass/facts`](../../../../tests/omega/pass/facts) and
[`tests/omega/fail/facts`](../../../../tests/omega/fail/facts) as
`affine_discard_root_cleanup[_through_<place>]_exit` and
`affine_discard_root_cleanup_through_<place>_rejected`. Each reject twin authors both
`sample.json` (`rejection_contains`) and `expected.txt`; `rejection_contains` takes
precedence.

## What remains open

Compound nested linear resource graphs with multiple non-trivial drop dependencies, conditional drop flags with partially moved fields, and asynchronous cancellation cleanup paths remain governed under the wider `RESOURCE-CLEANUP-MODEL` roadmap.

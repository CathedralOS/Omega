# Selected recursive result bounds

Parent: **FILESYSTEM-PLATFORM-SERVICES**, whose Windows recursive directory
creation exposed this shared compiler defect. This row is a compiler prerequisite,
not acceptance of the Windows filesystem provider. The current source migration
also belongs to **BUILD-OWNED-TARGET-SELECTION**: remove this row's retired
target prefixes without changing the bound, operation, or false-input control.

An acknowledged selected tail-recursive countdown carries its result bound
of at most 60 into exact subtraction `60 - result`. Each place copies this fact
and operation unchanged: local, field, parameter, call result, match join, const
name, a nested machine, another machine and another target body. The public `Countdown` requirement publishes that contract and bare termination;
one ordinary `CountdownCode` implementation retains the private `remaining`
ranking witness. Build selects it for all four targets because the former
bodies were identical. The target-body place also selects an ordinary
`EvaluationCode` implementation whose public requirement retains `invokes Countdown`.

Each positive binds all four hosted `ProgramEntry` targets and has exit 0 as its
oracle. Each `invalid/` twin changes only the supplied bound from 60 to 61 and
must reject the required `value <= 60` fact. The pass/fail copies under
`tests/omega/pass/facts/target_recursive_bound_through_<place>_exit` and
`tests/omega/fail/facts/target_recursive_bound_through_<place>_rejected` pin
the same row, byte-identical to the place programs.

## Historical target-family run record

Host macOS arm64 (Jarod's Mac mini), worktree base `9d9ba4fb625`, shared
generation producer `build/swarm/factsr1/producers/gen-9d9ba4fb625` (omega
sha256 `5a2fde4a3a91`, corpus_runner sha256 `98a455599b4d`, opt_level 1):

- All nine positives, `omega run --both <place>/main.omg`: `native exit 0` and
  `interp exit: 0`.
- All nine twins, `omega --check <place>/invalid/main.omg`: exit 1 with the
  authored `expected.txt` fragments `violates required fact` and
  `value <= 60`.
- Corpus legs recorded at `9d9ba4fb625+wt`: nine `checked` rows carrying
  `check-target:macos_arm64`, nine `rejected` rows carrying the diagnostics
  above, nine native `built ... exit:0` rows, and nine both-leg rows with
  `exit:0`, `interpreter-exit:0` and `interpreter:agrees`.

The nine reject fixture families were renamed from the bare
`target_recursive_bound_through_<place>` shape to the canonical
`target_recursive_bound_through_<place>_rejected` names; fixture bytes are
unchanged.

On the fresh `e2a6d051c734` baseline, all nine positives rejected with the same
NON-TAIL self-recursion diagnostic while target-neutral copies of the same
recursion checked: recursive-tail normalization ran before calls bound to the
exact target-body declaration. The repair (`8fa2ea70ef0`, one normalization
phase after final binding) is in this row's history; all nine places
checked and executed at native and Terminal exit 0 at that historical revision.

## Contracts and evidence

The governing contracts are [terminal calls](../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms),
[Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[runtime recursion](../../../../wiki/spec/language/termination.md#calls-and-productive-loops)
and [call markers](../../../../wiki/spec/language/effects.md#call-site-acknowledgements).

- Establishment: the supplied value and remaining count establish the recursive
  machine preconditions; strict countdown descent supports its progress promise.
- Identity and lifetime: only the exact selected self entry becomes a backedge;
  argument snapshots and any authored selection occurrence remain attached.
- Composition: the `result <= 60` contract travels through the nine places into
  exact subtraction, without a caller-side equality guard re-establishing it.
- Soundness: 61 violates the same required bound; separate call controls retain
  non-tail, receiver, overload, precondition and marker refusals.
- Psi 05–07: normalized checked state edges and erased contracts must preserve
  the result through Terminal Psi execution, including the acknowledged call.
- Omega 00–02: the Build-selected ordinary implementation and scalar argument/result calling plans
  must realize the same checked program; no target-specific source recognizer.
- Omega 03–06: register homes and activation state must preserve the countdown
  and value over the constant-stack cycle and blocking-call boundary.
- Omega 07–09 and emission: the native image must execute the same branch and
  subtraction result as Terminal Psi. Cross-production is not foreign execution.

Run a positive with `omega run --both <place>/main.omg`; both engines must exit
0. Run its twin with `omega --check <place>/invalid/main.omg`; it must reject the
authored precondition. Focused corpus checks use
`python3 tools/corpus_gate.py --both --filter facts/target_recursive_bound`.
Host/producer-bound observations belong in the qualification and claim records.
Windows native execution remains deferred to a Windows host, not inferred from
these commands on macOS.

## Current source contract

All nested blocking transition-arm calls acknowledge `block`, as required by
[call-site acknowledgements](../../../../wiki/spec/language/effects.md#call-site-acknowledgements).
The current baseline rejects all nine old positives for their missing `step`
acknowledgement; this is a source migration, not a claimed compiler repair.
Reject twins still supply only 61 instead of 60 and fail the same `value <= 60`
obligation. Ordinary requirement calls report `cannot prove requires contract`;
the other-machine twin still fails at `Worker::deliver` with `violates required fact`.

The match-join place keeps the input 60, result bound 60, and exact subtraction.
Its two arms additionally write 17 and 23 through an ordinary caller-owned marker;
main checks both markers, so swapped or default-only dispatch traps. These markers
supply no static bound and cannot justify the subtraction.
The pure `tick` and countdown return do not promise an observable iteration
count: this row establishes the carried result bound, not a separate claim that
native optimizers execute each pure countdown step. Filesystem recursion and
other observable cycle behavior remain their own acceptance.

## Current migration evidence

On macOS arm64, the retained optimized `f6273e83f99c` corpus producer checks
all nine positives; all nine reject twins refuse the supplied 61 with their
authored bound diagnostic. Its native/Terminal run builds all nine positives,
with exit 0 from each engine and agreement. The retained optimized `fbd577583f38`
CLI separately runs the nine authored sample programs with `--both`, each exit 0;
all nine sample controls reject the same bound. Producer source, effective
profiles, exact source/oracle inputs and raw observations are retained in
`build/target-recursive-retirement-20261101` in the working checkout.
The only producer-source difference is corpus inspection support from the
preceding migration, which these programs do not invoke; production compiler
and SDK inputs remain the fixed base. These observations are not executions
of a newer main compiler.

Invalid sample folders use `expected.txt` for the same rejection fragments as
their pinned fixtures. Redundant `sample.json` overrides are removed; the folder
name still classifies each as intended-invalid. No separate oracle can drift.
No foreign-host execution or broader filesystem capability closure is claimed.

# Target-scoped recursive result bounds

Parent: **FILESYSTEM-PLATFORM-SERVICES**, whose Windows recursive directory
creation exposed this shared compiler defect. This row is a compiler prerequisite,
not acceptance of the Windows filesystem provider.

An acknowledged target-scoped tail-recursive countdown carries its result bound
of at most 60 into exact subtraction `60 - result`. Each place copies this fact
and operation unchanged: local, field, parameter, call result, match join, const
name, another state, another machine and another target body. Four authored target
bodies share the contract; all four must check in every compilation.

Each positive binds all four hosted `ProgramEntry` targets and has exit 0 as its
oracle. Each `invalid/` twin changes only the supplied bound from 60 to 61 and
must reject the required `value <= 60` fact. The pass/fail copies under
`tests/omega/{pass,fail}/facts/target_recursive_bound_through_*` pin the same row.

On the fresh e2a6d051c734 baseline, all nine positives reject with the same
NON-TAIL self-recursion diagnostic. Target-neutral copies of the same recursion
check; target-scoped copies do not. The failure is premature normalization before
calls bind to the exact target-body declaration, not a new source-language rule.

## Contracts and evidence

The governing contracts are [terminal calls](../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms),
[target families](../../../../wiki/spec/language/machines.md#target-scoped-families),
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
- Omega 00–02: the target-family choice and scalar argument/result calling plans
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

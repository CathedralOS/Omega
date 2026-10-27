# Saved case predicate currentness

A saved case verdict selects the same unwritten case payload after a disjoint case replacement.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, remaining case saved-observation composition. A copied `choice in Choice::Full` Boolean passes through an alias and a replacement of the other Choice. Its true arm must justify the current Full payload and the nonzero divisor in `490 / choice.amount`; runtime must return 70. Each twin changes only the replacement destination from the other Choice to the observed Choice. The Boolean survives, but cannot authorize accessing its now-absent payload.

Fixed optimized baseline `eed37e0d2571646b78af87c1f6cc691ec176302e` checks the five owned-value places, but wrongly refuses the parameter, nested-machine, other-machine and target-body places when the tag receiver calls `capture`, which clears only the other Choice. It also refuses the further receiver production that writes Full before reading its tag. All ten replacement twins reject on that baseline. Every stale-case twin rejects. The repaired producer runs all nine positives with native exit 0 and Terminal exit 0 on macOS ARM64; all nine twins reject the intended selected-case obligation. The programs and twins are pinned under `tests/omega/{pass,fail}/facts/saved_case_predicate_through_<place>_{exit,rejected}/`.

Governing contracts: [mutation and subject identity](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity), [patterns](../../../../wiki/spec/language/patterns.md), and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md). No new language rule is proposed.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [stale replacement](local/invalid/main.omg) |
| field | [program](field/main.omg) | [stale replacement](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [stale replacement](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [stale replacement](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [stale replacement](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [stale replacement](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [stale replacement](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [stale replacement](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [stale replacement](target_body/invalid/main.omg) |

The guard retains the evaluated decision separately from a reusable current
expression. A call-containing initializer still has an actual completed tag
read. Its original statement identifies that receiver and write epoch; the
later alias use supplies the consumer point, not another evaluation.

Write prefixes restart at every statement. Currency therefore consumes the
producer's remaining writes, every intervening statement and the consumer's
prefix through the same directed interval used for invocation operands. Writes
before the read do not invalidate its new tag; later disjoint writes preserve
it, and the twins replace that exact storage and lose payload authority.
Shared storage stability admits only an authenticated invocation referent.

The [write-before-read program](writes_before_read/main.omg) and its
[replacement twin](writes_before_read/invalid/main.omg) are another production
of this same observation requirement. They return and test a reference after
writing Full through it. The further trailing-write fixture under
`tests/omega/fail/control_flow/saved_case_predicate_receiver_trailing_write_rejected/`
replaces the case inside the saved initializer after its tag read; the completed
Boolean cannot restore the retired payload fact.

The requirement evidence is bounded to this operation:

- Establishment: `Full` construction establishes the nonzero payload domain;
  the true saved case test selects that payload before division.
- Identity and lifetime: Boolean copies retain the original observation;
  disjoint and overlapping replacements distinguish current storage facts.
- Composition: the nine places carry the same case value and quotient operation
  through fields, arguments, results, joins, constants, lexical scope and targets.
- Soundness boundaries: no caller equality guard re-establishes the fact;
  every stale replacement rejects without granting access or changing loans.
- Psi 05–07: actual Terminal execution exercises case dispatch, payload access,
  reference arguments and call effects. This row adds no proof-only construct.
- Omega 00–02: hosted Build bindings select the entry and realize case layout
  and reference/value call placements. This row does not cover stack-argument
  thresholds, foreign bindings or aggregate return placement generally.
- Omega 03–06: the host realizes calls, payload loads, integer division and
  normal/crash routes with its ordinary register and frame machinery.
- Omega 07–09: the emitted macOS ARM64 image exits 0 and agrees with Terminal
  execution. Other hosted bindings and target bodies are checked; Windows and
  Linux execution remain unverified on this machine.

Producer: source `ec25f66f10a33964c7cb7919abe8dd68310b8679`, optimized
executable SHA-256 `bae86e60fa9e508db5b8af1c54dc9553fa008bf72d71065a877d7415c5beb08f`.
All ten positive programs run with native and Terminal exit 0 on macOS ARM64;
all ten replacement twins reject selected-case access. A separate fresh-tag
probe starts Empty and observes the Full value written by the receiver call,
so its exit-zero result requires that call's effects. Negated receiver tests
challenge the opposite polarity, and stale/trailing-write controls must reject.
The audited corpus run completes 49 programs with native/Terminal exit 0, including this row, saved numeric/byte snapshots, stored loans, descriptor replacement and reference projection. Field and tag observations share the checked fresh-receiver retirement and normal-completion route: the reference carrier ends after the read, before its backing owner; an established outer operand keeps its enclosing consumer lifetime. Target reference custody remains independently checked.

The parent remains open for remaining generic/reference saved observations,
single-evaluation pattern destructuring, and matching-host Windows/Linux
execution. This row establishes this case-observation requirement, not all
remaining STATE-LOCAL-VALUE-FRONTIER acceptance.

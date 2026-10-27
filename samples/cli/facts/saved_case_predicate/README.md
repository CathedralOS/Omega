# Saved case predicate currentness

A saved case verdict selects the same unwritten case payload after a disjoint case replacement.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, remaining case saved-observation composition. A copied `choice in Choice::Full` Boolean passes through an alias and a replacement of the other Choice. Its true arm must justify the current Full payload and the nonzero divisor in `490 / choice.amount`; runtime must return 70. Each twin changes only the replacement destination from the other Choice to the observed Choice. The Boolean survives, but cannot authorize accessing its now-absent payload.

Optimized baseline `eac9952c077414c4b91e4d3dcd7c7fdb46aa31eb` accepts the local, field, call-result, match-join and const-name positives. Parameter, nested-machine, other-machine and target-body positives wrongly refuse with `cannot prove selected case for payload access`. Every stale-case twin rejects. The repaired producer runs all nine positives with native exit 0 and Terminal exit 0 on macOS ARM64; all nine twins reject the intended selected-case obligation. The programs and twins are pinned under `tests/omega/{pass,fail}/facts/saved_case_predicate_through_<place>_{exit,rejected}/`.

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

The guard records its original evaluation statement. That statement identifies
its actual receiver and write epoch; the later alias use supplies the consuming
program point, not another evaluation. Shared storage stability admits only an
authenticated invocation referent. Disjoint writes preserve the observed tag;
the twins replace that exact storage and lose payload authority.

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

Producer: source `1b63cc4aae74a9386b7ae1ec1c89394de332c5cb`, optimized
executable SHA-256 `75faeda8aa69f86b499b28e915fbf9609a9ec70456d67b0f192445ae7a0769bc`.
The subsequent fixture edit supplies the nested crash helper's own explicit
crash contract; it does not change the fact, bound or quotient operation.

The parent remains open. Case tests whose receiver expression itself calls a
reference-returning machine still lose the saved fact, including when that
call writes before the tag read. These are further productions of this same
observation requirement, not separate fact rows. The overlapping trailing-write
control still rejects. Generic/reference and single-evaluation destructuring
composition also remain on `TASKS.md`.

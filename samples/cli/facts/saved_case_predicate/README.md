# Saved case predicate currentness

A saved case verdict selects the same unwritten case payload after a disjoint case replacement.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, generic/reference saved case observations. Each place passes its case through an opaque `Value` constrained by `Value == &'storage mut Choice`. A saved `Choice::Full` verdict survives an alias and a write to the other Choice. It must justify the current payload and the nonzero divisor in `490 / choice.amount`.

Every positive invokes both Full (quotient 70) and Empty (fallback 0), and traps if either result is wrong. The field place checks neighboring values 11 and 13; all places observe the disjoint clear. Each twin changes only the post-observation replacement destination from the other Choice to the observed Choice. Its saved Boolean survives, but cannot authorize accessing the replaced payload.

Fixed optimized baseline `a809ca2a41c21115180dce9d1ca68e94d90ebf65` refuses all nine positives while parsing the lifetime-qualified type operand. The parser checkpoint then exposes early equation matching's lifetime restriction. Specialization must retain the formal region relation rather than substituting an anonymous actual into both the input and return types. Expression-match consumers share transition guards' saved-observation publication, and completed stores retain conditional payload domains without selecting a case.

Governing contracts: [type equality and inference](../../../../wiki/spec/language/generics.md#static-type-equality), [returned lifetimes](../../../../wiki/spec/language/lifetimes.md#returned-views), [patterns](../../../../wiki/spec/language/patterns.md), [mutation and subject identity](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity), and [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md). No new language rule is proposed.

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

The [inferred application](inferred/main.omg) and its [stale-write twin](inferred/invalid/main.omg) exercise another production of the same fact: the ordinary borrow arguments determine `Value` before the equation is discharged, keeping the callee's invocation lifetime out of the caller's static argument scope. Declaration-order, reference-access, pointee, undeclared-region and wrong-return-source controls live beside generic fixtures under `tests/omega/`.

The nine places and inferred application are pinned under `tests/omega/{pass,fail}/facts/saved_case_predicate*`. On macOS ARM64, optimized producer source `914d1adfdf4` executes the nine places with native and Terminal exit 0; their nine twins reject selected-case access. The authored four-target Build rows and target bodies remain checked. Windows and Linux execution require matching hosts.

The parent remains open for its remaining saved numeric/byte observations and single-evaluation pattern destructuring acceptance. This row closes the generic/reference case-observation requirement only.

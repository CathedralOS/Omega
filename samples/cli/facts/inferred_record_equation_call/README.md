# Inferred record constructor index

Fact: an omitted Setting is inferred from the exact record-valued Token index and consumed as Setting.minimum.

This row advances **STRUCTURAL-GENERIC-MATCHING**. The frozen [root](main.omg)
uses `read(item)` to infer `Setting` from `Token<SEVEN>` and `Item == Token<Setting>`.
The consumer returns `Setting.minimum`, authored as `7`, independently of runtime
marker contents. Every positive program traps unless that result is observed and
expects exit zero. The root source is unchanged from the baseline reproduction.

Every positive binds all four hosted targets. Runtime evidence is from Linux
x86-64 only; foreign target declarations are not foreign-host execution.

| Place | Positive | Explicit conflicting argument |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Each twin (including [root invalid](invalid/main.omg)) fixes `Setting` explicitly
to `EIGHT` against the unchanged `Token<SEVEN>` actual. It must reject with
`has conflicting constructor constants`. The corresponding pass/fail fixtures are
under `tests/omega/{pass,fail}/facts/inferred_record_equation_call`.

## Requirement and execution path

Authoritative contracts are [structural inference](../../../../wiki/spec/language/generics.md#structural-type-equations-and-inference),
[applications](../../../../wiki/spec/language/generics.md#applications-and-specialization),
and [canonical indices](../../../../wiki/spec/language/evaluation.md#canonical-static-identities).
Run each positive with `omega run --both <place>/main.omg`; both engines must
observe exit 0. Check each twin with `omega --check <place>/invalid/main.omg` and
its authored diagnostic. Generic fixtures also exercise equal explicit indices,
repeated equal/conflicting values, direct record literals, qualified constants,
wrong heads/carriers, and data-equation reconstruction.

- **Establishment:** exact declared actual type plus the constructor equation
  determines the omitted binder. The runtime result comparison is an oracle,
  not evidence used to choose the static argument.
- **Identity and lifetime:** constructor and record carrier retain exact nominal
  identities. Canonical value equality ignores constant spelling. Selected
  constant origins restore a path only when it selects the exact retained header
  in the original lexical scope. This row introduces no loans or mutation.
- **Composition:** one indexed type reaches the consuming call through all nine
  places; both match arms and state/call arrivals retain it. The consuming body
  reads the inferred record field directly.
- **Soundness:** explicit/repeated conflicts, wrong constructors and wrong declared
  carriers reject. Existing kind, occurs-check, selected-constructor obligations
  and unsolved-equation rejection remain required.
- **Psi 05–07:** the solver retains non-scalar const references and compares their
  canonical values. Machine completion restores the exact source operand for
  ordinary resolved const admission. Existing closed-recipe normalization runs
  after source formation before tuple selection, including nongeneric callers.
  The specialized consumer lowers to target-neutral Terminal Psi.
- **Omega 00–02:** ordinary Build entry rows, record geometry and scalar result
  calling plans realize the checked program. Inference adds no target policy.
- **Omega 03–06:** existing scalar/record instruction selection, register and
  activation storage carry the result through calls and transitions.
- **Omega 07–09 and emission:** normal image emission and installation produce a
  native executable whose authored result agrees with Terminal execution.

The parent task remains open for forwarding, retained extensions, lifetime
identity, additional expression combinations and unspecialized alternatives.
This does not establish full Omega-written product compiler completion.

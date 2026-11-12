# Validation subjects

Ratified 2026-11-08. A one-line `omega --check` ran Psi 05 on 837 machines, of
which its product reached eight; the rest was the bundled library, checked in
full on every user compile because every compile claimed to validate all
authored source. The rule now: a check fully validates the packages it was
asked to validate and freshly checks only the dependency declarations those
need. A library is checked in full when it is itself the subject: its own
development, a workspace check, or a project admitting, updating or auditing
it. There is no optimization flag. Exact-key reuse of a checked dependency
slice is allowed (owner ruling 2026-11-12).

The normative owner is [validation subjects](../../spec/build/validation_subjects.md),
with touch points in [optimizations](../../spec/build/optimizations.md#phase-and-product-boundaries),
[component publication](../../spec/build/component_publication.md),
[acceptance](../../spec/packages/acceptance.md#ordinary-compilation),
[workspaces](../../spec/build/declarations.md#workspaces) and the
[command line](../../spec/build/command_line.md#command-vocabulary). Track
implementation under `VALIDATION-SUBJECTS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

- Subject set by request: the selected package; every declared member for a
  workspace check with no member named; the admitted revision at admission;
  plus any dependency a product names with `Build::validate`. Locality,
  bundling and editability do not make a subject.
- Closure from every subject declaration, including unused ones, plus
  executed Build and preparation entries, product entries and selection
  obligations; following calls, callable values, callbacks, cleanup, types and
  invariants, contracts, proofs, lemmas, selected conformances and laws,
  layouts and bindings. Target obligations are exactly what the selections
  commit to, under the existing provider-selection rule.
- Subject survives orchestration: a dependency's Build execution demands its
  closure, never the dependency's full inventory.
- Dependencies are discovered while checking; deferred bodies stay in name
  resolution; a demanded declaration gets every ordinary check.
- `Build::validate(builder, dependency)` records a requirement, only widens,
  names a directly registered dependency, is owned by the root Build
  activation, and is satisfied by the ordinary pipeline before success.
- Full validation of a revision at admission, update and audit; an invalid
  declaration is a validation failure. A consumer's edit does not promote its
  unchanged dependencies to subjects.
- Exact-key reuse of dependency slices only (owner ruling 2026-11-12); never
  the consumer's own package or Build-selection-dependent obligations.
  `--verify` unchanged. No program-scope or optimization
  setting narrows a subject. Pruning stays after checking within the scope.
- Precondition: the toolchain release gate checks each bundled library as a
  subject before the consumer default changes.

## Shim

A separate package that consumes an application as a dependency checks that
application only through its closure. That is the rule applied honestly, not
a loophole: nothing executed, proved with, cleaned up or laid out escapes, and
the author has declared that the unreached code is not theirs to validate in
that check. The compiler certifies the requested scope and does not guess which
package is the "real" one.

## Implementation

1. Measure the bundled-library gate: does the release workflow check core and
   std as subjects, including unused declarations? Add the explicit
   full-package job where it does not.
2. Make the subject set explicit request state and carry it through
   orchestration, including dependency Build execution and generated source.
3. Replace post-hoc graph filtering with discovery during checking. Extend the
   dependency graph with type and data references, which the validation-subjects
   audit did not traverse.
4. Add `Build::validate`, the workspace default, and full validation of the
   admitted revision in install, update and audit. How `Build::validate`
   names a dependency, and how a bundled library becomes a subject, waits on
   the owner question for `validation-subjects` (`Build::validate` naming) in
   [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md).
5. Run the acceptance cases, measure which bodies are checked, including
   preparation and update steps, then switch the consumer default.

## Acceptance

| Case | Required result |
| --- | --- |
| Unused invalid function owned by the subject | Check fails |
| Unrelated invalid body in a dependency | Consumer check may pass; checking that dependency as a subject fails |
| Unused subject helper references that invalid dependency body | Consumer check fails |
| Dependency needed only through a type, proof, cleanup, callable value or conformance | Its required checks run and reject if invalid |
| Dependency Build executes during preparation | Its closure checks before execution; unrelated dependency bodies stay outside the job |
| Dependency gains an unrelated valid private function | Consumer body-checking work does not grow |
| `Build::validate(core)` on the app | Invalid unused `core` function fails the app build; a dependency's Build cannot add subjects |
| Optional optimizations disabled | Coverage and verdicts unchanged |

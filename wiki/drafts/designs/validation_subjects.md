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

The 2026-11-12 completion selects borrowed registration handles for
`Build::validate`, reusable registration identity shared with preparation, and
explicit package roots with a narrow core-owner rule. It also ratifies
`ordinary-library-packages`: optional distribution does not change ordinary
`std`/`alloc` ownership or confer compiler custody. The two delivery tracks
below are independent; removal of all demonstrated `std` coupling must not
block the validation API/root repair. No new implementation or execution result
is claimed here.

## Decisions

- Subject set by request: the selected package; every declared member for a
  workspace check with no member named; the admitted revision at admission;
  plus any direct dependency selected by its registration with `Build::validate`. Locality,
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
- `Build::validate(builder, &registration)` borrows an exact direct dependency
  registration from the supplied root Build, in Build or Product scope. It
  records a full-check request, only widens subjects, and is satisfied by the
  ordinary pipeline before success; it does not immediately execute preparation.
- The immutable opaque registration is copyable and activation/purpose-scoped.
  Preparation and validation reuse its one coordinator identity; neither an
  empty record nor variable spelling can identify another registration. Handle
  copies survive preparation, but another/expired activation cannot use them.
  Completed requests retain the resolved immutable instance and checking context.
- Full validation of a revision at admission, update and audit; an invalid
  declaration is a validation failure. A consumer's edit does not promote its
  unchanged dependencies to subjects.
- Exact-key reuse of dependency slices only (owner ruling 2026-11-12); never
  the consumer's own package or Build-selection-dependent obligations.
  `--verify` unchanged. No program-scope or optimization
  setting narrows a subject. Pruning stays after checking within the scope.
- Precondition: the toolchain release gate explicitly checks each library it
  distributes as a subject before the consumer default changes. Ordinary
  package roots retain their resolved owners; checking existing core preserves
  its toolchain owner without creating a general bundled-package category.

## Shim

A separate package that consumes an application as a dependency checks that
application only through its closure. That is the rule applied honestly, not
a loophole: nothing executed, proved with, cleaned up or laid out escapes, and
the author has declared that the unreached code is not theirs to validate in
that check. The compiler certifies the requested scope and does not guess which
package is the "real" one.

## Implementation

1. Measure the full-library gate: does the release workflow explicitly check
  the libraries it distributes, including unused declarations? Add ordinary
  full-package jobs where missing, retaining exact owners and the narrow
  existing-core root selection. The overlap guard protects the core subtree;
  do not assume it also blocks `std` or `alloc` without separate evidence.
2. Make the subject set explicit request state and carry it through
   orchestration, including dependency Build execution and generated source.
3. Replace post-hoc graph filtering with discovery during checking. Extend the
   dependency graph with type and data references, which the validation-subjects
   audit did not traverse.
4. Add borrowed-handle `Build::validate`, the workspace default, and full
  validation of the admitted revision in install, update and audit. Replace
  the fieldless registration result with its real opaque activation-scoped
  identity; keep preparation/validation reuse and Build/Product scope exact.
  Retain requests by resolved instance/context, idempotently, after Build ends.
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
| `Build::validate(builder, &registered)` on a direct dependency | Its invalid unused declaration fails the product check; another Build activation cannot add subjects |
| Optional optimizations disabled | Coverage and verdicts unchanged |

### Registration and root controls

- Borrowing or copying one handle identifies one registration before and after
  preparation. Repeated validation requests are idempotent by exact resolved
  instance and checking context. Preparation cannot accidentally consume the
  caller's handle or replay earlier Build statements.
- Identical aliases in Product and Build scope identify different registrations;
  validate each in its corresponding execution/checking context. Invalid,
  fabricated, stale or cross-activation handles reject. A helper using the
  actual root builder and its handle is permitted.
- Recording validation does not immediately execute or certify the dependency.
  The ordinary pipeline must satisfy it before success, and full checking does
  not execute every declaration. Transitive packages retain ordinary demanded
  closure unless independently selected as full subjects.
- Reports/publication retain resolved immutable instance and context, not a
  coordinator pointer or a name string. Different contexts are not deduplicated
  merely because a path, alias or source revision agrees.
- Qualify explicit checks of `source/library/core/main.omg`,
  `source/library/std/main.omg` and `source/library/alloc/main.omg` as their
  existing package roots. Core keeps its toolchain owner; ordinary packages keep
  their resolved owners. Re-rooting must not create another core or label the
  library as the application's package.
- Unrelated overlapping roots, a directory merely named core, and attempts to
  install or approve replacement primitives retain their rejection controls.
  Full-package validation preserves compiler-realization obligations rather
  than inventing source bodies for compiler-supplied operations.

## Ordinary library migration

This separate outcome removes demonstrated coupling under
[ordinary-library-packages](../../spec/build/declarations.md#ordinary-library-packages).
Current implementation evidence motivating it is specific:

- Library discovery requires both `core/` and `std/build.omg`, even when the
  requested program needs no `std`. Remove that unconditional dependency without
  disabling integrity verification for declared distribution components.
- The sealed catalog embeds hosted console/process-exit and related `std`
  declarations. Separate only genuine compiler-supplied primitives and necessary
  entry/ABI bridges into explicit exact bindings, keeping ordinary library work
  and wrappers in their selected package. Do not copy all of `std` into core.
- Entry-contract discovery can use package-graph suppliers but otherwise captures
  bundled `std` source with toolchain-contract custody. Replace that hidden path
  with normal target-declared dependency and contract selection; do not silently
  mix bundled declarations with another resolved compatible revision.

A whole-library release digest is legitimate content verification, not evidence
that nominal package owners were merged. Keep distribution content, declaration
owner, exact primitive custody and selected dependency instance distinct.
The packaging and catalog repairs must preserve primitive correspondence and
release integrity instead of removing checks that exposed the coupling.

Acceptance for this independent migration:

1. A package needing no `std` checks without an installed `std` copy, given its
   required core and target support. No unchecked entry fallback supplies it.
2. `std` checks as an ordinary package root with its resolved owner, source
   revision and dependencies. Distribution location grants no special role.
3. Selecting another compatible `std` revision uses that exact dependency and
   its declared target support without bundled-source mixing or compiler custody
   inferred from the package name, path or signature.
4. A package imitating a compiler-recognized declaration, including copied source
   bytes, receives no primitive custody without the exact approved binding and
   owner correspondence. Required genuine compiler bridges remain checked.
5. Distribution verification still rejects missing/changed components that the
   selected release promises. Optional libraries are accounted for explicitly;
   making `std` optional is not an excuse to skip the release manifest/digest.

These are acceptance requirements, not commands or tests run for this
documentation change. Runtime-provider policy is settled separately as
`sealed-runtime-providers`
([sealed compiler intrinsics](../../spec/build/foreign_bindings.md#sealed-compiler-intrinsics)),
and library renaming is `LIBRARY-RESPONSIBILITY-NAMES` on
[TASKS.md](../../../TASKS.md), under
[library responsibilities and names](../../spec/build/declarations.md#library-responsibilities-and-names);
this settlement authorizes neither.

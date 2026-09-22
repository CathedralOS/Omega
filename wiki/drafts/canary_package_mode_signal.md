# Canary package-mode signal — design

Design record for **CANARY-PACKAGE-MODE-SIGNAL** (TASKS.md, new-scope row).
Audited at `6f918986063` on linux x86-64. Delete this design once
`fixture_declares_ordinary_std` selects on the parsed build declaration and
the `repository_build_declarations.rs` named exception is gone.

## Problem

`fixture_declares_ordinary_std`
(`omega-rust/omega/compiler/compiler/tests/canary_suite.rs:3410-3413`) selects
"package mode" with two substring checks — `builder.depend(Source::Path` and
`source/library/std` — on the raw `build.omg` text. When both match,
`repository_fixture_package_inputs` (:3418) wraps the fixture in a synthetic
two-package closure: the root package plus a hard-wired
`omega-language-std` binding pointing at `repo_root()/source/library/std`.
The authored `location` is never resolved, normalized, or compared.

Two witnessed defects at `2a9f9c02ad`:

- `tests/omega/pass/proofs/kernel_theorem_equality_certificates` declared its
  std edge with six `../` segments where five reach the repository root. The
  authored path pointed outside the repository; the substring made it pass.
- `tests/omega/pass/proofs/quotient_define_managed_compile` declares an std
  edge it never imports. The edge is load-bearing because the substring *is*
  the mode signal: removing it demotes the fixture to standalone-source
  identity, which the managed `Quotient::define` admission refuses
  ("declaration `EquivalenceClass` has non-hermetic source origin `User`").
  The edge is retained today as a named exception in
  `packages/manager/tests/repository_build_declarations.rs` (~:488).

Both reduce to one defect: the harness infers *how* to compile a fixture from
*a substring in its build file* rather than from the parsed build declaration.

## Why the signal cannot be "has a build declaration"

2039 pass fixtures carry `main.omg`; 1381 carry `build.omg`, and 153 of those
(147 with no `depend` call at all) never mention std. Selecting package mode
from `build.omg` presence alone would silently re-wrap 153 fixtures that today
compile under standalone-source identity — a corpus-wide behavior change the
row does not ask for. The signal must be an authored opt-in to package mode
that does not require a consumed std edge.

## Design

**Signal: the parsed build declaration, not the raw text.** The harness
already runs `extract_build_declaration` (build-declarations crate) on every
packaged fixture. Extend that read to `project_dependency_rows`
(`build-declarations/src/dependencies/mod.rs:205`), which yields one
`DependencyRow` per `builder.depend` / `depend_as` / `build_depend` /
`build_depend_as` call on the `builder` receiver — already grammar-checked
(`WrongReceiver` / `WrongArguments` reject malformed rows), so no second
parser is needed.

Package mode selects on a two-part rule:

1. Any retained `DependencyRow` in `DependencyPurpose::Product` — a fixture
   that declares a product dependency edge is a package; the harness resolves
   each declared `Source::Path` location against the project root, normalizes
   `..` segments, and requires the canonical result to name an existing
   directory carrying its own `build.omg`. A declared path that fails any of
   those checks refuses the fixture instead of passing it through the
   substring. `kernel_theorem_equality_certificates`'s escaping path rejects
   here.
2. An explicit mode marker for fixtures that need package identity with no
   consumed dependency — the row's `quotient_define_managed_compile` shape.
   The honest spelling is a declared application/package with a **resolved
   dependency set that may be empty**. Two candidate carriers, cheapest first:
   - *Harness-side:* the fixture roster gains a `PACKAGE_MODE` marker column
     (the rosters in `tests/fixture_rosters/` already enumerate every
     fixture's role) and `repository_fixture_package_inputs` consults it
     instead of the substring. Zero language surface; the opt-in is visible
     at the roster the suite already audits.
   - *Source-side:* a named `builder` operation — e.g.
     `builder.package_mode()` — recognized by `project_dependency_rows`'s
     sibling classifier as a mode row with no edge. Stronger provenance
     (the mode travels with the fixture), but spends new build-declaration
     vocabulary on a harness concern.

   Recommendation: start harness-side. The roster is already the suite's
   authority for fixture roles, keeps the language surface untouched, and
   makes the opt-in enumerable — `assert_mixed_canary_category_standard_
   library_edges` can then assert "mode iff consumed edges or roster marker"
   instead of "substring present".

**Package construction stays declarative.** `repository_fixture_package_
inputs` keeps the two-node closure it produces today (root identity +
`omega-language-std` at `repo_root()/source/library/std`), but each resolved
`Source::Path` dependency must canonicalize to that std directory before the
binding is attached — the authored path and the wired location are compared,
not just the substring. A declared edge to any other resolved path is a
fixture the harness cannot yet synthesize and fails with a named diagnostic,
not a silent substring miss.

## Acceptance mapping

| Row acceptance | Mechanism |
|---|---|
| Package mode selected from something other than the std substring | Parsed `DependencyRow` / roster marker replaces `fixture_declares_ordinary_std` |
| Fixture compiles in package mode with no std edge | Roster marker covers the zero-dependency package; `quotient_define_managed_compile` drops its inert edge and keeps package provenance |
| Unresolvable declared path refused | Canonicalize-then-exists check on every `Source::Path` location; the exception in `repository_build_declarations.rs` is deleted |

## Implementation legs

1. `canary_suite.rs`: replace `fixture_declares_ordinary_std` with a parsed
   read — `extract_build_declaration` + `project_dependency_rows` +
   canonical-path resolution; thread the roster marker for the zero-edge
   opt-in. Same gate feeds `entry_free_fixture_build`,
   `hosted_main_program_entry_build_for`, and
   `cross_target_program_entry_build` (all three currently re-run the same
   substring probe at :3685 / :3704 / :3735).
2. `packages/manager/tests/repository_build_declarations.rs`: delete the
   `quotient_define_managed_compile` named exception; the proofs category
   re-counts to 15/12 (the fixture moves from the std-declaring side to the
   package-mode-marked side).
3. Negative coverage: a scratch `build.omg` declaring a `Source::Path` that
   escapes or misses the repository refuses with a named diagnostic — the
   regression pin for the six-`../` defect.

## Open coordination

- `canary_suite.rs` and the fixture rosters are shared swarm surfaces; the
  migration touches no compiler behavior, only the harness's compile-input
  selection.
- Source-side `builder.package_mode()` vocabulary, if ever wanted for
  authored projects rather than test fixtures, is a separate build-language
  decision — out of this row's scope.

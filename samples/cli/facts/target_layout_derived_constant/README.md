# Derived target-layout constants

Owning requirement: **TARGET-SEMANTIC-APPLICATIONS**. The fact is: a target-selected
layout query contributes to a derived constant whose result discharges exact
subtraction and agrees with execution.

The primary program is frozen at SHA256
`d5105d7f9f49adb831f7e2eaa3656e7d84d4714d2beec01b8617874e209cd94f`.
It computes `const OBSERVED_SIZE: u64 = size_of<Main>() + 1u64`, transports that
value, and performs `observed - 5u64` before any equality guard. The runtime
branch traps unless both the exact value and subtraction result agree with the
target's authored expectation. Linux selects extent 4 and therefore value 5;
Windows/macOS select extent 8 and therefore value 9.

Original failing baseline: `d6cf554fd1cbf20551983d29bae26f5cf3b81664`. Its optimized
CLI (opt-level 1, SHA256
`7eb4e799f088a9940752614a8f6b53a6dc43349f5e12d3030233cac0f520617e`)
runs the existing direct-query control with native and Terminal exits 0, but
rejects this unchanged primary in 0.341 seconds with CLI exit 200:
`constant initializer: constant call lost its exact authored target selection`
at line 22. The baseline build took 10m45s. Existing main already included
`6e37ba29d416499f1f717aa51452e13fa639853a`'s const-name provenance repair;
this advance uses it.

Acceptance: `omega run --both main.omg`, and the same command for each place.
All positive projects bind all four hosted `ProgramEntry` targets and promise
exit 0. Historical qualification used Linux x86-64 native plus Terminal;
macOS arm64 replays retain their own producer and host identities. Explicit
target compilation alone is not execution. The source-backed
`mbx nextest run -p compiler --test package_review historical_constant_closure_requires_exact_target_and_provider`
also executes the unchanged Windows-target program in Terminal, whose authored
branch requires value 9. It emits a Windows native product without executing it.
Windows and Linux ARM64 native execution remain unverified.

| Place | Carrier of the same value | Pinned fixture suffix |
| --- | --- | --- |
| local | immutable local | `through_local_exit` |
| field | `SizeBox.observed` | `through_field_exit` |
| parameter | identity machine parameter/result | `through_parameter_exit` |
| call result | `read_size()` | `through_call_result_exit` |
| match join | both value arms | `through_match_join_exit` |
| const name | named derived constant | `through_const_name_exit` |
| other state | state arrival parameter | `through_other_state_exit` |
| other machine | attached machine result | `through_other_machine_exit` |
| target body | target-scoped family result | `through_target_body_exit` |

The nine fixture pairs are
`tests/omega/{pass,fail}/facts/target_layout_derived_constant_through_*`.
Each `invalid/main.omg` changes only Small's and Large's provider result to 3.
The unchanged subtraction then rejects with target bounds `[-1, -1]` outside
`u64`. The root has the same reject twin. No caller equality guard establishes
the subtraction fact.

The repair follows the selected initializer dependency closure, rather than
recognizing a query only at its root. Target observations defer whole scalar
expressions and their aliases/helpers. After validated layouts close, the
ordinary scalar evaluator completes dependencies before consumers, preserves
source-call inventories, and consumes canonical values at exact selected const
uses. Call provenance does not choose the enclosing arithmetic operator.
Checked specializations close their own query subjects; their arena coordinates
are never read as identities in the separate authored forest. The previous
query-only leaf exemption and Omega's direct-query declaration stamping are
removed.

Query geometry retains its own constant inputs, independently of actual stored
values. The ordinary layout producer reports every pending by-value extent;
the selected evaluator receives those exact source positions and closes the
pending initializer's authored and materialized observations. Source jobs and
checked subjects keep their own expression identities. Reference descriptors
retain only their own geometry. The former duplicate source-type prewalk is
removed. `controls/composed_extents/` preserves the same total geometry and
oracle using two independent extent sites; its reject twin changes only the
provider results. Both are pinned beside the nine-place fixtures.

Every pending constant declaration is a formation job, including unused private
constants. A demanded crashing initializer and a selected crashing match arm
reject; an unselected arm remains unevaluated. Those controls use the existing
`tests/omega/{pass,fail}/constants/target_layout_derived_*` fixtures. Inlined aliases retain unfinished named canonical inputs;
completed initializer history stays outside new work. Declaration-only jobs do
not acquire native machine owners or storage authority.
Their full query assignment remains on the canonical declaration value, so an
erased private evaluator does not become a missing native call target.

Private specialization asks the same layout owner for newly bound query inputs
before checking freezes its judgments. The invocation owns its target inputs;
preparation reuse includes both those inputs and their exact work selection.
The generic thin-reference/slice-reference and unused-recipe regressions are
pinned under `tests/omega/pass/layouts/runtime_{generic_reference_layout_queries,unused_layout_recipe}_exit`.
The portable receiving operation also retains conventional sum/mixed geometry;
`runtime_case_layout_queries_exit` checks both without granting case access.


The specification and consuming layers are covered as follows:

- **Establishment:** selected validated geometry and admitted initializer
  arithmetic establish the bound; all nine reject twins reach the consuming
  exact subtraction.
- **Identity and lifetime:** exact declaration/selection identities survive
  aliases and copying. Source-backed receiving controls reject a different
  target, missing/repeated plan inputs, a copied digest and an equal-valued
  `Same` provider substituted for `Small`.
- **Composition:** `controls/` covers alignment composition, ordinary/generic
  helpers, a forward helper alias, public aliases and an authored `size_of`
  declaration. The helper underflow twins reject at `main`'s `slack` operation;
  their exact negative bound is pinned explicitly.
- **Soundness boundaries:** an overflowing intermediate rejects even when later
  subtraction would cancel it. An unused carrier mismatch rejects. A skipped
  pure crashing call remains unexecuted; its demanded twin rejects for the
  unhandled Trap route. Qualified destinations refuse their unsupported late
  admission path. A cyclic target-constant graph rejects without provisional
  values. Same-product native `code_layout` remains forbidden. Existing
  declared-operator and unselected-provider controls remain rejecting.
- **Psi 05–07:** the same authored programs reach canonical Terminal Psi and
  execute their result/trap branch. The SDK test independently roundtrips
  canonical bytes and checks historical closure admission.
- **Omega 00–02:** historical plans retain exact provider and target provenance;
  actual checked query subjects use the selected layout authority. Ordinary
  scalar call/state placement carries the transported value.
- **Omega 03–06:** native instruction selection, register homes and frame
  realization execute the exact subtraction and result branch on Linux x86-64.
- **Omega 07–09 and emission:** the emitted ELF executes with exit 0 and agrees
  with Terminal. Retained run evidence includes the actual image, canonical
  Terminal artifact and engine streams; no compile-only result is counted.

Canonical authority is [semantic evaluation](../../../../wiki/spec/language/evaluation.md#target-dependent-applications),
[constant evaluation](../../../../wiki/spec/language/constants.md#evaluation),
[layout queries](../../../../wiki/spec/layouts/plans.md#size-and-alignment-queries)
and [Terminal compatibility](../../../../wiki/spec/terminal-psi/product.md).

The broader capability remains open. Late aggregate/qualified constants,
provider-free observation receipts, arithmetic layout predicates in ordinary
call preconditions, and package generic application review still need connected
implementation. A separate exploratory `requires value == 5/9` probe rejects
before closure; this row establishes scalar value/range transport, not arbitrary
predicate normalization. No language rule or authority gate is relaxed.

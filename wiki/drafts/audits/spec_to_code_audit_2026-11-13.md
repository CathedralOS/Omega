# Spec-to-code audit, 2026-11-13

Read-only audit of duplicated implementation and unnecessary work, measured
on `main` at `a93022df945edf9147d3e75332c7ee944594029a`. The product
specification is `wiki/spec/`. `AGENTS.md` and `omega-rust/pipeline.md` are
requirement sources for architecture: one `omega` command tree, subsystem-owned
folders, representation-to-representation stages, and compact arena-backed
storage. This pass does not delete code.

Every count below names the command that produced it. Path buckets classify
whole files. A brace scan that tried to find `#[cfg(test)]` regions inside
implementation files is reported separately and is not part of any total.

## 1. Case study: deletion of the old checker

Pull request [#2823](https://github.com/CathedralOS/Omega/pull/2823)
(`compiler: delete the old checker and its checking connections`) merged on
2026-11-13 as squash commit `cea9cdcde460532df72d055c27b6aa49d905c3ed`, whose
only parent is `0a2a19c05509855233747e06623ea535e9dad2fb`. GitHub records
`additions: 5676`, `deletions: 861818`, `changed_files: 568`. The same figures
come from `git diff --numstat 0a2a19c05509855233747e06623ea535e9dad2fb cea9cdcde460532df72d055c27b6aa49d905c3ed`
(568 files, 5,676 insertions, 861,818 deletions, net −856,142). The pull
request text's "about 861,000 lines removed" is that deletion count.

### Why the removed checker was unnecessary

The specification puts the checked meaning of a program on Terminal Psi, not
on a side tree of source facts. [Terminal fact validity](wiki/spec/terminal-psi/fact_validity.md)
decides when a fact may justify a later obligation in Terminal Psi: premises
dominate their uses, joins introduce merge tokens, and cycles need checked
invariants. The [Psi product](wiki/spec/terminal-psi/product.md#semantic-boundary)
says execution, proof questions, evidence, diagnostics, work, and lowering
refer to the same stable values, places, operations, and edges, and that Psi's
meaning requires no typed-tree arena. [`omega-rust/pipeline.md`](omega-rust/pipeline.md)
(Verification placement, and the crate-placement rule) says the default
compile trusts each stage's own output, and that `lowered-discharge` is the
compilation's only program verdict. Structural elaboration builds the module;
the lowered-form checker reconstructs obligations on that module.

`AGENTS.md` records the owner's ruling of 2026-11-12: the lowered-form
checker's verdict is the only verdict of a compilation. Psi 05
(`typed-trees-to-checked-trees`), `validation`, Psi 06
(`checked-trees-to-lowered-psi`), `checked-trees`, `facts`, and
`representation-{planning,selections}` were frozen for deletion. The design
record [Check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md)
says why that second checker could not be kept as a fallback: it proved facts
on source trees and then handed them to a second lowering, and none of the
obligations it accepted was backed by evidence the proof kernel re-checked.
The owner identifies that removal with the remembered "about 790k lines." The
measured deletion is the figure of record.

What #2823 deleted, by package name in the deleted `Cargo.toml` files:

| Package | Role before the deletion |
| --- | --- |
| `typed-trees-to-checked-trees` | Psi 05. `checking` ordered validation, checked facts, execution plans, and publication of a checked program. |
| `validation` | Source-semantic judgments (`program_validation` and the contract, borrow, domain, and arithmetic engines). Its own README said it did not construct the checked program; Psi 05 did. |
| `checked-trees` and `facts` | The representation Psi 05 published and Psi 06 consumed. |
| `representation-planning` and `representation-selections` | Planning and selection sidecars under `validation/`. |
| `lowered-psi`, `lowered-psi-to-lowered-psi`, `lowered-psi-to-terminal-psi` | Psi 07–08 and their representation. After Psi 06 was gone, nothing produced `LoweredPsi`. |

Psi 06 itself was not in this diff. #2796
(`b31a2a44c5ff659ce9f50dddba74bb055c5a9934`) had already deleted it. At the
parent of #2823, `lowered-psi`'s crate docs said the representation was
produced by nothing yet, because checked-tree lowering had been deleted, and
was only transformed by Psi 07. Terminal publication (Psi 08) still had a
Cargo edge from `terminal-production`, but it had no producer of its input.
That is an orphaned pipeline stage, not a second required product.

The other large deletions are connection points in crates that survived:
build-time evaluation (layouts, const evaluation, machine execution), typed
preparation (monomorphization, body cloning, selection), `proof` (the
`checker` and mathematical-declaration elaboration that emitted checked-tree
certificates), package review capture, and provider planning. #2823's text
says those owners no longer depend on the retired checker, and that missing
judgments explicitly refuse rather than manufacturing a checked tree or an
empty proof. Much of that code was the only implementation of a language
capability, coupled to checked-tree types. It was unnecessary as a route
around the new verdict. The capabilities themselves stay required until
`lowered-discharge` covers them. Ordinary source and Build compilation were
left refusing where replacement preparation is unavailable. That refusal is
the intended state, not a leftover shim to delete again.

### Measured split of the 861,818 deleted lines

Classification is by path, applied to every numstat row. Generated,
vendored, and build-artifact rules matched no file. A scan of the first 25
lines of all 371 files with status `D` (`git diff --name-status` on the same
range) found none of the markers `do not edit`, `@generated`,
`auto-generated`, `autogenerated`, `this file is generated`, or
`generated by`.

| Bucket | Deleted lines | Files | Rule |
| --- | ---: | ---: | --- |
| Authored implementation | 856,448 | 502 | Remaining `.rs` files. |
| Documentation | 2,883 | 24 | `.md` and `.txt`. Not one of the five requested buckets; kept separate so it is not called code. |
| Test fixtures and test files | 1,976 | 16 | Path contains `/tests/`, `/fixtures/`, `/fixture/`, `/expected/`, `/golden/`, `/corpus/`, or `/testdata/`, or the file name is `tests.rs`, `*_tests.rs`, `*_test.rs`, or `test_*.rs`. |
| Crate manifests | 283 | 25 | `Cargo.toml`. Hand-written package definitions, not generated code. |
| Lockfile | 228 | 1 | `Cargo.lock`. |
| Generated code | 0 | 0 | Filename or header marker, as above. |
| Vendored dependencies | 0 | 0 | `vendor/`, `third_party/`. |
| Build artifacts | 0 | 0 | `/target/` or object-file suffixes. |
| **Total** | **861,818** | **568** | Matches `git diff --numstat` and the GitHub deletion count. |

The 1,976 test-file lines are the files listed by that rule, the largest
being `compiler/tests/package_review.rs` (594) and
`provider-planning/.../builder/tests.rs` (438). `src/checked/test_roots.rs`
contributes 11 lines only because its name starts with `test_`.

### Directory check of the 856,448 authored Rust lines

A second pass asked whether any of that Rust was generated, a checked-in
expected output, or a data table. It did not move any lines out of the
authored bucket.

The command read `/tmp/2823.numstat` (from
`git diff --numstat 0a2a19c05509855233747e06623ea535e9dad2fb cea9cdcde460532df72d055c27b6aa49d905c3ed`),
kept `.rs` paths that the test-file rule does not claim, and summed deletions
by the directory that contains `src` (or the parent directory when the file
has no `src`). The sum is 856,448. Every directory with a non-zero sum:

| Deleted lines | Directory |
| ---: | --- |
| 342,563 | `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees` |
| 336,630 | `omega-rust/psi/compilation/validation` |
| 36,359 | `omega-rust/psi/compilation/build-time-evaluation` |
| 30,841 | `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees` |
| 27,348 | `omega-rust/psi/compilation/representations/checked-trees` |
| 25,250 | `omega-rust/omega/packages/review-evidence` |
| 24,192 | `omega-rust/psi/compilation/proof` |
| 13,997 | `omega-rust/omega/compilation/realization/provider-planning` |
| 6,210 | `omega-rust/psi/compilation/representations/checked-trees/facts` |
| 2,001 | `omega-rust/omega/compilation/realization/selected-dispatch` |
| 1,910 | `omega-rust/omega/compilation/compiler` |
| 1,836 | `omega-rust/psi/compilation/pipeline/07_lowered-psi-to-lowered-psi` |
| 1,776 | `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees` |
| 1,529 | `omega-rust/psi/compilation/psi-compiler` |
| 1,528 | `omega-rust/omega/build-system/build-settings` |
| 757 | `omega-rust/psi/compilation/validation/representation-planning` |
| 597 | `omega-rust/psi/compilation/pipeline/08_lowered-psi-to-terminal-psi` |
| 266 | `omega-rust/psi/compilation/representations/lowered-psi` |
| 261 | `omega-rust/psi/compilation/validation/representation-planning/selections` |
| 154 | `samples/cli/scientific/spectrometer_attribution` |
| 120 | `omega-rust/psi/compilation/psi-compiler/terminal-production` |
| 102 | `omega-rust/omega/compilation/products` |
| 84 | `omega-rust/omega/packages/package-compilation` |
| 54 | `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations` |
| 21 | `omega-rust/psi/compilation/pipeline/structure-elaboration` |
| 19 | `omega-rust/psi/compilation/representations/typed-trees` |
| 14 | `omega-rust/psi/compilation/pipeline/lowered-discharge` |
| 14 | `tools/xtask` |
| 7 | `omega-rust/omega/compilation/native-compiler` |
| 3 | `omega-rust/omega/compilation/compiler/examples/build_charge` |
| 3 | `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations` |
| 2 | `omega-rust/psi/compilation/representations/terminal-psi/codec` |

`git diff --name-status` on the same range has no `build.rs`, no
`*.generated.rs`, and no expected-output or golden file outside the test-file
bucket. `git ls-tree -r --name-only 0a2a19c05509` under the wholly removed
crate trees matches no `build.rs`, `generated`, `expected`, `fixture`, or
`golden` name.

For each of the 348 deleted files in this bucket, `git cat-file --batch` at
`0a2a19c05509` searched the whole blob for `do not edit`, `@generated`,
`auto-generated`, `autogenerated`, `this file is generated`, and
`generated by`. The only hit is
`05_typed-trees-to-checked-trees/src/values_scalar.rs:9898`, the prose
"each generated byte". The substring `generated by` is the start of
`generated byte`. It is not a generated-file marker. The same blobs were
scored for table-shaped lines (a line opening with a quote, digit, or
parenthesis and containing at least three commas or four quotes). The highest
share was 25 of 6,210 lines (0.4%) under `checked-trees/facts`. No directory
was reclassified.

`include_str!` occurs 7 times in the deleted
`immutable_checking_input.rs` (`git grep -c include_str!` at the parent). It
reads neighboring source files for a guard. It is not `build.rs` output.
Generated code stays 0. Vendored dependencies stay 0. Build artifacts stay 0.

Those path rules miss tests that live inside implementation files. A separate
brace scan of the 371 deleted files marked about 20,034 lines as inside a
`#[cfg(test)]` item (14,780 in the wholly removed crates, 5,254 in deleted
files of surviving crates). The scan is approximate: it counts braces outside
a naive string and comment reading. It is not subtracted from the 856,448.
Embedded tests are a small fraction of the deletion either way.

Of the 861,818, `git diff --numstat` assigns 718,503 lines to files under the
wholly removed crate trees and 143,315 to edits elsewhere
(`718,503 + 143,315 = 861,818`):

| Group | Deleted lines |
| --- | ---: |
| `05_typed-trees-to-checked-trees/src` | 342,563 |
| `validation/src` | 336,630 |
| `checked-trees`, including `facts/` | 33,607 |
| Psi 07, Psi 08, and `lowered-psi` | 2,797 |
| Other files inside those crate trees (READMEs, manifests, notes) | 2,906 |
| Edits in surviving trees | 143,315 |

The largest surviving-tree deletions are `build-time-evaluation` (36,749),
`04_typed-trees-to-typed-trees` (30,978), `review-evidence` (25,695), `proof`
(24,201), and `provider-planning` (14,633). Those five sums are numstat
deletions whose crate directory is that package, including files the package
still has.

### Reconciling 861,818 with "about 790k"

The remembered 790k is this deletion. The owner confirmed that the 861k
figure and the remembered 790k are the same removal. The merge diff measures
861,818 deleted lines, which is 71,818 above 790,000. "About 861,000" in the
pull request text is the same count rounded the other way. Use 861,818.

`TASKS.md` still phrases the ruling as "Psi 05, validation and Psi 06, about
790k lines." That sentence is the memory of this removal, not a second
measurement that should be added to 861,818 or subtracted from it. Psi 06's
own deletion landed separately in #2796 (129,851 deletions, of which 124,010
are `*.rs` under `06_checked-trees-to-lowered-psi/src`, from
`git diff --numstat b31a2a44c5ff659ce9f50dddba74bb055c5a9934^ b31a2a44c5ff659ce9f50dddba74bb055c5a9934`).
It is part of how the old checker came out. It is not the gap between 790k
and 861k.

### Patterns to look for in what remains

Similar-looking code is not unnecessary. These are the shapes this deletion
actually had. A hit still needs a spec passage, a caller, and a reason the
behavior is not a distinct requirement.

1. **A second verdict behind a live connection.** Psi 05 and `validation`
   stayed reachable from Build evaluation, review, and provider planning
   after `lowered-discharge` was the only accepted verdict. Look for an
   entry that can still succeed by running, or inventing the result of, a
   judgment the spec assigns to one stage. A function that refuses a missing
   judgment is the end state #2823 left behind.

2. **Storage whose producer or consumer is gone.** `checked-trees`, `facts`,
   `representation-planning`, `representation-selections`, and `lowered-psi`
   existed to carry one retired chain. Look for a table, receipt, or sidecar
   that no current stage constructs or reads. A comment or a forbidden-edge
   string that names a deleted path is not itself a second implementation.

3. **Two lowerings of one program.** Psi 05's `execution*.rs` files built
   Terminal-facing plans, and Psi 06 lowered again. The product contract
   wants one set of values, places, operations, and edges. Look for a later
   stage that rebuilds a relation the producer already retained.

4. **A per-shape adapter beside one general operation.** The design record
   found validation's arithmetic engine surrounded by recognizers for
   incidental source shapes. Look for a second procedure that exists only
   to recognize an arrangement the general operation already covers. Keep a
   branch the spec states as its own rule.

5. **Default-path revalidation of a product this compile just produced.**
   [Verification placement](omega-rust/pipeline.md#verification-placement)
   keeps independent validators at trust boundaries, foreign packages, and
   explicit validator requests. It does not ask the default path to carry a
   second implementation of the same rules. A kernel that re-decides a
   certificate is a stated requirement (`proof/admission`), not this pattern.

6. **A compatibility shim that still performs the old algorithm.** Old
   tree-check receipt encodings were deleted by making nonempty unsupported
   requests refuse. Look for a shim that still builds a checked tree, an
   empty proof, or a success flag. Do not treat an explicit refusal as
   unused code.

7. **Names of the deleted crates left in live controls and owner docs.**
   `rg` over `*.rs` and `*.toml`, excluding `wiki/`, `samples/`, and
   `.agents/`, finds 14 matching lines in six files: `tools/xtask/src/boundary.rs`
   (5), `tools/xtask/src/stage_coverage.rs` (4), `tools/xtask/src/rules.rs`
   (1), and comments in `typed-trees`, `terminal-psi`, and `terminal-psi/codec`.
   In `boundary.rs` the names are mostly forbidden dependency edges
   (`SOURCE_STAGES`, `TERMINAL_STAGES`, `REPRESENTATION_PLANNING`,
   `checked-trees`). A forbidden path that no longer exists cannot match a
   Cargo edge; whether the entry still guards a resurrection is a tools
   question, not a reason to delete the firewall. Stale prose that points
   at deleted files also remains in `omega-rust/psi/compilation/README.md`,
   `omega-rust/optimization.md`, `omega-rust/omega/compilation/compiler/README.md`,
   `omega-rust/omega/compilation/representations/README.md`,
   `omega-rust/omega/compilation/realization/trust-model/README.md`,
   `omega-rust/omega/build-system/build-settings/README.md`, and
   `omega-rust/psi/foundation/language-core/{atomic_operations,inline_assembly}.md`.
   Documentation drift is real and small. It is not counted as an
   implementation finding until the owning pass shows the text still changes
   a decision.

### Partition for the subsystem pass

Each path below has one owner. Investigators add evidence to an existing
finding rather than opening a second row for the same mechanism.

| Id prefix | Owner | Paths |
| --- | --- | --- |
| FE | Psi frontend | `pipeline/00`–`04`, representations `tokens`, `syntax-trees`, `symbol-resolved-trees`, `typed-trees`, and `psi-compiler` except `terminal-production`. |
| LF | Lowered-form checker and Terminal product | `structure-elaboration`, `lowered-discharge`, `proof` (including `admission`), `representations/terminal-psi`, `terminal-installation`, `psi-compiler/terminal-production`, `psi/interpreter`. |
| FD | Psi foundation and build-time evaluation | `psi/foundation`, `psi/compilation/build-time-evaluation`. |
| OC | Omega coordination, build, packages, CLI | `omega/compilation/compiler`, `native-compiler`, `products`, `omega/build-system`, `omega/packages`, `omega/application`, `omega/foundation`. |
| OR | Omega realization and native pipeline | `omega/compilation/realization`, `backend`, `pipeline`, `representations`, `omega/interpreter`. |
| TL | Tools, tests, library, bootstrap | `tools/`, `tests/`, `source/`, `bootstrap/`. Samples only where they embed a second implementation or a harness, not as a program corpus. |

Bootstrap competing variants are authorized by `bootstrap/MINIMIZATION.md`.
They are not duplicates of the product compiler merely because both exist.

## 2. Subsystem findings

The pass covered the six partitions in section 1 at
`a93022df945edf9147d3e75332c7ee944594029a`. It did not find a second live
implementation of a judgment the specification assigns to one stage. The
large crates that look like a second checker were checked and kept:

- `optimize_abstract_operations` in
  `01_abstract-operations-to-abstract-operations` is called from
  `native-compiler/src/pipeline.rs` and runs the Psi optimization catalog on
  abstract operations. That is the surviving executor of those passes after
  Psi 07 was deleted, not a second copy of it.
- `proof/admission` re-decides certificates, which
  [verification placement](omega-rust/pipeline.md#verification-placement)
  requires. `proof/src/lib.rs` is a syntax surface and says so.
- No `Cargo.toml` still depends on `checked-trees`, `validation`,
  `typed-trees-to-checked-trees`, `lowered-psi`, or `representation-planning`
  (`rg` over `Cargo.toml`).
- Explicit refusals in `psi-compiler` and `lowered-discharge` are the end
  state #2823 left. They are not shims that still succeed.

The register is
`wiki/drafts/audits/spec_to_code_audit_2026-11-13.tsv`. Line estimates use
`wc -l` or a counted span, and the command is named on each finding.

### TL-1. `stage-coverage` still measures the deleted checker

Category: likely unnecessary. Confidence: high. Owner: tools.
Authored lines: 495 in `tools/xtask/src/stage_coverage.rs` (`wc -l`), plus
the `mod`, match arm, and help lines in `tools/xtask/src/main.rs` (lines 9,
33, 57, 82).

`tools/xtask/src/stage_coverage.rs` defines `STAGES` as Psi 05, `validation`,
and Psi 06, and `main.rs` dispatches `stage-coverage` to `stage_coverage::run`.
Those three directories are absent (`test -d` on each path). The specification
is silent on this command; `tools/testing.md` ("Stage line coverage") says it
measures the checker and that nothing in CI calls it. `rg` of `stage-coverage`
over `*.yml`, `*.yaml`, `*.sh`, `*.rs`, and `*.md` finds the tool, that
section, the design record, and this audit. `git log -1` on the file is
`2935b3b89f3` (`#2817`), which kept the command when the corpus runner went
away. `OWNER_CODE_CONCERNS.md` already notes the empty Psi 06 column.

Simplification: delete the command, the module, and the testing.md section.
Validation: `mbx xtask check-rules --base origin/main --title "tools: drop stage-coverage of the deleted checker"`,
then `rg -n 'stage-coverage|stage_coverage' --glob '!wiki/drafts/**'` and
confirm the only remaining hits are historical design notes if those are left
on purpose.

### TL-2. The interior-state lint still names two missing roots

Category: likely unnecessary. Confidence: medium. Owner: tools.
Authored lines: the two dead path strings in `CHECKING_STATE_ROOTS`
(`tools/xtask/src/rules.rs`, lines 134–138). The function stays.

`checking_interior_state_violations` walks those roots for `thread_local!`
and `RefCell`. `collect_files_with_ext` returns immediately when
`read_dir` fails (lines 108–110), so the Psi 05 and `validation` entries
add no files. The stage 04 entry is a real directory and is still scanned.
The specification is silent; the comment cites the deleted test
`checking_does_not_reopen_typed_preparation`. The scan runs on every
`check-rules` pass (`rules.rs` line 1537).

Simplification: drop the two missing roots. Keep the stage 04 root. If the
rule is still about the checker, point it at `lowered-discharge` in a
separate change after reading what mutable state that crate is allowed to
hold. Validation: `mbx xtask check-rules --base origin/main --title "tools: drop deleted checker roots from the interior-state lint"`.

### TL-3. Forbidden-edge strings still name the deleted crates

Category: likely unnecessary, then dropped in section 3. Confidence before
review: low. Owner: tools.

`tools/xtask/src/boundary.rs` puts
`05_typed-trees-to-checked-trees` in `SOURCE_STAGES` (line 59),
`07_lowered-psi-to-lowered-psi` and `08_lowered-psi-to-terminal-psi` in
`TERMINAL_STAGES` (lines 67–70), `representation-planning` in
`REPRESENTATION_PLANNING` (line 52), and `checked-trees` on the Build
coordinator's forbidden list (line 192) and in `SOURCE_SIDE_PSI` (line 83).
Those directories are absent. A Cargo path dependency cannot resolve to a
missing directory, so the edges do not match a graph that compiles. The
specification is silent; `AGENTS.md` and `pipeline.md` require the firewall.
Section 3 drops the deletion: the strings are the guard that fires if the
paths come back.

### OC-1. Checked-tree pruning is still a named optimization, and its only helper has no caller

Category: spec question. Confidence: medium. Owner: Omega coordination.
Authored lines of the dead helper: 10 (`evaluated_build.rs` lines 161–170).
The wire variant stays until the specification changes.

`BuildConfig::checked_tree_product_roots` returns program roots when
`Optimization::CheckedTreeProductPruning` is selected. `rg -n checked_tree_product_roots`
finds that definition and no call. The variant is tag 22, phase
`CheckedTrees`, in `optimization_core.rs` (`OMGOPT\0\0` version 15).
[Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
still say: "Checked-tree pruning runs only after every declaration of the
check's validation subjects and their required dependency closure has been
parsed, resolved, typed, and checked." The same page projects a phase-local
set "for checked trees." `rules.md` lists the selection as required for that
phase. `validate_optimization_rollback` in `compile_request.rs` still treats
`CheckedTrees` as a phase a terminal-artifact rollback may name.

The helper can disappear without changing the codec. Removing the variant
changes selection identity and disagrees with the specification until that
page is amended. Tests: `optimization_core/tests/cases.rs` still projects the
phase.

Validation for deleting only the helper: `mbx test -p compilation-products --lib`
and `mbx clippy -p compilation-products --all-targets -- -D warnings`.
Do not land that deletion in the same change as a codec edit.

### OC-2. Two timing labels for Psi 05 have no reader

Category: likely unnecessary. Confidence: high. Owner: Omega foundation
(`artifacts`). Authored lines: 4, the two unused constants.

`TYPED_TREES_TO_CHECKED_TREES` and `EXECUTION_SETTLEMENT` in
`omega-rust/omega/foundation/artifacts/src/compile_timings/mod.rs` (lines
44–45 and 56–57) are unreferenced. `rg -n 'TYPED_TREES_TO_CHECKED_TREES|EXECUTION_SETTLEMENT'`
finds only those definitions. The specification is silent on timing labels.
`BUILD_AND_CHECKED_CONTINUATION` is live: `compiler/src/checked/checking.rs`
line 187 and `build_continuation.rs` line 266 pass it to the timer, and its
output text is still "CheckedTrees (contains Psi 00-05)". That label is a
rename, not a deletion. `git` history was not needed beyond the absence of
callers.

Validation: `mbx test -p artifacts --lib` and
`mbx clippy -p artifacts --all-targets -- -D warnings`.

### FD-1. The checked-tree measurement identity is still a current schema

Category: spec question. Confidence: medium. Owner: Psi foundation.
Authored lines are not estimated as a deletion. The codec arm has to stay
if old records exist, and that was not proved either way.

`EvaluationMeasurementIdentity::CheckedTree` is schema 7.
`is_current` returns true for it (`evaluation-accounting/src/invocation.rs`).
`BuildEvaluationSponsor::new` stamps that identity (`lib.rs` line 169).
`encode_build_evaluation_usage` writes it as tag 0
(`build_record_codec.rs`). Production sponsors that this pass found use
`TerminalPsi`: `build-settings/src/activation.rs` `measurement` and
`build-time-evaluation/src/evaluation_accounting.rs`
`pure_terminal_invocation_account`. `rg -n 'BuildEvaluationSponsor::new\('`
finds tests and one `#[cfg(test)]` helper in
`filesystem_scope/runtime.rs` (the `#[cfg(test)]` is at line 799). The
specification pages opened for work budgets,
[scoped execution](wiki/spec/build/scoped_execution.md) and
[embedding](wiki/spec/build/embedding.md), do not name this enum. They are
silent on schema 7.

`charge_work_unit`, `reserve_live_cell`, and the sponsor-level
`charge_result_custody` call `require_checked_tree` and refuse a
`TerminalPsi` sponsor. `rg` of those methods outside
`evaluation-accounting/tests` finds no production caller of the sponsor
methods. `accounts.rs` asserts that a terminal sponsor's `charge_work_unit`
returns an error. That refusal is the fence, not a leftover success path.
The live custody charge is `EvaluationInvocationAccount::charge_result_custody`,
which does not consult the old identity.

Simplification, if the owner wants it: stop constructing new current
sponsors with `CheckedTree`, and decide whether tag 0 remains decodable.
Do not delete the refusal or the decode arm in the same unreviewed step.

Validation after any later code change: `mbx test -p evaluation-accounting`
and `mbx test -p package-compilation --lib build_record_codec`, plus clippy
on both packages. The package names should be confirmed from their
`Cargo.toml` before the change; this audit did not run the tests.

### DOCS-1. Owner comments still point at deleted stages

Category: likely unnecessary. Confidence: high that the text is stale, low
that deleting it changes behavior. Owner: whichever crate the comment sits in.
`rg -c` over `*.rs` and `*.toml`, excluding `wiki/`, `samples/`, and
`.agents/`, found 14 lines naming the deleted stage paths, in
`typed-trees/src/lib.rs`, `terminal-psi/src/lib.rs`,
`terminal-psi/codec/src/lib.rs`, `tools/xtask/src/boundary.rs`,
`stage_coverage.rs`, and `rules.rs`. The last three are TL-1, TL-2, and the
dropped TL-3. The crate docs do not change a decision. Markdown with the
same stale links includes `omega-rust/psi/compilation/README.md`,
`omega-rust/optimization.md`, `compiler/README.md`,
`representations/README.md`, `trust-model/README.md`,
`build-settings/README.md`, and
`language-core/{atomic_operations,inline_assembly}.md`.

Simplification: correct the sentences while touching the file for another
reason. No standalone documentation pull request.

## 3. Independent review

This pass re-read the callers of each finding above and tried to break the
simplification. It did not re-open the directory census.

| Id | Verdict |
| --- | --- |
| TL-1 | Stands. `tools/testing.md` says CI does not call the command, and the three directories are gone. A zero column is not a regression guard worth a full instrumented compiler build. |
| TL-2 | Stands, narrowed. The function is live because stage 04 is real. Only the two missing roots are dead. Leaving them makes a failed `read_dir` look like a clean scan. Retarget; do not delete the lint. |
| TL-3 | Dropped. `boundary.rs` lists `checked-trees`, `representation-planning`, Psi 05, and Psi 07/08 as forbidden dependency edges, including the comment that retired Lowered Psi stages do not return through the Psi facade. The strings do not match a Cargo edge today. They are the guard that fires if those paths are recreated and depended on. Deleting them weakens the firewall the deletion just wrote down. |
| OC-1 | Stands as a spec question. The 10-line helper has no caller and can go. The tag-22 variant and the specification sentence cannot go in that same change: `OMGOPT` version 15 and `validate_optimization_rollback` still treat the phase as nameable. |
| OC-2 | Stands for the two unread constants. `BUILD_AND_CHECKED_CONTINUATION` is passed from two live call sites, so the follow-up renames its text and does not delete the constant. |
| FD-1 | Stands as a spec question. Tag 0 may already be stored in build records; this pass did not prove that no record uses it. The sponsor charge methods that refuse a `TerminalPsi` account are the fence `accounts.rs` tests. Do not delete that fence. |
| DOCS-1 | Stands, and is not its own change. The comments do not affect compilation. |

No finding was upgraded to confirmed duplication. Similar names
(`CheckedCompilation`, `compiler/src/checked/`, `PsiOptimization`) are the
current product coordinator and the abstract-operation optimizer, which have
live callers.

## 4. Ranking and follow-ups

Rank is maintenance saved, then confidence, then how likely the change is to
break a real reader. Confirmed duplication: 0. Likely unnecessary: 4
(TL-1, TL-2, OC-2, DOCS-1). Unresolved spec questions: 2 (OC-1, FD-1).
Dropped by review: 1 (TL-3).

| Rank | Id | Why this order |
| --- | --- | --- |
| 1 | TL-1 | Removes a command that builds an instrumented compiler to count lines in directories that are gone. |
| 2 | OC-1 | The helper is dead, but the selection is still in the versioned vocabulary and in the specification. Ask before touching the tag. |
| 3 | FD-1 | Schema 7 is still "current." Decoding and the refusal fence stay until the question is answered. |
| 4 | TL-2 | The lint silently skips two roots on every rules run. The edit is small and the stage 04 scan stays. |
| 5 | OC-2 | Four lines of unread constants, plus a stale label on a live timer. |
| 6 | DOCS-1 | Stale sentences. No behavior. |

Authored Rust that these follow-ups can remove, from the commands above:
495 (`stage_coverage.rs`) plus the four `main.rs` lines, about 10 for
`checked_tree_product_roots` if that helper is removed without the wire tag,
4 for the unused timing constants, and 2 path strings. That is about 515
authored lines. Generated, vendored, and fixture reduction from these
follow-ups: 0. Documentation reduction is the stage-coverage section in
`tools/testing.md` (lines 440–497, 58 lines) and the comment fixes in DOCS-1,
which are rewrites rather than a measured deletion.

### Follow-up 1. Remove `stage-coverage`

`tools: drop stage-coverage of the deleted checker`

Checks:

- `mbx xtask check-rules --base origin/main --title "tools: drop stage-coverage of the deleted checker"`
- `mbx xtask fmt --check`
- `rg -n 'stage-coverage|stage_coverage' tools`

### Follow-up 2. Point the interior-state lint at directories that exist

`tools: scan stage 04, not the deleted checker, for hidden state`

Checks:

- `mbx xtask check-rules --base origin/main --title "tools: scan stage 04, not the deleted checker, for hidden state"`
- `mbx xtask fmt --check`

### Follow-up 3. Drop unread timing constants and correct the live label

`tooling: drop unread Psi 05 timing labels`

Checks:

- `mbx test -p artifacts --lib`
- `mbx clippy -p artifacts --all-targets -- -D warnings`
- `mbx xtask fmt --check`
- `mbx xtask check-rules --base origin/main --title "tooling: drop unread Psi 05 timing labels"`

### Follow-up 4. Ask, then delete only the uncalled pruning helper

Do not change `Optimization::CheckedTreeProductPruning` or schema 7 in this
change. Record the two spec questions (checked-tree pruning after the checker
is gone, and whether schema 7 is still current) in `OWNER_QUESTIONS.md`.
The helper deletion, if it lands alone:

- `mbx test -p compilation-products --lib`
- `mbx clippy -p compilation-products --all-targets -- -D warnings`
- `mbx xtask check-rules --base origin/main --title "omega: drop the uncalled checked-tree pruning helper"`

### What this pass could not verify

- Whether any on-disk build record still encodes measurement tag 0. The
  codec can write and read it; this pass did not search artifact stores.
- Runtime of `stage-coverage`. The directories are absent, so a run would
  report zeros, and this pass did not build the instrumented compiler.
- `mbx test` and clippy for the packages above. The audit is read-only and
  did not compile.
- A line-by-line reading of the largest live crates (`typed-trees` 71,141
  lines, instruction selection 68,336, the resolver 66,064, the interpreter
  56,772, terminal semantics 53,446, proof admission 48,667; all `wc -l` of
  `src` excluding a `tests` path). The pass traced the deletion's patterns
  and the live callers of the old names. It did not claim those crates are
  free of every other duplication.

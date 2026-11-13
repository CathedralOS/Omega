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

### The decision that let it grow

The deleted tree is one structural choice, visible in the crate boundaries
and in how the files inside Psi 05 were named.

The compiler kept a second program representation, `checked-trees`, between
typed trees and Terminal Psi, and it kept the verdict on that representation.
Psi 05 both judged it and lowered it. At the parent of #2823, the Rust under
`05_typed-trees-to-checked-trees/src` splits by filename prefix
(`git cat-file --batch` over `git ls-tree -r --name-only 0a2a19c05509` of
that directory; the four groups sum to the directory's 342,563 lines):

| Prefix group | Lines | Filenames |
| --- | ---: | --- |
| `checks` | 147,327 | `checks*.rs`, the judgments |
| `facts_and_flow` | 98,839 | `published*.rs`, `borrow*.rs`, `flow*.rs`, facts stored beside the tree |
| `execution` | 73,221 | `execution*.rs`, a lowering inside the checking stage |
| other | 23,176 | the stage driver and the remaining files |

`validation` was not a representation-to-representation stage. Its README
said it supplied judgments and did not construct the checked program; Psi 05
did. `representation-planning` and `representation-selections` sat under that
sidecar. `facts` was a second store of the same decisions, keyed beside the
tree (`checked.facts.resolved_types` is the path Psi 06's README names).

Psi 06, removed earlier by #2796, was a second lowering of that side
representation. Its README at `ae891752fe8d296bf623b6d35680e1b066c2e715`
(the parent of #2796) says the stage "selects the checked machine, lowers
its source closure" and that "the checker's permission ledger is consumed
as checked: lowering does not re-validate the checker's own fresh output."
Psi 07 and Psi 08 then optimized and published the product of that second
lowering. After Psi 06 was gone, `lowered-psi` had no producer and Psi 07
still transformed it.

Once the types and the verdict lived on `checked-trees`, other owners took
a dependency on that representation. Those connection points are the 143,315
deleted lines outside the wholly removed crate trees (the table below).

Four patterns follow from that tree. Section 3 groups the live findings
under them. A pattern with no live file and symbol under it is not a finding.

1. **A parallel representation for the verdict.** `checked-trees` held a
   second copy of the program so that checking could have its own types.
   The product contract says execution, proof, evidence, diagnostics, work,
   and lowering refer to one set of values, places, operations, and edges
   ([semantic boundary](wiki/spec/terminal-psi/product.md#semantic-boundary)).
2. **A sidecar that is not an `X-to-Y` stage.** `validation` and its
   planning crates judged the program from beside the pipeline. The
   crate-placement rule in [`pipeline.md`](omega-rust/pipeline.md)
   admits shared vocabulary and evidence, and it does not admit a second
   verdict owner.
3. **Connection points in owners that needed the side representation.**
   Build-time evaluation, typed preparation, proof elaboration, package
   review, and provider planning imported checked-tree types. When the
   representation went away, the connections went with it.
4. **A second lowering that trusted the first.** Psi 05's `execution*.rs`
   already built plans from checked facts, and Psi 06 lowered again without
   re-checking. The specified order is one lowering, then selected
   optimization of that same representation, then publication
   ([optimization ordering](wiki/spec/terminal-psi/product.md#optimization-ordering)).

### Why that shape is not the specified compiler

The specification puts the checked meaning of a program on Terminal Psi. [Terminal fact validity](wiki/spec/terminal-psi/fact_validity.md)
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

### Questions the four patterns did not confirm

Two further shapes were searched and did not become findings.

- **A per-shape adapter beside one general operation.** The design record
  describes validation's arithmetic engine surrounded by recognizers for
  incidental source shapes. No live pair of that form was confirmed.
  `proof/admission`'s `verify_obligation` is the kernel `lowered-discharge`
  calls, which is the single proof verdict the design record asks for.
  The closed crash guard is a different decision, and two live walkers
  disagree on it (LF-1).
- **A shim that still builds a checked tree or an empty proof.** The
  surviving checker connection points refuse. `require_source_owner` and
  the carrier rows return `no product route yet` when the judgment or the
  input shape is absent. A different shim, for an old snapshot directory
  name, is OC-4.

Names of the deleted crates in live Rust and Cargo files, outside `wiki/`,
`samples/`, and `.agents/`, are 14 lines in six files (`rg -n` for the crate
path fragments): `tools/xtask/src/boundary.rs` (5),
`tools/xtask/src/stage_coverage.rs` (4), `tools/xtask/src/rules.rs` (1), and
comments in `typed-trees`, `terminal-psi`, and `terminal-psi/codec`. The
`boundary.rs` lines are forbidden dependency edges. They are the guard
against putting the parallel representation back (TL-3, dropped). Stale
prose in owner READMEs is DOCS-1.

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

## 2. The spec's pipeline against the crates that exist

The specification does not list crate names. It lists responsibilities and
an order. [Semantic boundary](wiki/spec/terminal-psi/product.md#semantic-boundary)
gives Psi parsing, resolution, typing, checking, proof construction,
expression lowering, selected target-neutral optimization, and publication,
and gives Omega provider selection, target realization, its own optimization,
ABI lowering, native emission, and execution. [Optimization ordering](wiki/spec/terminal-psi/product.md#optimization-ordering)
fixes the Psi order as lowering, then selected Psi optimization, then
Terminal publication. [Exact selection](wiki/spec/build/optimizations.md#exact-selection)
names the phase sets: checked trees, Psi, abstract operations, target
operations and instruction selection, pre-allocation, allocation recovery,
post-allocation, and resolved layout. An optimization consumes and produces
the same representation. [`pipeline.md`](omega-rust/pipeline.md) is the
architecture source for which directory is which `X-to-Y` stage.

Marks mean: **matching** (the crate is the stage the spec describes),
**duplicated** (a second live crate does the same stage), **extra** (a crate
or a spec phase the required pipeline does not have), **missing** (the spec
names the stage and no crate performs it). Directories were listed with
`ls` of `omega-rust/psi/compilation/pipeline`,
`omega-rust/omega/compilation/pipeline`, and both `representations/` trees
at `a93022df945e`.

| Spec responsibility | Current home | Mark |
| --- | --- | --- |
| Parsing | `00_source-files-to-tokens`, `01_tokens-to-syntax-trees` | matching |
| Resolution | `02_syntax-trees-to-symbol-resolved-trees` | matching |
| Typing | `03_symbol-resolved-trees-to-typed-trees` | matching |
| Typed-to-typed preparation | `04_typed-trees-to-typed-trees` | matching. The spec does not make this a verdict. The crate-placement rule keeps it as an empty-work boundary that still runs specialization and record patterns. Its README still says the passes feed "the checker" (DOCS-1). |
| Expression lowering onto Terminal Psi | `structure-elaboration` | matching. The crate README says it is not wired into the default compile. `compiler/src/checked/checking/lowered_route.rs` `RouteScope::judge` calls `AcceptedTerminalSource::judge`, which elaborates. |
| Checking, the only verdict | `lowered-discharge` | matching. The same `judge` calls `lowered_discharge::judge_compilation_for`. |
| Selected Psi optimization, before publication | no Psi `X-to-X` stage | **missing.** Publication refuses a nonempty Psi selection. See PL-1. |
| Terminal publication | `psi-compiler/terminal-production` | matching. `ProductCodeSource::publish` publishes `accepted()?.produce(...)`. |
| Proof construction | `proof/` surface, `proof/admission` `verify_obligation` | matching, and not a pipeline stage. The kernel is what the verdict calls. |
| Abstract-operations optimization | `01_abstract-operations-to-abstract-operations` | the directory matches an `X-to-X` stage. `optimize_abstract_operations` refuses a nonempty `AbstractOperations` selection and runs the Psi projection instead (PL-1). |
| Target realization | `02_abstract-operations-to-target-operations` | matching as the representation change. A nonempty `TargetOperations` selection is refused (PL-2). |
| Instruction selection | `03_target-operations-to-selected-instructions` | matching. The README describes legalization and selection, not a second copy of stage 02. |
| Pre-allocation and selected-lowering optimization | `04_selected-instructions-to-selected-instructions` | matching. `selected_optimization.rs` runs one of `SelectedLowering` or `PreAllocation` and returns `UnsupportedComposition` when both are selected. |
| Allocation recovery | `05_selected-instructions-to-register-homes` | matching. `register_allocation.rs` projects `AllocationRecovery`. |
| Post-allocation | `06_register-homes-to-post-allocation-machine` | matching. `optimization.rs` uses `PostAllocationMachine`. |
| Encoding, then layout formation | `07_post-allocation-machine-to-selected-form-encoding`, `08_selected-form-encoding-to-resolved-layout` | matching as Omega realization (ABI, emission, layout). They are representation changes, not a second optimizer for an earlier phase. |
| Resolved-layout optimization | `09_resolved-layout-to-resolved-layout` | matching. The phase enum value it executes is `FunctionRelativeLayout`. |
| Checked-tree optimization | no stage | **extra** in [optimizations.md](wiki/spec/build/optimizations.md#phase-and-product-boundaries), which still says checked-tree pruning runs after source has been checked. `AGENTS.md` forbids that representation. The live leftover is OC-1. Do not add the stage back. |
| `checked-trees`, `validation`, Psi 05, Psi 06, `lowered-psi`, Psi 07, Psi 08 | absent | correctly absent. These were the parallel representation, the sidecar, and the second lowering. |

Representation directories match that table. Psi has `tokens`,
`syntax-trees`, `symbol-resolved-trees`, `typed-trees`, and `terminal-psi`.
Omega has `abstract-operations`, `target-operations`,
`selected-instructions`, `register-homes`, `post-allocation-machine`, and
`machine-code`. There is no `checked-trees` and no `lowered-psi`.

No row is **duplicated**. The mismatches are the missing pre-publication
Psi stage, the Psi catalog executing inside the abstract-operations stage,
and the checked-tree phase the spec still names.

## 3. Findings, grouped by the four patterns

The pass covered the six partitions at
`a93022df945edf9147d3e75332c7ee944594029a`. One judgment is implemented
twice and the two copies disagree (LF-1). The proof kernel itself is still
one implementation.

- No `Cargo.toml` still depends on `checked-trees`, `validation`,
  `typed-trees-to-checked-trees`, `lowered-psi`, or `representation-planning`
  (`rg` over `Cargo.toml`).
- `proof/admission` re-decides certificates, which
  [verification placement](omega-rust/pipeline.md#verification-placement)
  requires. Package `proof` is a separate syntax-report crate with no
  production caller (LF-4).
- Explicit refusals in `psi-compiler` and `lowered-discharge` are the end
  state #2823 left. They are not shims that still succeed.

| Pattern from section 1 | Findings |
| --- | --- |
| Parallel representation for the verdict | OC-1, FD-1 |
| Sidecar that is not an `X-to-Y` stage | TL-1, TL-2, OC-2, DOCS-1 |
| Connection points | TL-3, dropped: the firewall strings are the guard, not a second implementation |
| Second lowering, or a phase running on the wrong representation | PL-1, PL-2 |
| A retired spelling is still the implementation | OC-3, TL-4, TL-5 |
| A lookup that still accepts the previous address | OC-4 |
| Two evaluators of one closed proposition | LF-1, PA-1 |
| A second prover with no production caller | PA-2 |
| Three answers to one domain spelling | RS-1, RS-2, RS-3 |
| A second layout of a plan the previous stage stored | IS-1, IS-2, IS-3 |
| Storage whose producer never writes it | LF-2, LF-3 |
| A named surface with no caller | LF-4, LF-5 |

The register is
`wiki/drafts/audits/spec_to_code_audit_2026-11-13.tsv`. Line estimates use
`wc -l` or a counted span, and the command is named on each finding.

### PL-1. The Psi optimization phase has no stage before publication

Pattern: a phase running on the wrong representation. Category: spec
question. Confidence: high on the call sites, medium on what a repair
should do. Owner: Psi publication and the abstract-operations stage.
Estimated deletion: 0. The passes may be the implementation the missing
stage needs.

[Optimization ordering](wiki/spec/terminal-psi/product.md#optimization-ordering)
says the order is lowering, selected Psi optimization, then Terminal
publication. [Phase boundaries](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
say Terminal Psi is the immutable output of Psi optimization, and that a
receiving native lowerer does not rerun Psi passes.

Three call sites do something else:

- `ProductTerminalPublication::accepted` in
  `omega-rust/omega/compilation/products/src/terminal_publication.rs`
  returns an error when `project_psi()` is nonempty: "no product route yet:
  selected Psi optimizations have no accepted evidence transport".
- `PostTerminalOptimizationSelections::new` in
  `omega-rust/omega/foundation/optimization-core/src/optimization_core/post_terminal.rs`
  rejects any optimization whose phase is not post-terminal, so a Psi-phase
  selection never enters native lowering. The comment above the type says
  sealed artifacts have already executed `CheckedTrees` and `Psi`
  optimizations. The checked-tree half of that sentence describes the
  deleted representation.
- `optimize_abstract_operations` in
  `01_abstract-operations-to-abstract-operations/src/lib.rs` calls
  `run_psi_pipeline_for_projection` and returns
  `UnconsumedOptimizationPhase(AbstractOperations)` when that phase's own
  selection is nonempty. `lower_to_target` in
  `native-compiler/src/pipeline.rs` passes `selections.project_psi()` of the
  post-terminal subset, so the production native call carries an empty Psi
  projection. A nonempty projection is reachable from `run_psi_pipeline`,
  which is compiled for tests and `test-support`.

The pre-publication Psi stage is missing. An executor for the Psi catalog
lives in the abstract-operations stage and, on the production native path,
runs as the empty identity. This is the smaller form of Psi 05 containing
`execution*.rs` and Psi 07 optimizing a different representation than the
one the spec names. It is not a second copy of a live Psi stage, because
that stage is not there.

Follow-up: decide where a nonempty Psi selection executes, on Terminal Psi
before publication, or stay refused. Do not delete `run_psi_pipeline_for_projection`
until that decision has a stage to put the passes in.

### PL-2. The target-operations phase is refused by the stage named for it

Pattern: a phase whose stage does not run it. Category: spec question.
Confidence: high. Owner: `02_abstract-operations-to-target-operations`.
Estimated deletion: 0.

`lower_optimized_to_target_operations` in
`02_abstract-operations-to-target-operations/src/lib.rs` returns
`UnconsumedOptimizationPhase(TargetOperations)` when
`optimized.selections().for_phase(TargetOperations)` is nonempty. The same
shape is the `AbstractOperations` refusal inside
`optimize_abstract_operations` (PL-1). [Exact selection](wiki/spec/build/optimizations.md#exact-selection)
gives each phase one owner that executes its projection. These stages
implement the nonempty selection as an error.

The refusal is fail-closed, which is the right response to a phase the
pipeline does not run. The finding is that the vocabulary still names the
phase. Deleting the refusal would make a nonempty selection fall through.
The question is whether `TargetOperations` and `AbstractOperations` remain
selectable phases.

### TL-1. `stage-coverage` still measures the deleted checker

Pattern: sidecar names. A tool walks stages that are not in the pipeline.

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

Pattern: sidecar names.

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

Simplification: drop the two missing roots. Keep the stage 04 root. Do not
add `lowered-discharge` in that edit. Its `src` contains `RefCell` in
`reuse.rs` (the certificate transport at line 191 is not under
`#[cfg(test)]`) and a `thread_local!` in `prove.rs` under `#[cfg(test)]`
(line 245). The scan is a substring search, so those hits would fail
admission before anyone decides whether the uses are allowed. Validation:
`mbx xtask check-rules --base origin/main --title "tools: drop deleted checker roots from the interior-state lint"`.

### TL-3. Forbidden-edge strings still name the deleted crates

Pattern: connection points. These strings are the guard against recreating
the dependency, so the review drops the finding.

Category: likely unnecessary, then dropped in section 4. Confidence before
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

Pattern: parallel representation. The phase belongs to `checked-trees`,
which has no crate.

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

Pattern: sidecar names. The labels are a stage id for a stage that is gone.

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

`record_preliminary_check` in `compile_timings/counters.rs` (lines 128–134)
is the only Rust hit for that name. Nothing increments
`PRELIMINARY_CHECK_CALLS`. `preliminary_check_totals` is still read by
`package-manager` `review/candidate/passes.rs` `note_pass_accounting`, which
prints `preliminary_checks`, and by `compiler/examples/build_session.rs`,
which asserts the count rose by 2. With no writer, that note is always zero
and the example assertion cannot succeed. The two functions and their
statics are 19 lines. Deleting them includes updating those two readers.
`CheckedTreeProductPruning` stays with OC-1.

Validation: `mbx test -p artifacts --lib` and
`mbx clippy -p artifacts --all-targets -- -D warnings`.

### FD-1. The checked-tree measurement identity is still a current schema

Pattern: parallel representation. Schema 7 still has an identity for the
deleted program form.

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

Pattern: sidecar names and connection-point comments.

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
`language-core/{atomic_operations,inline_assembly}.md`. Two stage READMEs
disagree with the call chain in section 2: `04_typed-trees-to-typed-trees`
says the passes are "input the checker reads," and `structure-elaboration`
and `lowered-discharge` say they are not wired into the default compile,
while `RouteScope::judge` calls both.

Simplification: correct the sentences while touching the file for another
reason. No standalone documentation pull request.

### OC-3. The parser still implements the spellings the spec retired

Pattern: a retired spelling is still the implementation. Category: likely
unnecessary. Confidence: high. Owner: CLI. This is a retarget, not a line
deletion. The three parser files are 394 lines (`wc -l` of
`arguments/audit.rs` 130, `arguments/inspect.rs` 98, `arguments/packages.rs`
166).

[Command vocabulary](wiki/spec/build/command_line.md#command-vocabulary)
says there is one public spelling for each operation, and that former flat
install/update, action-first audit, and hyphenated inspection commands are
not compatibility aliases. [Public Psi naming](wiki/spec/build/command_line.md#public-psi-naming)
says the interpreted engine spelling is `--engine psi` and the inspection
command is `psi inspect`. [Source rewriting](wiki/spec/build/command_line.md#source-rewriting)
names `omega source rewrite`.

`arguments/mod.rs` `parse` binds `install`, `update`, `audit`, and
`inspect-terminal` (lines 77–86). `source` and `packages` occur only inside
`parse_audit` (lines 134–142), so `omega packages install` and
`omega source rewrite` are not commands. `arguments/run.rs` accepts
`--engine terminal` (the field comment at line 11, and the errors at lines
195 and 198). A search of `omega-rust/omega/application` Rust for
`psi inspect`, `source rewrite`, `floor-only`, and `--engine psi` finds
none of those spellings.

The tests in the same files lock the retired spellings, including
`help_succeeds_across_every_command_surface`. Replacing the parser without
those tests would put the old tree back at the next edit.

Follow-up: one change retargets `parse` to `packages install|update|audit`,
`psi inspect`, `source rewrite`, and `--engine psi`, and updates the tests
with it. Do not keep the retired spellings as aliases. Validation:
`mbx test -p omega --bin omega` and
`mbx clippy -p omega --all-targets -- -D warnings`.

### OC-4. Local snapshot recovery still scans the previous directory name

Pattern: a lookup that still accepts the previous address. Category: spec
question. Confidence: medium. Owner: packages. Estimated deletion, if the
old name is no longer required: 72 lines (`address.rs` 46–74 is 29,
`recovery.rs` 126–166 is 41, `timings.rs` 75–76 is 2).

Publication addresses are `source-v2-<digest>` (`local/mod.rs`,
`snapshot/address.rs`). On a direct miss, `recover_bounded` in
`local/recovery.rs` enumerates the snapshot directory and accepts a
`source-<64 hex>` name when `is_legacy_snapshot_name` matches and
`verify_selected_snapshot` succeeds. The specification pages for sources
and resolver security do not name `source-<digest>` or `source-v2-`. The
scan does re-verify before return, so it is not a bypass of custody
checking.

`wiki/spec/packages/locks.md` requires lock schemas 4 and 5 as historical
consent. That requirement is not this directory name. If old
`source-<digest>` snapshots are still an operational need, the scan stays.
The question is whether they are.

### TL-4. The testing guide still runs Python gates that are not in the tree

Pattern: a retired spelling is still the documented procedure. Category:
likely unnecessary. Confidence: high. Owner: tools. The full-baseline
section is 78 lines (`sed -n '499,576p' tools/testing.md | wc -l`). Those
are instructions, not an implementation.

At this revision `tools/check_native_boundary.py`, `tools/fmt.py`,
`tools/test_affected.py`, `tools/doctor.py`, and `tools/check_rules.py` are
absent. `tools/testing.md` still tells a full baseline to run the first
four, and the requirements section still runs `tools/test_affected.py` and
describes `tools/doctor.py`. `AGENTS.md` already names the replacements:
`mbx xtask check-rules`, `mbx xtask fmt --check`, `mbx xtask check-boundary`,
and `mbx xtask doctor`. `rules.rs` still says a baseline was written by
`python tools/check_rules.py` and points a source-read ratchet at
`tools/check_native_boundary.py`. `check-rules` ignores `#` lines, so the
false writer name does not change a count. No workflow runs the Python
paths.

Follow-up: point the guide at the xtask commands and delete the
`test_affected.py` selector instructions. Retarget the two `rules.rs`
sentences in the same change. The stage-coverage section of the same file
stays with TL-1.

### TL-5. The swarm slot scripts only see the abandoned pool

Pattern: a retired spelling is still the implementation. Category: likely
unnecessary. Confidence: high. Owner: tools. The three scripts are 107
lines (`claim_slot.sh` 67, `free_slots.sh` 20, `assign_slot.sh` 20).

`AGENTS.md` says the pool is `.agents/worktree/worker-N`, claimed with
`cargo xtask worktree acquire`. The swarm skill names that same command.
`claim_slot.sh`, `free_slots.sh`, and `assign_slot.sh` look only at
`.codex/worktrees/slot-$i`. That directory is not in the tree.
`tools/swarm/launch.py` is absent (removed in `#2086`,
`e537a3b890a`). `tools/swarm/README.md`, `cloud.md`, `dispatch.md`, and
`manifest.schema.json` still describe `launch.py`. No caller outside the
three scripts references them (`rg` of the script names is their own usage
comments). `binary_fresh.sh` does not use this pool, and `tools/testing.md`
still calls it, so it stays.

Follow-up: delete the three scripts and the sentences that only describe
the missing launcher. Dispatch stays `cargo xtask worktree` and
`cargo xtask claims`.

### LF-1. Two closed-proposition walkers disagree when a child is unknown

Pattern: two evaluators of one closed proposition. Category: confirmed
duplication. Confidence: high. Owner: terminal installation and the
interpreter. The two functions are 58 and 85 lines. A repair keeps one
walker, so this is not a net deletion of both.

The [Crash](wiki/spec/terminal-psi/calls_and_outcomes.md#crash) section
requires one rule for both sides. The sentences are:

> Disproving every route removes that cause's edge from the caller's semantic frontier.
>
> The interpreter returns a distinct crash outcome with cause and semantic site, not ordinary result data. Build-time evaluation rejects an invocation with a surviving crash route; an invocation that disproves every route remains admissible.

A route is removed only when it is false. Unknown is not a disproof. In a
conjunction, false is absorbing: `false && unknown` is false, and the
unknown sibling does not keep the route alive. In a disjunction, true is
absorbing. `unknown => true` is true, because a true conclusion does not
depend on the premise. That is the installation walker. The interpreter
walker, which returns unknown as soon as any child is unknown, does not
implement that rule.

`closed_guard` in
`terminal-installation/src/evaluation_and_synchronous_calls.rs` (lines
274–331) keeps scanning a conjunction or disjunction after an unknown
child. A deciding sibling still wins: `false` ends a conjunction and
`true` ends a disjunction. `closed_guard` in
`terminal-interpreter/src/boundary_effects.rs` (lines 397–481) returns
unknown as soon as any child is unknown, so `false && unknown` is unknown
there and false in the installation walker. Implication differs the same
way: the installation walker accepts `unknown => true` (line 327), and the
interpreter walker returns unknown (line 477). The interpreter walker also
evaluates IEEE-float literals (lines 404–427). The installation walker has
no float arm and falls through to unknown.

`InputObservations::evaluate` in the same interpreter file (lines 131–166)
is a third walk of the connectives. It returns on the first deciding
sibling or the first unknown, so the result depends on child order, and
then it calls the interpreter `closed_guard` for a leaf. Build-time
admission calls the installation walker through `admit_compiler_evaluation`
(`source_jobs.rs`, `calling_policy_plans.rs`). Boundary admission calls
the interpreter walker through `admit_boundary_crash`.

The crash-bucket shape test copied beside both walks is already
`terminal_codec::semantic_module::crash_routes_are_canonical`.

The callers ask different questions, and the walker gap changes who is
admitted.

`exclude_failure_routes` admits the build-time invocation only when every
alternative is `Some(false)`. `Some(true)` rejects with "the checked crash
route can occur". `None` rejects with "cannot be proved impossible". That
is the spec's build-time sentence: a surviving route rejects the call, and
a disproof of every route admits it.

`validate_boundary_crash_outcome`, which `admit_boundary_crash` calls,
admits the crash outcome on `Some(true)`. Every alternative `Some(false)`
returns `GuardFalse`. Any `None` returns `UnsupportedGuard`, which
`admit_boundary_crash` surfaces as `BoundaryCrashNotPermitted`. The
interpreter then neither admits the crash nor records the guard as false.

On a bucket whose only alternative is `false && unknown`, build-time
admits the invocation and runtime rejects a reported crash as
`UnsupportedGuard`. Build-time admits a call whose route the runtime
walker cannot show is impossible. Runtime does not admit that crash.
The false conjunct really does disprove the conjunction, so the
installation answer is the one the spec's "disproves every route" sentence
allows, and the runtime answer is a missed disproof.

On `unknown => true`, build-time rejects the invocation because the
installation walker returns `Some(true)` and the route survives. Runtime
returns `None` and rejects the crash as `UnsupportedGuard`. Both refuse.
Only build-time records a surviving route.

On an IEEE-float literal comparison the installation walker has no arm, so
it returns `None` and build-time rejects the invocation. The interpreter
walker can return `Some(true)` and admit the crash, or `Some(false)` and
reject it as `GuardFalse`. Runtime can admit a crash outcome for a float
guard that build-time refused to let run. Neither walker admits an
invocation, or a crash, that the other walker has evaluated to false.

`InputObservations::evaluate` walks the same connectives a third time, for
`scalar_requires` (lines 101–118). `Some(true)` continues. `Some(false)`
and `None` both reject the boundary call. Because it stops at the first
unknown, `false && unknown` is `Some(false)` when `false` is first and
`None` when `unknown` is first. The call is rejected either way. The order
changes the error text, not the admission.

The repair is one walker, the installation rule, plus the float arm the
installation walker lacks. Follow-up 5 has the failing twin.

### LF-2. Check-table summaries and may-sets have no producer

Pattern: storage nothing writes. Category: likely unnecessary. Confidence:
high. Owner: terminal check tables.

`CheckTables::summaries` is a default-empty table. `lowered-discharge`
`places.rs` says it stays empty. `Summary` and `SummaryBasis` in
`check-tables/src/edges.rs` are not constructed, and their `reach` and
`invokes` fields are `MaySet`. `MaySet::of` and `MaySet::Unknown` occur
only in `may_sets.rs` tests. Service reach that the checker does judge is
`TerminalMachine::published_service_ceiling`, which `ceilings::judge`
reads. That record stays.

Follow-up: drop the unread summary column, `Summary`, and `MaySet`. Leave
the module ceiling. Validation: `mbx test -p terminal-check-tables --lib`,
`mbx test -p structure-elaboration --lib`, and
`mbx test -p lowered-discharge --lib`.

### LF-3. Scalar-leaf elision rewrites an operation, and nothing calls it

Pattern: a second discharge beside the kernel. Category: likely
unnecessary. Confidence: high. Owner: terminal semantics. The `elision`
module in `semantics/src/scalars.rs` is lines 802–1191.

`elidable_proof_bearing_scalar_leaf` asks for the canonical goal, decides
that goal from literals, and returns a replacement `OperationKind`. It
does not call `proof_admission`. `rg` of the function and of
`canonical_goal_is_discharged` finds only that module and the re-export in
`semantics/src/lib.rs`. [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md)
say a producer proves the canonical goal and cannot substitute a different
question. [Optimization ordering](wiki/spec/terminal-psi/product.md#optimization-ordering)
says an empty selection constructs no rule candidates. Publication records
an empty Psi selection. `lowered-discharge` is the path that states this
goal and sends it to the kernel.

Follow-up: delete the module and the re-export. The canonical-goal
constructor stays. Validation: `mbx test -p terminal-semantics --lib` and
`mbx test -p lowered-discharge --lib`.

### LF-4. The syntax-only proof crate has no production caller

Pattern: a named surface with no caller. Category: spec question.
Confidence: high that nothing calls it, medium on whether the architecture
still wants the crate. Owner: `psi/compilation/proof`. Non-test lines are
about 961.

`pipeline.md` assigns this directory syntax-only proof surface
descriptions and keeps `proof/admission` as the mathematical checker. The
crate comment says the reports establish no source judgment and no
certificate. `build_proof_surface_report`, `BoundaryProofObligation`, and
`ProofLemma::discharging` occur only under `proof/src`. The workspace
`Cargo.toml` lists the package. No other manifest depends on it. The
specification pages for proofs do not name these reports. The lemma search
is a second prover beside `verify_obligation`, and it has no caller either.

Follow-up: if the surface is not a product report, drop the package and
leave `proof-admission`. If a report is required, name the caller. Do not
delete the kernel.

### LF-5. Production stage labels still name CheckedTrees and Lowered Psi

Pattern: sidecar names. Category: likely unnecessary. Confidence: medium.
Owner: terminal production. The label table is 28 lines.

`OBSERVATION_LABELS` in `terminal-production/src/lib.rs` (lines 51–78)
records `CheckedTrees`, `LoweredPsi`, and `OptimizedLoweredPsi` for machine
selection, lowering, optimization, entry receipt, and publication. The
publication row says its input is `OptimizedLoweredPsi`. Those three
representations were the deleted chain. `pipeline.md` says the served
route is structural elaboration, lowered-form judgment, and accepted-source
publication, with no checked-tree or legacy optimization fallback.

Follow-up: label the live publication and entry-receipt rows with the
representations they carry. Leave elaboration and discharge as the route.
Validation: `mbx test -p psi-terminal-production --lib`.

### PA-1. Predicate denotation decides closed IEEE comparisons

Pattern: a second evaluator of one closed comparison. Category: confirmed
duplication. Confidence: high. Owner: proof admission. The float function
and its call are about 40 lines (`closed_ieee_float_relation` at 586–618
and the branch at 522–528).

[Fact validity](wiki/spec/terminal-psi/fact_validity.md) says the bounded
predicate-denotation conversion decides equality and order between two
literal fixed integers, and then: "Open integer relations, mathematical
arithmetic, IEEE relations, and storage observations remain unchanged."
[Integer certificates](wiki/spec/terminal-psi/integer_certificates.md)
says the same conversion "evaluates no open storage observation, arithmetic
expression, or IEEE relation."

`normalize` in `proof/admission/src/predicate_denotation.rs` calls
`closed_ieee_float_relation` before the integer-literal arm. Two
`IeeeFloatLiteral` operands become `Truth` or `Falsehood` through
`FloatSemantics`. The interpreter `closed_guard` decides the same six
comparison kinds. The installation walker has no float arm and returns
unknown. `normalize` runs on the proof path: `lowered-discharge`
(`prove.rs`, `joins.rs`, `prove/ranges.rs`, `known.rs`) and terminal
semantics (`descriptors.rs`, `scalars.rs`).

The challenge stands. This is not the integer conversion the spec
requires. Collapsing `+0 == -0` to `Truth` lets a later proof step treat
an IEEE relation as a Boolean constant. The repair leaves
`ScalarIeeeFloatComparison` unchanged in `normalize`. Crash-route
evaluation stays on the one walker from LF-1. The fixed-integer arm stays.

### PA-2. The source-integer theory search has no production caller

Pattern: a second prover beside the kernel. Category: likely unnecessary.
Confidence: high. Owner: proof admission. About 760 lines, not counting
the two test modules.

`SourceIntegerTheory` is constructed only inside the `#[cfg(test)]`
modules in `bounded_denotation.rs` (lines 12662, 13138, and 13177).
`closed_predicate` and `predicate_from_assumptions` build a proof term
from closed arithmetic and an assumption search. They do not call
`verify_obligation`. [Proof foundation](wiki/spec/proofs/foundation.md)
says elaboration and search are untrusted producers, and that the kernel
does not accept a producer's Boolean success flag. No spec page names
this theory.

The challenge stands. Deleting the search leaves the `Denotation` integer
vocabulary and the law tables that certificate elaboration calls.
`verify_obligation` stays.

### RS-1. Three domain lookups, and they do not share a collapse rule

Pattern: three answers to one name. Category: confirmed duplication.
Confidence: high. Owner: the symbol resolver. About 104 lines:
`resolve_domain_symbol` is `symbols.rs` 1590–1662, and `domain_named` is
`lowering_machine.rs` 837–868.

[Modules](wiki/spec/language/modules.md) says distinct visible declarations
competing for the same domain reject, and neither carrier ownership nor
import order selects a winner.

`membership_domain_lookup` unions a top-level hit with a scan of root
children, then `prefer_module_local_domain`. Equal display paths collapse.
Several remaining paths are `Contested`, which `membership_selection`
returns as `Ok(None)`. A domain competing with a case is an error.
Ordinary expression membership uses this path through
`assign_membership_symbol`.

`resolve_domain_symbol` is what a proof-fact membership writes into
`domain_symbol`. It returns invalid when any pooled row differs in
authored name or semantic id. It does not look at cases. It does not call
`membership_selection`.

`domain_named` returns immediately on a signature-free `Unique` or
`Ambiguous`, before the `domain_definitions` scan and before
`prefer_module_local_domain`. `mark_token_bound_domain_homes` uses it.

The challenge corrected one sentence from the first write-up. These are
not one field written twice. Proof facts and ordinary expressions are
different slots, and each slot calls a different function. The
disagreement stays: the same spelling can be a domain, contested, or a
different module-local row depending on which function runs.

Follow-up: one pool, the membership rule. The other two call it.

### RS-2. Operator homes use string maps; later facts use the symbol table

Pattern: a second name map. Category: confirmed duplication. Confidence:
high. Owner: the symbol resolver. About 158 non-comment lines:
`NamespaceScope` is `selection.rs` 2561–2692, and `explicit_domain_homes`
is 2842–2879.

[Pipeline](omega-rust/pipeline.md) says scoped symbol-tree lookup is the
baseline and extra lookup maps need a measured reason.
[Modules](wiki/spec/language/modules.md) says importing a declaring module
exposes its directly declared public domains.

`select_operator_homes` runs before `symbols::assign`. `NamespaceScope`
fills two `HashMap`s. `domain_exposed_to` is true for an exact path
string, an empty module, the same module string, or a `::`-free spelling
whose import text equals the domain path. Later domain facts,
`resolve_domain_symbol`, `membership_domain_lookup`, and `domain_named`
use `domain_name_reaches`, which accepts a leaf through
`same_semantic_name` and a module import. A qualified spelling such as
`Carrier::Leaf` can reach through the symbol table and fail
`domain_exposed_to`, because the authored text contains `::` and is not
the full path string. The operator home is not recomputed after the
symbol table exists.

Follow-up: install the namespace symbols first and home operators with
`domain_name_reaches`. Drop `NamespaceScope`.

### RS-3. Two child scans, and the static call keeps only `entry`

Pattern: two scans of one machine's children. Category: confirmed
duplication. Confidence: high. Owner: the symbol resolver. About 45 lines:
the matching loop is `symbols.rs` 2049–2076, and the entry arm in
`static_call_target` is `symbols_targets.rs` 905–921.

[Modules](wiki/spec/language/modules.md) says ambiguous imported
declarations reject and traversal order cannot select between them.

`find_child_by_name_and_kind` keeps the requested kind and drops symbols
that fail `symbol_source_is_selected`. Two selected children of that kind
and name under a module return none.

`child_symbol_by_kinds_matching` does not call
`symbol_source_is_selected`. Under a module parent the kind test is
skipped, so `Data foo` beside `Machine foo` is invalid here and a machine
there.

`machine_entry_symbol` uses the local scan: a state named the call target,
then `"entry"`. `static_call_target` asks
`find_child_by_name_and_kind` for `"entry"` only.
`normalize_static_module_calls` assigns that result after the call already
has a target. A state named the call target wins in the first scan and
loses in the static pass.

Follow-up: the local scan calls `find_child_by_name_and_kind` per kind.
`static_call_target` calls `machine_entry_symbol`.

### IS-1. Legalization rebuilds sum layouts the target row already stores

Pattern: a second layout of a sealed plan. Category: confirmed duplication.
Confidence: high. Owner: instruction selection. About 213 lines for the
solvers that recompute a stored result.

[Pipeline](omega-rust/pipeline.md) says the default compile path does not
rebuild output the same compile just produced.
[Exact selection](wiki/spec/build/optimizations.md) says legalization stays
necessary, and that does not require a second layout of a plan the
previous stage sealed.

`TargetUnitOperation::StructuralCaseMembership` stores `case_tag` and
`tag_byte_offset`. `project_structural_case_membership` does not read that
row. It calls `membership_layout`, which walks the type and builds the
layout again. `EstablishCase` and the activation-claim arms call
`sum_layout` the same way. `home_layout` calls `sum_type_layout` and then
`sum_layout` again and drops the second result. `StructuralCase` stores
`byte_offset` and `layout`; `dispatch_projection` rebuilds both.

The off-path checker in `legalization_target.rs` is the validator stage 02
already documents. It stays off the default path.

Follow-up: copy the stored tag, offset, and layout onto the legalized row.
Keep one sum walker.

### IS-2. Selection recomputes the layout legalization just attached

Pattern: a second judgment of the row just built. Category: confirmed
duplication. Confidence: high. Owner: instruction selection. About 142
lines: `membership_layout` in `selection_inputs.rs` 705–762, the sum
rebuild in `fields` at 1232–1296, and the geometry check in
`structural_case::build` at 6492–6510.

On `select_instructions`, `case_membership::observe` errors unless the
recomputed tag and offset equal the legalized row.
`fields` requires `evaluate_conventional_sum_layout` to equal the layout
already on the row. `structural_case::build` errors when the recomputed
`byte_offset` differs, then uses `pointer_slots` that the legalized case
row does not store. The equality check is the second judgment. The slot
list is the part legalization dropped.

Follow-up: keep the pointer-slot list on the legalized row. Emit the case
from that row. Delete the selection `membership_layout` and the sum-layout
equality in `fields`.

### IS-3. Selection re-runs the calling-convention plan

Pattern: a second run of one ABI function. Category: likely unnecessary.
Confidence: medium. Owner: instruction selection. About 22 lines:
`scalar_call_abi::validate` at `selection_call_abi.rs` 1698–1702 and
`cleanup_call::emit` at `selection_scalar_graph.rs` 2484–2500.

`validate` builds `canonical = evaluate_call_plan(...)` and returns an
error when `call.call_plan != canonical`. Legalization already copied
`call_plan` from the target operation. `evaluate_call_plan` is one
function in `calling_conventions`. This is a second run on the default
path, not a second ABI. The installation-role and owned-argument checks
in the same function are selection work and stay.

The challenge stands at medium confidence. A reviewer can treat the
equality as a local refusal of a plan selection is about to encode. The
spec still says the default path does not re-validate output the same
compile just produced.

## 4. Independent review

This pass re-read the callers of each finding above and tried to break the
simplification. It did not re-open the directory census.

| Id | Verdict |
| --- | --- |
| TL-1 | Stands. `tools/testing.md` says CI does not call the command, and the three directories are gone. A zero column is not a regression guard worth a full instrumented compiler build. |
| TL-2 | Stands, narrowed. The function is live because stage 04 is real. Only the two missing roots are dead. Leaving them makes a failed `read_dir` look like a clean scan. Retarget; do not delete the lint, and do not add `lowered-discharge` in that edit. Its `RefCell` and test `thread_local!` would fail the substring scan. |
| TL-3 | Dropped. `boundary.rs` lists `checked-trees`, `representation-planning`, Psi 05, and Psi 07/08 as forbidden dependency edges, including the comment that retired Lowered Psi stages do not return through the Psi facade. The strings do not match a Cargo edge today. They are the guard that fires if those paths are recreated and depended on. Deleting them weakens the firewall the deletion just wrote down. A missing directory does not erase a path dependency from `cargo metadata`: the match is the declared path. On this tree no `Cargo.toml` names those prefixes, so the gate sees nothing to flag. |
| OC-1 | Stands as a spec question. The 10-line helper has no caller and can go. The tag-22 variant and the specification sentence cannot go in that same change: `OMGOPT` version 15 and `validate_optimization_rollback` still treat the phase as nameable. |
| OC-2 | Stands for the two unread constants and for `record_preliminary_check`, which has no caller. `BUILD_AND_CHECKED_CONTINUATION` is passed from two live call sites, so the follow-up renames its text and does not delete the constant. Deleting the preliminary-check counter includes the review note and the `build_session` example that still read it. |
| OC-3 | Stands. `parse` binds the spellings `command_line.md` says are not aliases, and it does not bind `packages install`, `psi inspect`, `source rewrite`, or `--engine psi`. The change is a retarget of one parser, with its tests. |
| OC-4 | Stands as a spec question. The legacy `source-<digest>` scan re-verifies before return. The spec does not say whether those directories are still required. |
| TL-4 | Stands. The Python gates named in `tools/testing.md` are not in the tree. Point the guide at the xtask commands. |
| TL-5 | Stands for the three slot scripts. They only look at `.codex/worktrees`, which is not the pool `AGENTS.md` names. `binary_fresh.sh` stays. |
| FD-1 | Stands as a spec question. Tag 0 may already be stored in build records; this pass did not prove that no record uses it. The sponsor charge methods that refuse a `TerminalPsi` account are the fence `accounts.rs` tests. Do not delete that fence. |
| DOCS-1 | Stands, and is not its own change. The comments do not affect compilation. The stage READMEs that say elaboration and discharge are unwired are stale relative to `RouteScope::judge`. |
| PL-1 | Stands as a spec question. Publication refuses a nonempty Psi selection, native lowering excludes pre-terminal selections, and the catalog executor sits in the abstract-operations stage. Do not delete the passes to make the map look clean. |
| PL-2 | Stands as a spec question. The `TargetOperations` and `AbstractOperations` refusals are fail-closed. The question is whether those phases stay selectable. |
| LF-1 | Stands. On `false && unknown`, build-time admits the invocation and runtime rejects a reported crash as `UnsupportedGuard`. On `unknown => true`, both refuse, and only build-time records a surviving route. Neither side admits a case the other has evaluated to false. The shared walker is the installation rule, plus the float arm. |
| LF-2 | Stands. The summary column and `MaySet` have no production constructor. The module ceiling `ceilings::judge` reads stays. |
| LF-3 | Stands. The elision module has no caller outside its re-export. Discharge stays in `lowered-discharge` and `proof_admission`. |
| LF-4 | Stands as a spec question. Nothing depends on package `proof`. `pipeline.md` still names the directory. `proof-admission` stays. |
| LF-5 | Stands. The publication label still says its input is `OptimizedLoweredPsi`. |
| PA-1 | Stands. `normalize` collapses a closed IEEE literal comparison to `Truth` or `Falsehood`. Fact validity says IEEE relations remain unchanged in this conversion. The fixed-integer arm stays. |
| PA-2 | Stands. `SourceIntegerTheory` is constructed only in the two test modules. The kernel stays. |
| RS-1 | Stands, with one correction. Proof-fact membership and ordinary expression membership are different slots. Each calls a different function. `domain_named` returns on a signature-free unique hit before `prefer_module_local_domain`. The three rules do not collapse the same way. |
| RS-2 | Stands. `domain_exposed_to` matches path strings. Later facts use `domain_name_reaches`. A qualified spelling can pass one and fail the other. |
| RS-3 | Stands. `child_symbol_by_kinds_matching` does not call `symbol_source_is_selected`. `machine_entry_symbol` prefers a state named the call target. `static_call_target` asks only for `"entry"`. |
| IS-1 | Stands. `StructuralCaseMembership` stores `case_tag` and `tag_byte_offset`. `project_structural_case_membership` ignores that row and calls `membership_layout`. The off-path checker in `legalization_target.rs` stays off the default path. |
| IS-2 | Stands. Selection rebuilds the same layout and errors when the copy differs. The pointer-slot list is the part the legalized row dropped. |
| IS-3 | Stands as likely unnecessary, medium confidence. `validate` re-runs `evaluate_call_plan` and errors when the plans differ. That is one ABI function run again, not a second ABI. |

Confirmed duplication is no longer only LF-1. Similar names
(`CheckedCompilation`, `compiler/src/checked/`, `PsiOptimization`) are the
current product coordinator and the abstract-operation optimizer, which have
live callers.

## 5. Ranking and follow-ups

Confirmed duplications come first, because each one can pick a different
answer or rebuild a result the previous stage stored. Inside that group,
LF-1 stays first: the repair keeps one walker, so its line count is zero,
and sorting it by lines would bury the crash-guard disagreement. The other
confirmed rows are ordered by lines the repair removes. Likely-unnecessary
rows follow, largest deletion first. Retargets that rewrite instead of
delete (OC-3) come after those. Spec questions come last, ordered by the
lines that would go if the answer is to delete. Confidence breaks ties.

Confirmed duplication: 7 (LF-1, PA-1, RS-1, RS-2, RS-3, IS-1, IS-2).
Likely unnecessary: 12 (TL-1, TL-2, OC-2, DOCS-1, OC-3, TL-4, TL-5, LF-2,
LF-3, LF-5, PA-2, IS-3). Unresolved spec questions: 6 (OC-1, FD-1, PL-1,
PL-2, OC-4, LF-4). Dropped by review: 1 (TL-3).

| Rank | Id | Lines a follow-up can delete | Why this order |
| --- | --- | ---: | --- |
| 1 | LF-1 | 0 | First in the confirmed group. The walkers disagree on `false && unknown`. The repair keeps one walker. |
| 2 | IS-1 | 213 | Confirmed. Legalization rebuilds a layout the target row already stores. |
| 3 | RS-2 | 158 | Confirmed. Operator homes use string maps. Later facts use the symbol table. |
| 4 | IS-2 | 142 | Confirmed. Selection rebuilds the layout legalization just attached. |
| 5 | RS-1 | 104 | Confirmed. Three domain lookups do not share a collapse rule. |
| 6 | RS-3 | 45 | Confirmed. The static call keeps only `entry`. The first scan prefers a state named the call target. |
| 7 | PA-1 | 40 | Confirmed. `normalize` decides an IEEE relation the spec says stays unchanged. |
| 8 | PA-2 | 760 | The source-integer theory search. Constructed only in tests. |
| 9 | TL-1 | 499 | `stage_coverage.rs` plus its `main.rs` arm. |
| 10 | LF-3 | 390 | `elision` in `semantics/src/scalars.rs`, lines 802–1191. No caller. |
| 11 | LF-2 | 146 | `may_sets.rs` is 93 lines. `Summary` through `Termination` in `edges.rs` is lines 127–179 (53). Nothing constructs them. The module ceiling stays. |
| 12 | TL-5 | 107 | The three `.codex` slot scripts. |
| 13 | LF-5 | 28 | The `OBSERVATION_LABELS` table. A relabel, not a deleted stage. |
| 14 | OC-2 | 23 | Two unread stage constants (4) and the uncalled preliminary-check recorder (19). |
| 15 | IS-3 | 22 | The calling-convention equality. One ABI function, run again. Medium confidence. |
| 16 | OC-1 | 10 | The uncalled helper. The wire tag stays until the spec question is answered, so only the helper is in this count. |
| 17 | TL-2 | 2 | Two missing roots. The stage 04 scan stays. |
| 18 | TL-4 | 0 | The testing guide's Python commands. A rewrite of instructions. |
| 19 | DOCS-1 | 0 | Stale sentences. No behavior. |
| 20 | OC-3 | 0 | Retarget. The parser implements spellings the spec forbids. |
| 21 | LF-4 | 961 | Spec question. Package `proof` has no caller. `pipeline.md` still names the directory. |
| 22 | OC-4 | 72 | Spec question. The legacy snapshot scan re-verifies. |
| 23 | FD-1 | 0 | Spec question. Keep the decode arm and the refusal fence. |
| 24 | PL-1 | 0 | Spec question. Do not delete the Psi passes. |
| 25 | PL-2 | 0 | Spec question. Keep the phase refusal. |

Lines the deletion follow-ups can remove, from the rank table: 213 (IS-1)
+ 158 (RS-2) + 142 (IS-2) + 104 (RS-1) + 45 (RS-3) + 40 (PA-1) + 760 (PA-2)
+ 499 (TL-1) + 390 (LF-3) + 146 (LF-2) + 107 (TL-5) + 23 (OC-2) + 22 (IS-3)
+ 10 (OC-1's helper) + 2 (TL-2) = 2,661. LF-5 rewrites its 28-line label
table. LF-1 keeps one walker. OC-3 rewrites the parser. Generated,
vendored, and fixture reduction: 0. Documentation rewrites are the
stage-coverage section in `tools/testing.md` (lines 440–497), the Python
baseline commands (TL-4), and DOCS-1.

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

### Follow-up 5. Share one closed-proposition walker

`psi: one crash-guard walker for build-time admission and the interpreter`

The installation rule is the one the Crash section requires: an unknown
sibling does not hide a deciding child, and `unknown => true` is true.
Add the IEEE-float arm the installation walker lacks, so a float guard is
not `None` on one side and a boolean on the other.

The failing twin goes in `terminal-interpreter` and calls `closed_guard`
on two propositions: a conjunction of `Falsehood` and an unknown child,
and an implication whose premise is unknown and whose conclusion is
`Truth`. It expects `Some(false)` and `Some(true)`. On the current
interpreter walker both return `None`, so this test fails today. The same
two propositions through the installation walker already return
`Some(false)` and `Some(true)`; a matching test there passes today. After
the shared walker, both tests pass. `InputObservations::evaluate` uses the
same walker, so `false && unknown` no longer depends on child order.

Checks:

- `mbx test -p terminal-installation --lib < /dev/null`
- `mbx test -p terminal-interpreter --lib < /dev/null`
- `mbx clippy -p terminal-installation -p terminal-interpreter --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `mbx xtask check-rules --base origin/main --title "psi: one crash-guard walker for build-time admission and the interpreter"`

### Follow-up 6. Delete the uncalled scalar-leaf elision

`psi: drop the uncalled scalar-leaf elision`

Delete `mod elision` in `semantics/src/scalars.rs` (lines 802–1191) and the
re-export of `elidable_proof_bearing_scalar_leaf` in `semantics/src/lib.rs`.
Leave `proof_bearing_scalar_leaf_semantics`. Discharge stays in
`lowered-discharge` and `proof_admission`.

Checks:

- `mbx test -p terminal-semantics --lib < /dev/null`
- `mbx test -p lowered-discharge --lib < /dev/null`
- `mbx clippy -p terminal-semantics --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `rg -n 'elidable_proof_bearing_scalar_leaf|canonical_goal_is_discharged' omega-rust`

### Follow-up 7. Drop the unread check-table summary and may-sets

`psi: drop unread check-table summaries`

Delete `may_sets.rs`, `Summary`, `SummaryBasis`, `CrashRoute`, and
`Termination` in `edges.rs` (lines 127–179), and the `summaries`,
`referent_sets`, and `selection_sets` fields. Leave
`TerminalMachine::published_service_ceiling` and `ceilings::judge`.

Checks:

- `mbx test -p terminal-check-tables --lib < /dev/null`
- `mbx test -p structure-elaboration --lib < /dev/null`
- `mbx test -p lowered-discharge --lib < /dev/null`
- `mbx clippy -p terminal-check-tables -p structure-elaboration -p lowered-discharge --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `rg -n 'MaySet|CheckTables::summaries|struct Summary' omega-rust/psi`

### Follow-up 8. Relabel publication with the representations it carries

`psi: stop labeling publication as OptimizedLoweredPsi`

`OBSERVATION_LABELS` in `terminal-production/src/lib.rs` (lines 51–78)
still names `CheckedTrees`, `LoweredPsi`, and `OptimizedLoweredPsi`. Point
the live publication and entry-receipt rows at typed structure and the
accepted Terminal artifact. Do not revive those stages.

Checks:

- `mbx test -p psi-terminal-production --lib < /dev/null`
- `mbx clippy -p psi-terminal-production --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `rg -n 'OptimizedLoweredPsi|CheckedTrees' omega-rust/psi/compilation/psi-compiler/terminal-production`

### Follow-up 9. Leave IEEE relations unchanged in predicate denotation

`psi: stop deciding IEEE relations in predicate denotation`

Remove the `closed_ieee_float_relation` branch from `normalize`. A closed
IEEE literal comparison stays `ScalarIeeeFloatComparison`. The
fixed-integer arm stays. Add a test that a literal IEEE comparison does
not become `Truth` or `Falsehood`. That test fails on the current
`normalize`.

Checks:

- `mbx test -p proof-admission --lib < /dev/null`
- `mbx test -p lowered-discharge --lib < /dev/null`
- `mbx clippy -p proof-admission --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `mbx xtask check-rules --base origin/main --title "psi: stop deciding IEEE relations in predicate denotation"`

### Follow-up 10. Delete the uncalled source-integer theory search

`psi: drop the uncalled source-integer theory search`

Delete `SourceIntegerTheory`'s search and the helpers only it calls
(`source_rewrite_proof`, `closed_source_predicate`,
`source_assumption_predicate`). Keep the `Denotation` integer vocabulary
and the law tables certificate elaboration calls. Keep `verify_obligation`.

Checks:

- `mbx test -p proof-admission --lib < /dev/null`
- `mbx clippy -p proof-admission --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `rg -n 'SourceIntegerTheory::' omega-rust/psi/compilation/proof/admission/src/bounded_denotation.rs`

### Follow-up 11. Copy the stored sum layout through legalization and selection

`backend: stop rebuilding sum layouts the target row already stores`

Legalization copies `case_tag`, `tag_byte_offset`, `byte_offset`, and
`layout` from the target row, and it keeps the pointer-slot list on the
legalized case. Selection emits from that row. Delete selection's
`membership_layout` and the sum-layout equality in `fields`. Leave
`legalization_target.rs` as the off-path checker.

Checks:

- `mbx test -p target-operations-to-selected-instructions --lib < /dev/null`
- `mbx clippy -p target-operations-to-selected-instructions --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`

### Follow-up 12. One domain lookup

`psi: one domain lookup for expressions, proof facts, and token homes`

`resolve_domain_symbol` and `domain_named` call `membership_domain_lookup`.
A proof-fact membership uses that result and still rejects a competing
case. A signature-free unique hit does not skip `prefer_module_local_domain`.

Checks:

- `mbx test -p syntax-trees-to-symbol-resolved-trees --lib < /dev/null`
- `mbx clippy -p syntax-trees-to-symbol-resolved-trees --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`

### Follow-up 13. Home operators with the symbol table

`psi: home operators with domain_name_reaches`

Install namespace symbols before operator homes. `domain_exposed_to` goes.
`NamespaceScope` goes.

Checks:

- `mbx test -p syntax-trees-to-symbol-resolved-trees --lib < /dev/null`
- `mbx clippy -p syntax-trees-to-symbol-resolved-trees --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`
- `rg -n 'struct NamespaceScope' omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees`

### Follow-up 14. One child scan for the machine entry

`psi: one child scan for the machine entry`

`child_symbol_by_kinds_matching` calls `find_child_by_name_and_kind` per
requested kind. `static_call_target` calls `machine_entry_symbol`, so a
state named the call target still wins.

Checks:

- `mbx test -p syntax-trees-to-symbol-resolved-trees --lib < /dev/null`
- `mbx clippy -p syntax-trees-to-symbol-resolved-trees --all-targets -- -D warnings < /dev/null`
- `mbx xtask fmt --check`

## 6. Large-crate coverage

Each of the six crates named below gets its own pass. A clean result means
the searches for a second walker, a repeated lowering, or a parallel lookup
found nothing new. It is not a line-by-line reading of every file.

### Interpreter, clean

`omega-rust/psi/interpreter` is 56,772 lines of Rust. `git diff --stat
a93022df945edf9147d3e75332c7ee944594029a -- omega-rust/psi/interpreter` is
empty. The only proposition walk is `InputObservations::evaluate` and
`closed_guard` in `boundary_effects.rs`, which is LF-1. Crash admission
uses that same `closed_guard`. `verify_obligation` does not appear.
`rg -n "fn evaluate" omega-rust/psi/interpreter --glob '*.rs'` is 1 hit.
`rg -n "closed_guard"` in that tree is 7 hits, all in `boundary_effects.rs`.
`rg -n "fn admit_"` is 3 hits: `admit_boundary_crash` (LF-1), plus
`admit_isolated_simulated_domain` and `admit_runtime_supplies`, which do
not walk a proposition. `CheckedTrees` appears in a test that asserts the
observation text does not contain it.

Not line-read: `storage_bytes_and_views.rs`, `storage_records_and_arrays.rs`,
`storage_providers.rs`, `storage_references.rs`, and `engine_structural.rs`.
Those files were searched for `Proposition`, `CrashRoute`, `evaluate`, and
`CheckedTrees`. No `mbx` run.

### Proof admission, two findings

`omega-rust/psi/compilation/proof/admission` is 48,667 lines.
`git diff --stat a93022df945edf9147d3e75332c7ee944594029a -- omega-rust/psi/compilation/proof/admission`
is empty. `verify_obligation` stays. Two new findings: PA-1, `normalize`
decides a closed IEEE comparison the spec says this conversion leaves
unchanged, and PA-2, `SourceIntegerTheory` is constructed only in tests.

Not line-read: the integer-affine, cast, shift, and forbidden-root replay
bodies, the quotient and indexed-scheme bodies in `mathematical_core.rs`,
and the `mathematical_typing.rs` judgment arms. No `mbx` run.

### Symbol resolver, three findings

`omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees`
is 66,064 lines. `git diff --stat a93022df945edf9147d3e75332c7ee944594029a --`
that directory is empty. RS-1, RS-2, and RS-3 are the three name answers.
`begin_complete` and `resolve_deferred` call one function twice, after
`machine_equations::complete` rewrites syntax. That is not a second name
map. `ResolvedIndexes` maps a symbol to a row. `lexical_integer_const_values`
is a literal ledger. `SourceLinks` is read by `complete`.

Not line-read: the rest of the resolver beyond the three lookups and their
callers. No `mbx` run.

### Instruction selection, three findings

`omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions`
is 68,336 lines. `git diff --stat a93022df945edf9147d3e75332c7ee944594029a --`
that directory is empty. IS-1 and IS-2 rebuild a sum layout the target row
already stores. IS-3 re-runs `evaluate_call_plan`. The off-path checker
`legalization_target.rs` stays off the default path and is not a new
finding. Stage 04's SelectedLowering ownership and stage 02's refusal of a
nonempty TargetOperations selection are not re-filed.

Not line-read: `selection_access_lifecycles.rs`,
`selection_views_and_bytes.rs`, `legalization_reference_custody.rs`, and
`legalization_target.rs` beyond the case and layout matches. No `mbx` run.

### What this pass could not verify

- Whether any on-disk build record still encodes measurement tag 0. The
  codec can write and read it; this pass did not search artifact stores.
- Runtime of `stage-coverage`. The directories are absent, so a run would
  report zeros, and this pass did not build the instrumented compiler.
- `mbx test` and clippy for the packages above. The audit is read-only and
  did not compile.
- Which CLI entry points reach `RouteScope::judge`. The function calls
  elaboration and discharge. This pass did not walk every command flag to
  that function, so the stage READMEs' "not wired" claim is contradicted
  for the product route and unverified for every other route.
- Two large crates are still unread: typed-trees (71,141) and terminal
  semantics (53,446). Proof admission's integer-affine, quotient, and
  typing-arm bodies were not line-read. The resolver and instruction
  selection files named in section 6 were searched, not line-read. The
  interpreter's storage files listed in section 6 were searched, not
  line-read.

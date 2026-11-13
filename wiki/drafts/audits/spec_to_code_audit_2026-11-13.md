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

Not started. The register is
`wiki/drafts/audits/spec_to_code_audit_2026-11-13.tsv`.

## 3. Independent review

Not started.

## 4. Ranking and follow-ups

Not started. No follow-up change is proposed until section 3 has reviewed
the highest-impact findings.

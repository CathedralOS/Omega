# Python outside `tools/`, 2026-11-13

The owner wants every Python file outside `tools/` identified so it can be deleted or replaced by `mbx xtask`. This inventory is that list. It does not change the compiler, and it does not propose a new Python tool.

`git ls-files '*.py'`, excluding `tools/`, `wiki/drafts/`, and `.git`, is **431** files at `68ac8495009d576f7086e502ab776adbfe62602b`: 277 under `samples/`, 142 under `tests/`, 9 under `bootstrap/`, and one each under `source/`, `omega-rust/`, and `.agents/`. The rows are in [`python_inventory_2026-11-13.tsv`](python_inventory_2026-11-13.tsv) (`path`, `class`, `evidence`, `group`).

Nothing in `.github/workflows/` reaches any of them. Admission, post-merge, Pages, and the warm build run `mbx` (`check-rules`, `fmt`, `check-boundary`, Clippy, two lowered-route tests, `sample-gate` on `samples/cli/facts/length/local`, and `mbx build`). The optional full post-merge test job runs `mbx test --workspace`. That job would compile `package_review.rs`, which calls `python3` on `tests/omega/pass/layouts/private_callback_inherited_policy_exit/fixture/prepare.py` (`omega-rust/omega/compilation/compiler/tests/package_review.rs:752` and `:789`). That path is not in this census. The kept file is `tests/fixtures/pass/layouts/private_callback_inherited_policy_exit/fixture/prepare.py`, and nothing executes that path.

## Counts

| Class | Files | What the class means here |
| --- | ---: | --- |
| LIVE-CI | 0 | A workflow step, or a script a workflow runs, executes or reads the file. |
| LIVE-XTASK | 1 | Rust in this repo reads or runs the file. The one hit is an example, not an `xtask` subcommand and not a `#[test]`. |
| LIVE-OTHER | 149 | A current `tests/` or `bootstrap/` command, or a shell script that command runs, executes the file. Imports from that execution count. |
| UNCERTAIN | 233 | A current document names the file, and no workflow, Rust path, or live shell executes the current path. |
| DEAD | 48 | No other tracked file names the current path, except a comment or another file in this dead set. |

`LIVE-CI` is empty, so it has no rows. The 149 live shell and document commands are manual gates. `tools/docs/testing.md:430` says the bootstrap gates are `sh` scripts and self-skip when `python3` is absent. `tools/docs/testing.md:438` is `sh tests/bootstrap/alpha-beta-edge.sh --edge`. No `mbx xtask` subcommand runs any file in this census. `mbx xtask sample-gate` checks a sample's `// Expected exit:` / `// Expected output contains:` header (`tools/xtask/src/samples.rs:516`, `tools/docs/testing.md:347`). It does not read these `.py` files.

A file landed in `LIVE-OTHER` only when the chain was an execution: a `python3` line, a `gate.sh` decoder argument, a heredoc import, or an import from one of those. A sentence that merely names a path, with no command, stayed `UNCERTAIN`. Sample READMEs were not treated as the repository's operating procedure, because `samples/README.md` and `mbx xtask sample-gate` are the sample check.

## What the live files do

| Group | Files | Lines | Role | Evidence (one chain for the group) | `mbx xtask` today |
| --- | ---: | ---: | --- | --- | --- |
| `gamma-proof-gates` | 79 | 6,658 | Fixture vectors and diagnostic comparisons for the Gamma proof and Beta-encoding gates. `run.sh` materializes the bound evaluator and runs `gate.py` or a named sibling; those modules import the rest. | `tests/gamma/derivation-checking/run.sh:21` | none |
| `delta-boundary-gates` | 22 | 1,485 | Delta boundary fixture rows. The `run.sh` heredoc imports them under `PYTHONPATH=$GATE_DIR`, or runs `gate.py`. | `tests/delta/frontend-boundary/run.sh:16` | none |
| `omega-customer-gates` | 10 | 1,511 | D customer gates (parser, outcome, request, executable), the source-closure check, and the parser measurement scripts. | `tests/bootstrap/omega-parser/run.sh:38` | none |
| `epsilon-evaluator-gates` | 10 | 3,879 | Epsilon checking, storage, refinement, and composition drivers. `model.py` alone is 2,282 lines. | `tests/epsilon/refinement/run.sh:30` | none |
| `product-psi-decoders` | 10 | 13,308 | After `tests/product/gate.sh` builds a native program, the Python file checks the artifact. The four Psi decoders are 12,085 of these lines. | `tests/psi/parser/test-parser.sh:5` then `tests/product/gate.sh:75` | none |
| `bootstrap-accelerators` | 9 | 2,030 | Host accelerators and successor-construction checks under `bootstrap/compiled_vm/` and `bootstrap/independent/`. | `bootstrap/compiled_vm/README.md:48` and `:627`; flag-only loads below | none |
| `alpha-seed-gates` | 4 | 871 | Independent Alpha VM (`alpha_ref.py`), bounds cases, native-exit normalization, and container structure. `alpha-beta-edge.sh` runs the container, conformance, and root-audit legs. | `tests/bootstrap/alpha-beta-edge.sh:163` runs `tests/alpha/container.sh:75` | none |
| `beta-assembler-gates` | 3 | 334 | Independent Beta assembler (`beta_ref.py`), finite root audit, and the word-prefix grammar. | `tests/bootstrap/alpha-beta-edge.sh:193` and `tests/beta/compiler/compiler-diamond.sh:39` | none |
| `construction-input` | 1 | 147 | `source/omega/construct.py` packs the construction application's real source closure and can feed that closure to a constructor. | `source/omega/README.md:35` | none |
| `citation-claim-checks` | 1 | 89 | Offline citation check for the typesafe-experiments skill. | `.agents/skills/typesafe-experiments/SKILL.md:143` names it; `:148` says to run it with Python 3 | none |
| `rust-example-git-proxy` | 1 | 34 | `executed_git_source` copies this file onto `PATH` as `git` so the example can watch a local repository. Cargo examples are not tests and are not in the workflows above. | `omega-rust/omega/compilation/compiler/examples/executed_git_source.rs:110` | none |

Two accelerator modules load only behind flags. `successor_test.py:110` loads `literal_controls.py` when the command is `successor_test.py --literals` (`bootstrap/independent/README.md:204`). `successor_test.py:126` loads `compile_test.py` when the command is `successor_test.py --compile-epsilon --literals --omega` (`bootstrap/independent/README.md:236`); `compile_test.py:16` and `:250` then load `composition.py` and `enum_test.py`. The `python3 ... --omega-utf8` line at `bootstrap/compiled_vm/README.md:627` does not pass those flags.

At the census commit the shell gates also called the Python helpers under `tools/bootstrap/`. Those files are outside this census. Replacing the 149 files here left that `tools/` Python in place; the bootstrap packing, forge, and invoke helpers are now `mbx xtask` commands.

## Uncertain

| Group | Files | Why it is not live, and why it is not dead |
| --- | ---: | --- |
| `sample-readme-oracle` | 230 | A sample README names the file, or a file that README names imports it. 72 of the citing lines still say `samples/cli/independent/...`, a directory that is not in the tree; the files now live under `samples/cli/scientific/` (and a few under `probes/`, `facts/`, `audio/`, `gui/`). `mbx xtask sample-gate` does not read them. They are reference implementations, frozen-output checkers, and small helpers (79 `reference.py`, 75 `validate.py`, and the rest oracles, freeze scripts, and terminal checkers). |
| `delta-pair-study` | 1 | `tests/delta/resource-boundary/README.md:309` says `pair_study.py` runs an instrumented Alpha reference over the Delta compiler. No `python3` line and no `run.sh` import executes it. The resource-boundary gate's heredoc does not import it. |
| `encoding-gamma-emitter` | 1 | `bootstrap/proofs/beta_encoding/README.md:258` says `emit_gamma.py` generated `definitions/encoding/` from `encoding.py`. No current command runs it. `encoding.py` itself is in `gamma-proof-gates` because the theory gate imports it. |
| `stale-callback-prepare` | 1 | The fixture README says the corpus does not run `fixture/prepare.py` (`tests/fixtures/pass/layouts/private_callback_inherited_policy_exit/README.md:44`). The command on `:50` sets `fixture=tests/omega/pass/layouts/private_callback_inherited_policy_exit`. That is the same deleted path `package_review.rs:789` uses. |

## Dead

| Group | Files | Lines | Evidence |
| --- | ---: | ---: | --- |
| `unreferenced-sample-helper` | 45 | 2,230 | No citing line in any tracked file. TSV evidence is `no-reference`. |
| `dead-sibling` | 2 | 176 | `samples/cli/scientific/clearance_route_planner/validate.py:32` and `:36` name `validation/build_runtime.py` and `reference.py`. `validate.py` is itself in the unreferenced set. |
| `comment-only` | 1 | 193 | `tests/gamma/beta-encoding-theory/encoder.py:574` mentions `python3 worksim.py` in a comment. Nothing imports or runs `worksim.py`. |

All 48 dead files are under `samples/`, except `worksim.py`.

## What to delete first

The safe deletion set is the dead groups: **48 files, 2,599 lines**, all sample helpers plus `worksim.py`. Nothing live names them.

The next decision, not a silent delete, is the **230 sample README oracles** (16,495 lines). They are the scientific and probe checkers (`reference.py`, `validate.py`, and their helpers). The live sample check is `mbx xtask sample-gate`, which uses the program header. Deleting the Python drops the frozen-output and invalid-input checks those READMEs describe. Keeping them means porting each checker into an `xtask` command, and retargeting the READMEs that still say `samples/cli/independent/`.

Do not delete the 149 `LIVE-OTHER` files, `construct.py`, or `git_source_proxy.py` until a Rust command covers that role. There is no `mbx xtask` command for any of them today.

`emit_gamma.py` is the recorded generator of the checked-in Beta-encoding definitions. `pair_study.py` is the recorded allocation study. `fixture/prepare.py` under the callback fixture is the C provider builder whose command still points at `tests/omega/`. Those three are the uncertain files outside `samples/`.

## What a Rust port of the live files takes

No `mbx xtask` subcommand covers these roles. A port is new Rust under `tools/xtask` (or a Rust test, for the example), not a new Python entry point.

The small end is the Alpha and Beta references (7 files, 1,205 lines) and `construct.py` (147 lines). `alpha_ref.py` and `beta_ref.py` are independent interpreters the shell compares with the audited seed and the Beta compiler. `construct.py` packs one source closure and optionally runs a constructor. Each of those is one `xtask` subcommand with the same inputs and the same byte comparison. The shell around them (`tests/bootstrap/alpha-beta-edge.sh` and the identity scripts) also calls Python under `tools/bootstrap/`, which this census does not include.

The Gamma, Delta, and Epsilon gates (111 files, about 12,000 lines) are fixture tables and expected diagnostics. The shell materializes tapes; the Python decides the cases and the verdict. Porting one gate means moving that gate's vectors and comparison into Rust, then pointing `run.sh` at the Rust command or deleting `run.sh` once `xtask` owns the materialization too. `tests/epsilon/refinement/model.py` (2,282 lines) is the large single file in that set.

The product and Psi decoders (10 files, 13,308 lines) are the largest live port. `gate.sh` already builds with the `omega` CLI. The Python then parses the native program and the expected cases. Four files, `tests/psi/{parser,checking,resolver,typing}/test_*.py`, are 12,085 lines of that. The Rust replacement is a test or `xtask` command that builds the same package and checks the same artifact bytes.

The nine bootstrap accelerator files are host VMs and successor-construction drivers (`gamma.py`, `test.py`, `source_test.py`, `successor_test.py`, and the four Epsilon controls). Replacing them is a bootstrap construction change under `bootstrap/MINIMIZATION.md`, not a line-by-line move of the fixture gates. `claim_checks.py` is 89 lines and is only the skill's offline citation check. `git_source_proxy.py` is 34 lines; the example can shell out to a small Rust binary instead of a Python `git`.

Porting the 230 sample oracles is a separate decision. `sample-gate` already runs the programs. A Rust port would reimplement each frozen-output checker the README still describes. That is 16,495 lines of per-sample logic, and it is not required to keep `mbx xtask sample-gate` green.

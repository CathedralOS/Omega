# Bootstrap lattice — active work

Last pruned: 2026-08-28.

This queue closes one direct compiler sequence. It is organized by produced
artifact, not by historical scripts, validation experiments, or intermediate
repository buckets.

## Fixed model

Let `C` be the exact production compiler source closure under `source/omega/`.
It is ordinary Omega deliberately authored with only the language surface
needed to express a robust compiler. That restriction is a property of `C`, not
a new language, dialect, compiler, or source owner.

```text
audited Alpha VM seed
    → Alpha assembler + Alpha-written Beta cold start → bc
    → bc builds the canonical Gamma evaluator and type checker
    → Gamma evaluates the declared Delta meaning route → delta
    → delta compiles C → omega₀
    → omega₀ compiles the same C → omega
```

`omega₀` and `omega` implement the same full Omega language from the same
source. `omega₀` may lower conservatively; `omega` may use the optimizer and
advanced backend already implemented in `C`. Difficult Omega features such as
the mathematical proof surface and linear dependent types may be absent from
the source of `C` even though the compiler produced from `C` accepts them.

The artifact chain is the bootstrap. There is no `omega-bootstrap` compiler,
Omega subset language, checkpoint compiler generation, DDC stage, or source
directory for either `omega₀` or `omega`. Shell and Python may invoke exact
commands, manage temporary files, and run negative tests. They may not supply
source discovery, parsing, resolution, lowering, evidence construction, or any
other semantic stage.

## Repository ownership

```text
source/alpha/assembler/             Alpha source-to-tape construction
source/alpha/checker/               separate derivation-checker artifact
source/beta/compiler/               bc source, artifact, cold start, admission
source/gamma/                       canonical evaluator and type checker
source/delta/compiler/              delta source, artifact, adjacent admission
source/delta/meaning/               canonical Delta-to-Gamma meaning route
source/omega/                       the one product compiler source closure C
source/omega/psi/                   target-neutral phases inside C
source/omega-rust/                  optional implementation/comparator
tests/lattice/                      shared cross-rung inputs
tools/lattice/                      replaceable command ordering
```

The Alpha checker is a separate binary from the Alpha VM and assembler. It is a
trust-floor service used beside compiler edges, not the compiler that builds
Beta and not another rung. Gamma likewise has no required compiler binary: `bc`
builds the Beta-written Gamma evaluator and type checker, and those programs
provide the canonical route used to realize Delta.

The artifact being admitted owns its validation. Beta admission therefore
lives under `source/beta/compiler/validation/`; Delta publication and executable
custody live under `source/delta/compiler/validation/`. Do not recreate generic
`bootstrap/`, `canaries/`, `assurance/`, `refinement/`, `on-ramp/`,
`proof-kernel/`, or repository-level `psi/` owners.

Product compiler implementation belongs to **OMEGA-PRODUCT-COMPILER-SOURCE** in
[`TASKS.md`](TASKS.md), not here. This queue consumes the exact closure that task
publishes; it does not maintain a second Psi/Omega implementation backlog.

## Edge status

| Producer edge | Current state | Remaining result |
| --- | --- | --- |
| Alpha seed and cold start → `bc` | exact source and tape exist; Alpha-rooted construction and bounded admission gates run | make the admission implementation small enough to audit and change locally |
| `bc` → Gamma evaluator/type checker | canonical Beta-written programs and bounded gates exist | preserve this route; do not invent a Gamma compiler artifact |
| Gamma meaning route → `delta` | source closure, repeated-output publication checks, target-dialect validation, and artifact identity custody exist | complete canonical execution, executable reconstruction, and checked publication |
| `delta + C` → `omega₀` | source owners fixed; compiler and final closure incomplete | accept the exact ordinary-Omega surface used by complete `C` and check the first build |
| `omega₀ + C` → `omega` | model fixed | rebuild unchanged `C` and check the second edge independently |

The Rust compiler under `source/omega-rust/` remains a useful differential
producer while maintained. Its agreement, availability, or pedigree is never a
bootstrap or release condition.

## 1. Alpha seed and cold start → `bc`

The accepted Beta compiler is `source/beta/compiler/bc.beta` and
`source/beta/compiler/artifacts/bc.tape`. Its cold construction, artifact
admission, and optional stress evidence stay under the same compiler owner.

- [ ] Collapse the remaining Beta admission explosion into one canonical exact
  instruction/event/memory identity format plus small responsibility-specific
  semantic modules. The current bounded admission consists of 190 Alpha modules
  and 60,937 lines; Checker A is 1,011,122 source bytes and the checker ROOT is
  80,986 bytes. Shape, control, data, and
  publication modules must consume common decoded facts rather than repeat byte
  offsets, macro bodies, or equivalent verification permutations. The first
  procedure-custody tranche centralizes the canonical parameter/slot/frame
  identity join for 52 consumers, removing 359 duplicated lines and 9,100
  source bytes while retaining the existing nth/cardinality checks and all
  eight binding teeth. The next tranche replaces five family-private witness-PC
  ingestion loops with one bounded exact-table owner, removing another 193
  lines and 3,482 bytes while leaving each family parser and semantic validator
  independent. A third tranche makes the whole-artifact frame summary's retained
  per-PC state canonical for ranged-store transfer, deleting that consumer's
  weaker duplicate fixed-point engine (474 lines and 12,231 bytes) while
  preserving its separate operand/value theorem and the exact ROOT observation.
  One live-count-derived procedure-span inventory now proves the total ordered
  71-procedure/359-block partition over the already checked block tables.
  Procedure-entry and inclusive block-range queries consume it in constant time
  without adding a witness or semantic token. Next remove census-only block
  listings family by family while retaining every PC-producing identity call.
  The first twenty low-risk consumers now rely on that owner, deleting 83
  unused block lookups, 23 redundant span scans, thirteen dead helper routines,
  and 458 source lines without changing the exact artifact subject. Four effect
  censuses and two fixed-emitter summaries also consume their validated live
  table counts instead of stale 242/95/291/355/613-row ceilings.
  Keep frame, effect, memory, stack, ranged-store, and meaning theorems separate
  rather than turning the inventory into another mega-checker.
- [x] Finish identity localization before changing the shared compiler frame
  macros. Procedure, block, transition, event, local, primitive, push,
  continuation, epilogue, and shared macro identities are centralized. The r13
  word-size optimization still shifts many semantic consumers. The checked
  stable-row memory resolver has landed with the complete `gen_stmts` and
  `gen_expr` memory families plus a same-block swapped-PC tooth. The final 14
  consumer modules now resolve all memory sites through checked identities and
  semantic load/store rejoins; no semantic consumer calls the coordinate-taking
  memory adapter.
  Synthetic `__write_str`, the sole missing shared owner found by the
  internal-site audit, now resolves from the checked prelude successor; effect
  custody owns its one exhaustive body check and the duplicate summary scan is
  gone. All shared owners therefore exist. The direct identity/table group and
  the `bc-cursor-leaf-summary`/`bc-skip-ws-summary`/`bc-slurp-summary`
  procedure subgroup now consume existing owners, removing 290 raw artifact-PC
  literals. The next memory, procedure/epilogue, and emit-layout tranches are
  also localized, including complete `gen_to` shape/rules and the remaining
  source-row-bound fixed emits. The earlier nine-module internal-site tranche is
  closed. The memory-consumer closure localized `cursor`, `label`, `slurp`,
  parse/data, ranged-store/resource, root-observation, and statement-label
  families while deleting 352 net lines. The broad compatibility-API census is
  classified. After the compact expression-rule, declaration/expect,
  summary/statement, main-ready/root-prelude, parse/control, classifier,
  fixed-keyword, main-bridge, expression-shape, classifier, bounded-emitter,
  emit-cmp, string-body, statement-boundary, literal-skip, and whitespace
  tranches, the only compatibility callers are five retained by design: the
  three low-level identity owners and the identity-derived fixed-emitter/gen-emit
  checks. No literal semantic consumer remains. Source rows `259`
  and `391` are not artifact PCs. `gen_emit`'s three identical newline events
  use exact-cardinality occurrence identity; an eighth tooth swaps two witness
  PCs and rejects before r13.
- [x] Apply the r13 optimization only after that localization. Acceptance is a
  change to `bc.beta`, centralized identity/shape/ABI owners, generated exact
  identities, and adjacent manifests—not mechanical edits across unrelated
  semantic modules. Preserve the cold-start fixed point and both exact-subject
  admission gates. Completed: the root prelude reserves `r13=8`; shared push,
  pop, prologue, epilogue, comparison, and combination emitters consume it;
  global custody rejects every other candidate write to `r13`. The persisted
  artifact fell from 52,141 to 40,693 bytes (21.96%) while the fixed point,
  corpus, structural checker, full exact observation, and all eight binding
  teeth remain green.
- [ ] **BLOCKED — OWNER Q18:** ratify the generic guarded
  simulation/coinduction judgment and finite certificate shape, then reconstruct
  the exact compiler proposition below `bc` and check it with the Alpha-owned
  derivation checker. Candidate compiler output must never select its own
  proposition or accept its own evidence.
- [x] Keep alternate checkers, fuzzing, large corpora, exhaustive mutations,
  generated refinement samples, and developer reports optional. The default
  edge builds the artifact and runs only the bounded gates required to admit
  that exact artifact. The lattice runner has exactly three Beta rows: cold
  construction, structural framing, and exact maximal-observation
  reconstruction. Path-policy tests pin that allowlist so optional evidence
  cannot silently enter the default edge.

Acceptance: changing one shared compiler macro changes `bc.beta`, its one shape
owner, generated identities, and directly relevant semantic obligations only.
No cached viewer, receipt matrix, source-row permutation suite, or debug output
is required by the edge.

## 2. `bc` → canonical Gamma meaning

`source/gamma/interp.beta` and `source/gamma/typeck.beta` are the canonical
Gamma programs built by `bc`. Gamma supplies safe definitional evaluation; it
does not contribute a separately published native compiler between Beta and
Delta.

- [ ] Keep the full compiler-sized evaluation bounded and practical without
  changing Alpha or Gamma meaning, hiding a semantic stage in a runner, or
  weakening exact evidence joins. A 12-hour ceiling is emergency containment,
  not an acceptable normal gate duration.
- [x] After the admitted r13 change, profile the exact Delta publication input
  again before attempting another dispatch mechanism or speculative Gamma
  rewrite. Preparation is already sub-second-to-low-second work; prior sampling
  placed roughly 90% of the canonical execution in Alpha instruction dispatch.
  The admitted artifact reproduces the measured prototype interpreter tape
  exactly. Two interleaved three-run representative measurements improved by
  10.64% and 10.83%. A fresh eight-second sample of the exact 2,150,135-byte
  closed Delta publication program recorded 5,113 of 6,207 samples at Alpha's
  `next` dispatcher, with nearly all remaining samples in Alpha opcode handlers
  and no output yet. Dispatch remains the next optimization target; preparation
  and packing completed in 1.15 seconds and 0.11 seconds respectively. The first
  dispatch simplification now places the hottest `imm` handler immediately
  before `next`, eliminating one native branch per immediate without changing
  the dispatch mechanism. An interleaved representative Gamma loop improved
  from 3.085 to 2.965 seconds (3.89%) and retired about 1.19% fewer native
  instructions with byte-identical output. A virtual-PC profile of the exact
  closed Delta input then localized the next avoidable cost: every internal
  expression evaluation repeated a positive-fuel check even though only
  function-call transfer decrements fuel. The canonical evaluator now checks
  its outer entry and every decrement boundary once, then evaluates
  subexpressions through a positive-fuel core while retaining the arena check
  on every entry. A second interleaved representative run improved from 2.955
  to 2.840 seconds (3.89%) and retired 3.52% fewer native instructions. On the
  exact closed input, the diagnostic virtual interpreter advanced through
  15,989,175 rather than 15,094,364 evaluator calls in the same 15 seconds
  (5.93% more semantic progress). Finally, variable expressions now perform a
  cached-slot hot read directly in the evaluator after their first complete
  frame-local lookup. The representative loop retired 2.32% fewer native
  instructions (57,187,216,060 versus 58,542,718,558), and the exact-input
  diagnostic cost fell from about 550.87 to 540.75 Alpha steps per evaluator
  call (1.84%). Direct canonical-u32 encode/decode paths then removed the hot
  `mk_int`/`get_int` calls while retaining boxed and constructor fallbacks. Six
  interleaved million-tail pairs improved from 1.790 to 1.583 seconds (11.55%),
  all pairs won, and a fixed 15-second exact-input profile completed 7.4% more
  evaluator work with 18.1% fewer helper calls. Explicit tests pin zero, one,
  both u32 edges, boxed negative/overflow arithmetic, comparisons, and both
  condition branches. This does not substitute for the required exact execution.
- [x] Retain the canonical evaluator input and output explicitly at the Delta
  producer edge, and retain evaluator/type-checker source and build-artifact
  identities at the `bc` → Gamma edge. The Delta publication evaluates an
  already elaborated closed Gamma program; inserting a separate type-checker
  execution there would invent another semantic stage. Optional Python and
  alternate Gamma implementations remain differential evidence only.

## 3. Gamma meaning route → `delta`

The canonical source is `source/delta/compiler/main.alp`; its independently
declared lower-rung meaning route is implemented under `source/delta/meaning/`.
The publication verifier already binds the source closure and tools,
reconstructs the packed Gamma program, compares repeated assembly observations,
and validates the bounded Darwin ARM64 target dialect. No compiler artifact has
yet been published.

The 2026-08-28 exact attempt was stopped after both parallel executions reached
9,660 seconds with no output. It produced no receipt and grants no authority.
That attempt predates the admitted Alpha dispatch reorder, hot-operand decode,
and immediate decode changes, so it is not a current runtime estimate. Fresh
preparation, elaboration, and packing take about 1.9, 1.25, and 0.11 seconds;
the two already-parallel interpreter executions are the only material wait.

- [ ] Execute the exact canonical Delta compiler source through the accepted
  Gamma route on the required V1 host, Darwin ARM64, using the four literal
  elaboration, packing, and repeated-execution commands. Retain the exact
  repeated assembly observation and receipt. A bounded smoke execution is not a
  substitute.
- [x] Complete realization replay in
  `source/delta/compiler/validation/`. `generate` and `verify` now run the exact
  supplied absolute tool paths and literal command profile against the captured
  assembly, validate the fresh temporary Mach-O, require empty diagnostics and
  byte equality with the candidate, and re-snapshot all inputs afterward.
  Handcrafted Mach-O fixtures can test the container validator but cannot mint
  reconstruction-bearing receipts.
- [x] Bind source identity, target identity, assembly identity, replayed
  executable identity, reconstruction obligations, and disclosed target/host
  admissions in the eventual publication receipt. Preserve `OPEN_REFINEMENT`
  until the independently selected source-to-artifact proposition is checked.
- [ ] **BLOCKED — OWNER Q16:** ratify the independent Delta v1 language,
  resource, exhaustion, and observation semantics, then bind that semantic
  subject into lower-rooted source-to-artifact refinement. The existing
  translator and corpus cannot select their own contract.
- [ ] Install the admitted result only under
  `source/delta/compiler/artifacts/`, with receipts rooted in the exact canonical
  execution and realization replay.

Exact execution, realization replay, strict target validation, frontend work,
and performance work are engineering tasks and are not blocked on Q16. Add
another host only when a separately declared publication profile requires it.

## 4. `delta + C` → `omega₀`

The Delta compiler needs to accept only the compositional ordinary-Omega forms
actually used by the complete compiler closure `C`. Accepted forms retain
ordinary Omega meaning; unsupported forms reject. This is not a Delta=Omega
claim and does not define a named Omega subset.

- [ ] Consume the deterministic transitive compiler manifest published by
  **OMEGA-PRODUCT-COMPILER-SOURCE**. Do not maintain a bootstrap-private source
  list, file allowlist, AST profile, feature list, or checkpoint tree.
- [ ] **BLOCKED — OWNER Q8:** settle requested-target versus source-selected
  target semantics, finalize the durable product build entry, and bind the
  package-resolved manifest for `C`.
- [ ] Once the complete compiler builds, derive the exact ordinary-Omega surface
  used by its resolved closure. Implement that surface in the Delta compiler
  with checked semantics, conservative lowering, target realization, explicit
  resource ceilings, and deterministic rejection outside the supported set.
- [ ] Keep generated/compile-time source, package acceptance, build inputs,
  imported tools, target selection, and emitted-artifact custody explicit in
  the closure. Omit interpreters, REPLs, viewers, proof explorers, debuggers, and
  other tools not imported by the compiler executable.
- [ ] Run `delta C → omega₀`, reconstruct and check the exact source/artifact
  refinement edge, retain all target dependencies and admissions, and run the
  compiler acceptance suite with `omega₀`.

Acceptance: the first Omega build is one direct Delta compiler invocation over
the product-owned closure. No shell/Python translation, private IR generation,
or second source tree participates.

## 5. `omega₀ + C` → `omega`

- [ ] Run `omega₀ C → omega` without modifying, regenerating, translating, or
  selectively replacing any part of `C`.
- [ ] Reconstruct and check the second source/artifact edge independently of the
  first.
- [ ] Demonstrate that the conservatively and production-lowered artifacts
  implement the same pinned source meaning.
- [ ] Treat binary equality and Rust agreement as reproducibility or diagnostic
  evidence only. Correctness comes from the checked edges and explicit
  admissions.

## Non-authoritative orchestration

Every required producer, checker, and gate command must remain directly
invocable. `tools/lattice/` may provide one short convenience sequence whose
failures print the exact command to rerun. No bootstrap claim may depend on that
runner, its working directory, or a particular shell.

Path ownership is enforced by [`tools/lattice/test-paths.sh`](tools/lattice/test-paths.sh)
and [`tools/lattice/check-path-hygiene.sh`](tools/lattice/check-path-hygiene.sh).
Remove obsolete aliases, cached profiles, historical bridge formats, and
validation output when no current compiler edge consumes them.

## External contract dependencies

The first authoritative product build also requires the package/security owner
to publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows remain review data rather than acceptance
authority. This blocks final publication, not implementation of the direct
compiler sequence.

Track product compiler implementation in [`TASKS.md`](TASKS.md) and package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

# Bootstrap lattice — active work

Last pruned: 2026-08-28.

This queue builds one direct compiler sequence. It does not define a second
orchestration language, a separately named hosted bridge, or a collection of
checkpoint compilers.

## Fixed model

Let `C` be the exact production compiler source closure. `C` is ordinary Omega
written deliberately against only the language surface needed to express a
robust compiler. That restriction is a property of this source closure, not a
new language, dialect, compiler, or repository owner.

```text
audited Alpha seed
    → run Alpha assembler + Alpha-written Beta cold start → bc
    → run bc over the Beta-written Gamma evaluator/type checker
    → evaluate the Delta compiler through canonical Gamma meaning → delta
    → run delta over C → omega₀
    → run omega₀ over the same C → omega
```

`omega₀` and `omega` implement the same language from the same source. The first
binary may be conservatively lowered; the second may use the optimizer and
advanced backend already present in `C`. The source may avoid difficult Omega
features such as the mathematical proof surface or linear dependent types even
though the compiler it implements accepts them.

Every arrow must be directly invocable as a compiler operation over exact input
bytes with exact output bytes. Shell and Python programs may order commands,
prepare temporary files, or run negative tests, but they are replaceable
conveniences. No script may silently supply parsing, resolution, lowering,
evidence construction, source discovery, or another semantic stage.

The retired `source/on-ramp/omega-bootstrap` implementation, its private checked
IR generations, and its mirrored refinement snapshots are not bootstrap inputs.
Git history is their archive.

The artifact chain, rather than any directory called “bootstrap,” is the
bootstrap. The durable source owners are therefore the language directories;
`tools/lattice/` is optional command ordering only.

## Standing invariants

- Alpha, Beta, Gamma, Delta, and Omega are the only language capabilities in
  the chain.
- The final Delta-produced compiler accepts `C` directly. There is no separate
  `omega-bootstrap` compiler between Delta and `omega₀`.
- `C` uses an ordinary-Omega subset by authoring discipline: no private syntax,
  altered meaning, file allowlist, AST-permutation matching, or separately
  versioned profile.
- The compiler source is authored once. `delta C → omega₀` and
  `omega₀ C → omega` consume the same closure.
- The Rust producer at `source/omega-rust` is a development implementation and
  differential comparator. It is not a bootstrap or release dependency.
- Authority comes from exact source, pinned semantics, checked obligations,
  disclosed admissions, and source-to-artifact refinement—not compiler
  pedigree, fixed points, or agreement with Rust.
- The universal proof checker belongs to Alpha and is not another language rung.
- The artifact being admitted owns its validation. Validation may consume the
  Alpha checker, but candidate compiler output never accepts its own evidence.
- Standalone interpreters, viewers, REPLs, and debugging tools remain outside
  `C` unless the compiler executable imports them.

## Current state

| Owner | Present | Required closure |
| --- | --- | --- |
| Alpha | audited seed, VM, assembler, below-Beta checker artifact, implementations, and gates | preserve the explicit checker reconstruction and disclosed machine/host admissions |
| Beta | compiler source/artifact under `source/beta/compiler`, Alpha-rooted cold start, self-host tests, and adjacent validation | consolidate the exact source/artifact admission into one comprehensible validator |
| Gamma | Beta-written canonical interpreter and type checker; an alternate Gamma-hosted checker remains owned by Alpha | retain bounded canonical execution; do not invent a Gamma compiler artifact where canonical evaluation is the actual edge |
| Delta | compiler corpus, lower-rung meaning under `source/delta/meaning`, source-closure and publication checks | publish the exact Delta-produced compiler from Gamma and extend it to accept all of `C` |
| Omega source | one permanent product tree under `source/omega`, with target-neutral phases in `source/omega/psi` | consume the completed compiler closure published by `TASKS.md` and freeze the exact surface actually used by `C` |
| Rust comparator | working implementation under `source/omega-rust` | remain optional and non-authoritative |

## Artifact-edge queue

The headings below follow the actual producer edges. A validation task lives
with the artifact it admits; reusable language tests and developer diagnostics
do not become additional lattice steps.

### 0. Close and simplify the trust floor

- [x] Put the root checker under `source/alpha/checker/`.
- [x] Put the Beta compiler source, artifact, cold start, and admission evidence
  together under `source/beta/compiler/`.
- [x] Remove the generic `source/refinement/` owner. Future validation belongs
  beside the compiler or artifact being admitted.
- [x] Replace the circular `bc → check.beta → admit bc` story with a checker
  whose accepted artifact is constructed and audited below the Beta compiler it
  validates. Alternate Beta, Gamma, Rust, or Python checkers remain differential
  evidence, not acceptance authority.
- [x] Put every retained Beta admission module under its actual consumer and
  remove the cached viewers, duplicated generated programs, receipt matrices,
  and default stress/permutation paths that had no authoritative consumer. The
  bounded whole-compiler command now consumes all 188 retained `bc-*.alpha`
  modules from `validation/admission/obligations/`; the two exact-subject gates
  live in `admission/`, untrusted witness producers in
  `admission/witnesses/`, and optional generated refinement in `stress/`.
- [ ] Collapse the remaining Beta validator obligation explosion into one
  canonical data format and small responsibility-specific modules. The current
  188 fragments total 64,605 lines after replacing nine private instruction
  scans with shared exact effect-census logic and merging the duplicate
  selected-row decoders into the canonical exact-table helper. Push, pop,
  saved-frame prologue, optional frame allocation, parameter store, and
  epilogue bytes now have one responsibility-neutral exact decoder consumed by
  frame, effect, memory, expression, and stack-table checks; this reduced the
  ROOT tape from 79,124 to 77,889 bytes without changing either admitted
  subject. A shared fail-closed resolver now maps independently reconstructed
  source procedure IDs to unique checked entry-block PCs. Expression and the
  remaining bounded-emitter, label, statement, procedure, and root-observation
  consumers now resolve all 79 selected uses; the seven migrated entry values
  have no remaining absolute immediate uses. Reusable fail-closed block,
  transition, event, local-access, primitive, push, call-continuation, and
  epilogue resolvers now join source-owned identities to checked artifact PCs;
  `emit_dec` is the first complete consumer and no longer embeds its 32 raw PC
  occurrences. Fixed decimal/prelude emitters removed another 56 checked
  coordinates, and `gen_stmt` now derives all 126 of its former artifact
  coordinates from source identities and relative macro shape. These two
  tranches were net-negative by 126 lines. A targeted fifth negative control
  proves a mutated witness event PC cannot redefine that identity. The current
  ROOT is 79,565 bytes. Continue replacing
  shape/control/data/publication permutations with data decoded by common
  checks; do not recreate cached viewers or debug-only publication paths. In
  particular, finish one canonical exact instruction table that gives stable
  procedure/block/event identities to artifact PCs. Semantic modules must
  consume those decoded identities and shape facts rather than embedding copied
  byte offsets or instruction sequences. Acceptance includes changing one
  shared compiler macro without mechanically editing unrelated semantic
  fragments; the artifact-aware decoder and the one relevant shape owner should
  expose the change to all consumers.
- [x] Keep fuzzing, alternate checkers, large corpora, and exhaustive mutation
  campaigns as optional stress suites. The default lattice path must build each
  compiler and run only the bounded admission gates required for that edge.
- [x] State every admitted artifact, checker, exact input, exact output, and
  remaining assumption in one short chain manifest that can be audited without
  reading orchestration scripts.

### 1. Publish the Delta-produced compiler from below

Current lower-rung progress: the publication verifier binds the canonical
source closure and tools, reconstructs the packed Gamma program, independently
decodes repeated executions, requires byte-identical assembly, and validates
the bounded target dialect. It remains fail-closed until those observations
come from the exact full-source execution; it does not manufacture the missing
compiler artifact. Stage completion and retained-marker replay now apply the
same explicit template, closed-Gamma, raw-observation, and diagnostic ceilings
as final verification, reject size drift, and never retain a marker after
resource rejection. Attempt-plan and stage-start/replay identity reads likewise
apply every already-authoritative Alpha artifact/tape, source-image, template,
closed-Gamma, and document ceiling with bounded drift-checked reads; source
classes without an independently declared ceiling remain explicitly unbounded
rather than acquiring an orchestration-invented limit. The 2026-08-28 exact
attempt was stopped after both parallel
executions reached 9,660 seconds with zero output; it produced no receipt and
grants no publication authority.

Design blocker: [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md) Q16 must ratify the
independent Delta v1 language, resource, and observation semantics before the
canonical compiler can receive checked source-to-artifact authority. The
current Beta-written Delta-to-Gamma translator is an implementation of the
proposed meaning route, not the authority that selects its own semantic
subject. Exact executions, assembly publication, strict target validation,
executable reconstruction, and frontend implementation remain engineering work
and are not blocked on this ruling.

- [ ] Execute the exact canonical Delta compiler source through the accepted
  Gamma route on the current required V1 host, Darwin ARM64, using the four
  literal elaboration, packing, and repeated-execution commands, and retain the
  resulting assembly receipt. Bounded direct-command coverage does not
  substitute for those canonical observations. Add another host only when a
  separately declared publication profile requires it.
- [ ] Bind source identity, target identity, emitted artifact identity, and the
  exact reconstruction custody needed by the eventual refinement edge. The
  existing Darwin ARM64 artifact-custody verifier now requires the exact Mach-O
  header policy, a closed load-command vocabulary, bounded dyld/symbol/link-edit
  ranges, ordered nonoverlapping segments, terminal `__LINKEDIT`, and the exact
  dynamic dependency closure. Section custody is also closed over the reviewed
  compiler/linker vocabulary with exact flags, zero final-image relocations,
  ARM64 stub/pointer metadata, nonoverlap, and separation from load commands.
  It still has no canonical-execution artifact to bind and deliberately grants
  no source-refinement authority.
- [x] Once no exact publication attempt is live, move Delta compiler admission
  documentation, receipts, drivers, and tests from the flat `source/delta/`
  root into `source/delta/compiler/validation/`. Keep the semantic elaboration
  under `source/delta/meaning/`; do not create a generic assurance owner.
- [ ] **BLOCKED — OWNER Q16:** bind the independently ratified Delta
  semantics/resources and complete direct lower-rooted source-to-artifact
  refinement.
- [x] Remove the unconsumed `source/delta/build/delta{0,1,2}.exe` residue. None
  was referenced by a gate or included in the canonical source closure.
- [ ] Install the published compiler under `source/delta/compiler/artifacts/`
  with receipts rooted in that exact execution.
- [x] Keep [`source/delta/compiler/validation/lower-rooted-assembly-publication-v1.sh`](source/delta/compiler/validation/lower-rooted-assembly-publication-v1.sh)
  a verifier for already observed data, not a hidden producer. Its only command
  calls `verify` over sixteen caller-supplied evidence paths; it neither invokes
  a lower-rung executable nor writes an observation or receipt. The focused
  suite covers exact verification plus missing, malformed, oversized,
  disagreeing, and mutated evidence.
- [x] Expose every constituent publication command directly without
  `tools/lattice/verify-lattice.sh` or a private driver action. The prepared
  attempt now contains four literal scripts for Delta elaboration, input
  packing, and the two Gamma executions. The focused gate runs the real
  elaboration and packing commands plus one bounded real Gamma execution;
  marker custody observes those commands but does not select or execute them.
- [ ] Make the full Gamma execution practical to repeat. The current paired V1
  run is already parallel, but after two CPU-hours per execution a read-only
  sample still places about 91% of time in the audited Alpha VM's common
  instruction-dispatch path. Exact-workload profiling over 16,329,474,048
  Alpha instructions found `imm`, `sub`, `load`, `mov`, `store`, and `add`
  account for 90.2% of execution; the first five account for 81.7%. Reordering
  the existing ARM64 comparison rows by measured frequency preserved every
  handler and improved exact-workload progress by 8.7–11.2% and the
  representative million-call Gamma loop by 8.11% across five alternating
  runs. The reordered seed is now adopted: handler source and disassembled
  handler tails remain byte-identical, Alpha provenance/conformance/assembler
  reproduction, seed diamonds/fuzzing, eighteen representative checker gates,
  and all six principal Gamma gates pass. The same audited seed now issues the
  two adjacent operand-byte loads independently in the hot `mov`, `add`, `sub`,
  `load`, and `store` handlers before advancing the program counter once. Seven
  alternating million-call runs improved from a 2.497-second baseline mean to
  2.287 seconds, about 8.4%, with unchanged result and Alpha semantics; the full
  Alpha, seed-diamond/fuzz, checker, and six Gamma gate set passes. The hottest
  remaining `imm` handler now likewise decodes its adjacent destination byte and
  eight-byte immediate independently before one exact PC advance. Five
  alternating two-million-call pairs averaged 4.708 seconds before and 4.658
  seconds after (about 1.1%), all with status 42; the same complete gate set
  passes. A fresh bounded sample of the exact canonical Delta publication input
  spent 31,178 of 34,913 samples (89.3%) in comparison dispatch; no remaining
  handler exceeded 4.3%. Elaboration took 1.269 seconds and packing 109 ms, so
  preparation and an isolated native handler are not the next bottleneck.
  A bounds-checked 21-row branch-table prototype
  passed all Alpha gates and improved a synthetic two-opcode loop from
  4.28–4.29 seconds to 3.66 seconds, but regressed a representative million-call
  Gamma tail loop from 2.68–2.70 seconds to 3.18–3.19 seconds; it was therefore
  rejected. The simple row reorder is not enough to make the full publication
  run practical by itself. Exact Beta-generated traffic profiling found that
  `imm r?,8` accounts for 2,917,667,729 executions and that the common stack and
  frame helpers can eliminate 2,321,502,576 dispatches by reserving the already
  callee-saved `r13` as a closed-program word-size constant. A temporary
  implementation reached the unchanged-cold-start fixed point, shrank
  `bc.tape` from 52,141 to 40,503 bytes, shrank the Gamma interpreter tape from
  94,903 to 69,833 bytes, passed the Beta corpus and focused structural gates,
  and improved the representative million-call loop by 10.4--10.6%. It was not
  adopted because the semantic admission bundle duplicated old macro shapes
  and byte PCs throughout its 188 fragments; its first failure was the
  `emit_dec Word` canonical smoke. Shared macro-shape decoding has now landed,
  all selected procedure-entry uses resolve by identity, and the emit-dec,
  fixed-emitter, and `gen_stmt` tranches now derive their coordinates from
  source identities. A current direct r13 projection still changes canonical
  source-event rows from 617 to 611 and moves nearly every artifact coordinate;
  93 obligation modules retain matching literals. Centralize macro extents and
  successors for prologue, pop, push, parameter-store, and epilogue consumers,
  then replace row-number event identity with a checker-scanned semantic key.
  Reapply and admit the compiler change only when it changes `bc.beta`, the
  centralized mapper/shape/ABI owners, generated identities, and adjacent
  manifests rather than dozens of semantic obligations. Do that before another
  dispatch mechanism or speculative Gamma rewrite.
  Reduce the cost without changing Alpha or Gamma meaning, hiding a compiler
  stage, or weakening the
  exact evidence join. The 12-hour safety ceiling is not an acceptable normal
  gate duration.
- [x] Retire the unconsumed imperative Gamma compatibility compiler, its native
  artifact, scripts, and private example corpus. It was not the Gamma rung used
  to produce Delta and no default gate or external consumer used it; Git history
  is the archive.

### 2. Make the Delta-produced compiler accept the surface used by `C`

This section depends on the live product source becoming complete enough to
measure `C`. That is an engineering dependency, not an unresolved language
decision: Delta frontend work can proceed from ordinary completed slices, but
the accepted surface cannot be frozen from today's partial compiler.

- [ ] Derive the required input surface from the live resolved closure `C`, not
  from frozen vertical test cases or historical bridge formats.
- [ ] Implement that ordinary-Omega source frontend, checked semantics,
  conservative lowering, target realization, and artifact emission in the
  Delta compiler stage.
- [ ] Preserve ordinary Omega meaning for every accepted form and reject every
  unsupported form before artifact publication.
- [ ] Keep resource ceilings explicit and adjacent-boundary tested.
- [ ] Reuse ordinary compiler formats where useful; do not create a private
  sequence of versioned bridge IRs merely to measure progress.

### 3. Bind the completed product source into the lattice

Design blocker: [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md) Q8 must settle the
requested-target versus source-selected-target rule before the durable product
build entry and final `C` closure can freeze. Implementing Psi, proof checking,
optimization, lowering, emission, and the product command belongs exclusively
to **OMEGA-PRODUCT-COMPILER-SOURCE** in [`TASKS.md`](TASKS.md); those modules are
not bootstrap tasks. This queue begins at the exact closure that task publishes.

- [ ] Consume the deterministic transitive compiler manifest published by
  **OMEGA-PRODUCT-COMPILER-SOURCE** as `C`; do not maintain a second bootstrap
  source list, feature list, or implementation queue.
- [ ] **BLOCKED — OWNER Q8:** finalize the durable requested-target acceptance
  entry and bind the manifest for `C` through the package system.
- [ ] Census the ordinary Omega forms used by `C` only after the complete
  compiler builds; record a tested input boundary without naming a new dialect.
- [ ] Keep package acceptance, generated-source custody, target semantics, and
  admitted boundary claims explicit in the closure.
- [ ] Do not include tools that the compiler executable does not import.

### 4. Perform the first direct Omega build

- [ ] Run the published Delta-produced compiler directly over `C`.
- [ ] Verify and retain the exact `delta C → omega₀` source/artifact refinement
  edge, all reconstructed obligations, target closure, and disclosed admissions.
- [ ] Execute compiler acceptance tests with `omega₀`.
- [ ] Reject any build that requires a shell/Python transformation not expressible
  as the Delta compiler invocation itself.

### 5. Rebuild the same source with Omega

- [ ] Run `omega₀ C → omega` without modifying, regenerating, or translating `C`.
- [ ] Verify the second source/artifact edge independently.
- [ ] Treat binary equality as reproducibility evidence only. Semantic
  correctness comes from the checked edges, not equality between binaries.
- [ ] Demonstrate that optimized and conservative artifacts implement the same
  pinned source meaning.

### 6. Keep orchestration non-authoritative

- [x] Keep one Omega-written product source tree under `source/omega/`, with
  target-neutral Psi phases nested under `source/omega/psi/`; do not create a
  standalone Psi rung or an `omega-bootstrap` owner.
- [x] Maintain one short optional runner that invokes the independently
  executable gates in order.
- [x] Remove obsolete aliases, cache profiles, historical bridge formats, and
  path manifests when no current compiler invocation consumes them.
- [x] Every runner failure must name the exact compiler/gate command that can be
  rerun manually.
- [x] No bootstrap claim may depend on the runner implementation, working
  directory, or availability of a particular shell.
- [x] Name the optional runner by the compiler lattice (`tools/lattice/`) and
  the product-language cases by their owner (`tests/omega/`); do not recreate
  generic `bootstrap/` or `canaries/` repository buckets.
- [x] Keep checker and admission code beside its owner: the root checker under
  `source/alpha/checker/`, Beta admission under
  `source/beta/compiler/validation/`, and Delta publication/custody under
  `source/delta/compiler/validation/`. Do not introduce a generic `assurance/`
  or `refinement/` layer between compiler rungs.

## External contract dependencies

The first authoritative build also requires the package/security owner to
publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows remain review data rather than acceptance
authority. This blocks final publication, not implementation of the direct
compiler sequence.

Track product compiler implementation in [`TASKS.md`](TASKS.md) and package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

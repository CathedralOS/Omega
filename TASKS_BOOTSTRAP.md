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
| Omega source | one permanent product tree under `source/omega`, with target-neutral phases in `source/omega/psi` | finish the product compiler and freeze the exact surface actually used by `C` |
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
  bounded whole-compiler command now consumes all 189 retained `bc-*.alpha`
  modules from `validation/admission/obligations/`; the two exact-subject gates
  live in `admission/`, untrusted witness producers in
  `admission/witnesses/`, and optional generated refinement in `stress/`.
- [ ] Collapse the remaining Beta validator obligation explosion into one
  canonical data format and small responsibility-specific modules. The current
  189 fragments total 64,562 lines after replacing nine private instruction
  scans with shared exact effect-census logic. Continue replacing
  shape/control/data/publication permutations with data decoded by common
  checks; do not recreate cached viewers or debug-only publication paths.
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
compiler artifact. The 2026-08-28 exact attempt was stopped after both parallel
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
  exact reconstruction custody needed by the eventual refinement edge.
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
  representative million-call Gamma loop by about 6.8% in temporary builds.
  It passed Alpha conformance/assembler reproduction and the Gamma interpreter
  gate, but has not been adopted or subjected to the full cross-check suite.
  A bounds-checked 21-row branch-table prototype
  passed all Alpha gates and improved a synthetic two-opcode loop from
  4.28–4.29 seconds to 3.66 seconds, but regressed a representative million-call
  Gamma tail loop from 2.68–2.70 seconds to 3.18–3.19 seconds; it was therefore
  rejected and the committed seed remains unchanged. Validate and, if it
  survives the complete seed/checker/Gamma suite, land the simple row reorder;
  then profile Beta-generated stack/register traffic before attempting another
  dispatch mechanism or speculative Gamma rewrite. Reduce the cost without
  changing Alpha or Gamma meaning, hiding a compiler stage, or weakening the
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

### 3. Close the product compiler source

Design blocker: [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md) Q8 must settle the
requested-target versus source-selected-target rule before the durable product
build entry and final `C` closure can freeze. This does not block implementation
of compiler modules that do not exercise target selection.

- [ ] Finish the Omega-written Psi frontend, proof checking, optimizer, target
  lowering, artifact emission, and command entrypoint required by production
  `omega`.
- [ ] Compute the exact transitive compiler executable closure `C` through the
  package system.
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

## External contract dependencies

The first authoritative build also requires the package/security owner to
publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows remain review data rather than acceptance
authority. This blocks final publication, not implementation of the direct
compiler sequence.

Track product compiler implementation in [`TASKS.md`](TASKS.md) and package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

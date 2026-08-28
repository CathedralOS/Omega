# Bootstrap lattice — active work

Last pruned: 2026-08-27.

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
| Delta | compiler corpus, lower-rung meaning under `source/delta/meaning`, provisional artifacts, source-closure and publication checks | publish the exact Delta-produced compiler from Gamma and extend it to accept all of `C` |
| Omega source | one permanent product tree under `source/omega`, with target-neutral phases in `source/omega/psi` | finish the product compiler and freeze the exact surface actually used by `C` |
| Rust comparator | working implementation under `source/omega-rust` | remain optional and non-authoritative |

## Execution queue

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
- [ ] Collapse the Beta validator's generated checker/permutation explosion into
  one canonical obligation format and small responsibility-specific modules.
  The responsibility audit found 190 `bc-*.alpha` fragments, all consumed by
  the bounded whole-compiler command, amid roughly 72,000 lines. They now live
  under `validation/admission/obligations/`; the two exact-subject gates are in
  `admission/`, untrusted witness producers are in `admission/witnesses/`, and
  the curated/generated refinement suite is optional under `stress/` rather
  than a default lattice stage. This is ownership cleanup, not completed
  modularization. Next replace families of
  shape/control/data/publication permutations with one data-driven obligation
  decoder and responsibility-local checks. Delete cached viewers, duplicated
  generated programs, receipt matrices, and debug-only publication paths when
  no human or authoritative command consumes them.
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
compiler artifact.

Design blocker: [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md) Q16 must ratify the
independent Delta v1 language, resource, and observation semantics before the
canonical compiler can receive checked source-to-artifact authority. The
current Beta-written Delta-to-Gamma translator is an implementation of the
proposed meaning route, not the authority that selects its own semantic
subject. Exact executions, assembly publication, strict target validation,
executable reconstruction, and frontend implementation remain engineering work
and are not blocked on this ruling.

- [ ] Execute the exact canonical Delta compiler source through the accepted
  Gamma route for every required build host.
- [ ] Bind source identity, target identity, emitted artifact identity, and the
  exact reconstruction custody needed by the eventual refinement edge.
- [ ] Once no exact publication attempt is live, move Delta compiler admission
  documentation, receipts, drivers, and tests from the flat `source/delta/`
  root into `source/delta/compiler/validation/`. Keep the semantic elaboration
  under `source/delta/meaning/`; do not create a generic assurance owner.
- [ ] **BLOCKED — OWNER Q16:** bind the independently ratified Delta
  semantics/resources and complete direct lower-rooted source-to-artifact
  refinement.
- [ ] Replace provisional checked-in artifacts with publication receipts rooted
  in that execution.
- [ ] Keep [`source/delta/lower-rooted-assembly-publication-v1.sh`](source/delta/lower-rooted-assembly-publication-v1.sh)
  a verifier for already observed data, not a hidden producer.
- [ ] Ensure the publication path is reproducible by running the constituent
  compiler commands directly without `tools/lattice/verify-lattice.sh`.
- [x] Isolate the parked imperative Gamma compatibility compiler after
  confirming no default gate or external consumer uses it. Its compiler,
  artifact, scripts, and examples now live under
  `source/gamma/compatibility/imperative/`; it is not the Gamma rung used to
  produce Delta and does not appear in the canonical artifact chain.

### 2. Make the Delta-produced compiler accept the surface used by `C`

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

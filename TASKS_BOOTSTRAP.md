# Bootstrap lattice — active work

Last pruned: 2026-08-27.

This queue builds one direct compiler sequence. It does not define a second
orchestration language, a separately named hosted bridge, or a collection of
checkpoint compilers.

## Fixed model

Let `C` be the exact production compiler source closure. `C` is ordinary Omega
constrained to the compositional `Ωself` authoring profile.

```text
Alpha kernels
    → Beta compiler
    → Gamma compiler
    → Delta-produced compiler
    → compile C → omega₀
    → compile the same C with omega₀ → omega
```

`omega₀` and `omega` implement the same language from the same source. The first
binary may be conservatively lowered; the second may use the optimizer and
advanced backend already present in `C`. `Ωself` names the source profile, not
another compiler artifact.

Every arrow must be directly invocable as a compiler operation over exact input
bytes with exact output bytes. Shell and Python programs may order commands,
prepare temporary files, or run negative tests, but they are replaceable
conveniences. No script may silently supply parsing, resolution, lowering,
evidence construction, source discovery, or another semantic stage.

The retired `source/on-ramp/omega-bootstrap` implementation, its private checked
IR generations, and its mirrored refinement snapshots are not bootstrap inputs.
Git history is their archive.

## Standing invariants

- Alpha, Beta, Gamma, Delta, and Omega are the only language capabilities in
  the chain.
- The final Delta-produced compiler accepts `C` directly. There is no separate
  `omega-bootstrap` compiler between Delta and `omega₀`.
- `Ωself` is ordinary Omega with a restricted authoring census: no private
  syntax, altered meaning, file allowlist, or AST-permutation matching.
- The compiler source is authored once. `delta C → omega₀` and
  `omega₀ C → omega` consume the same closure.
- The Rust producer at `source/omega-rust` is a development implementation and
  differential comparator. It is not a bootstrap or release dependency.
- Authority comes from exact source, pinned semantics, checked obligations,
  disclosed admissions, and source-to-artifact refinement—not compiler
  pedigree, fixed points, or agreement with Rust.
- The proof kernel is a cross-cutting checker, not another language rung.
- Standalone interpreters, viewers, REPLs, and debugging tools remain outside
  `C` unless the compiler executable imports them.

## Current state

| Owner | Present | Required closure |
| --- | --- | --- |
| Alpha | audited seed, VM, assembler, gates | keep its accepted host assumptions explicit |
| Beta | Alpha-rooted compiler and self-host tests | retain exact source/artifact joins |
| Gamma | interpreter, type checker, proof-kernel implementation | retain bounded canonical execution |
| Delta | compiler corpus, lower-rung meaning under `source/delta/meaning`, provisional artifacts, source-closure and publication checks | publish the exact Delta-produced compiler from Gamma and extend it to accept all of `C` |
| Omega source | permanent owners under `source/psi` and `source/omega` | finish the product compiler and freeze the exact `Ωself` census |
| Rust comparator | working implementation under `source/omega-rust` | remain optional and non-authoritative |

## Execution queue

### 1. Publish the Delta-produced compiler from below

- [ ] Execute the exact canonical Delta compiler source through the accepted
  Gamma route for every required build host.
- [ ] Bind source identity, Delta semantics/resources, target identity, emitted
  artifact identity, and direct lower-rooted refinement.
- [ ] Replace provisional checked-in artifacts with publication receipts rooted
  in that execution.
- [ ] Keep [`source/delta/lower-rooted-assembly-publication-v1.sh`](source/delta/lower-rooted-assembly-publication-v1.sh)
  a verifier for already observed data, not a hidden producer.
- [ ] Ensure the publication path is reproducible by running the constituent
  compiler commands directly without `tools/bootstrap/verify-lattice.sh`.

### 2. Make the Delta-produced compiler accept `Ωself`

- [ ] Derive the required input surface from the live resolved closure `C`, not
  from frozen vertical canaries or historical bridge formats.
- [ ] Implement the complete `Ωself` source frontend, checked semantics,
  conservative lowering, target realization, and artifact emission in the
  Delta compiler stage.
- [ ] Preserve ordinary Omega meaning for every accepted form and reject every
  unsupported form before artifact publication.
- [ ] Keep resource ceilings explicit and adjacent-boundary tested.
- [ ] Reuse ordinary compiler formats where useful; do not create a private
  sequence of versioned bridge IRs merely to measure progress.

### 3. Close the product compiler source

- [ ] Finish the Omega-written Psi frontend, proof checking, optimizer, target
  lowering, artifact emission, and command entrypoint required by production
  `omega`.
- [ ] Compute the exact transitive compiler executable closure `C` through the
  package system.
- [ ] Census the ordinary Omega forms used by `C` and freeze `Ωself` only after
  the complete compiler builds.
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

- [ ] Maintain one short optional runner that invokes the independently
  executable gates in order.
- [ ] Remove obsolete aliases, cache profiles, historical bridge formats, and
  path manifests when no current compiler invocation consumes them.
- [ ] Every runner failure must name the exact compiler/gate command that can be
  rerun manually.
- [ ] No bootstrap claim may depend on the runner implementation, working
  directory, or availability of a particular shell.

## External contract dependencies

The first authoritative build also requires the package/security owner to
publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows remain review data rather than acceptance
authority. This blocks final publication, not implementation of the direct
compiler sequence.

Track product compiler implementation in [`TASKS.md`](TASKS.md) and package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

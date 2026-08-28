# The Bootstrap Lattice

> **Status: direction plus working lower rungs.** The fixed language spine is
> `Alpha → Beta → Gamma → Delta → Omega`. Alpha through Gamma exist on the
> audited lineage. Delta's complete lower-rooted publication and the direct
> Omega product builds remain open.

## The chain

Let `C` be the exact source closure of the Omega-written compiler. It is
ordinary Omega deliberately authored with a conservative subset of features:

```text
audited Alpha seed
  → Alpha assembler + Alpha-written Beta cold start → bc
  → bc-built canonical Gamma evaluator/type checker
  → canonical Gamma evaluation of Delta compiler source → delta
  → delta + C → omega₀
  → omega₀ + the same C → omega
```

The chain has no separate hosted bridge compiler. `omega₀` is the first product
compiler artifact. Rebuilding the same `C` closes the ordinary self-hosting
edge; it does not introduce another language or compiler generation.
Gamma contributes canonical evaluation, not a required separately published
“Gamma compiler” binary. The executable artifact and source/meaning role at
each arrow must be named exactly rather than inferred from the rung label.

## Trust by checking, not pedigree

A compiler being built by an earlier compiler establishes provenance and
dependency closure. It does not establish correctness: a bad compiler can
reproduce its defect indefinitely.

For each compiler edge, Omega instead fixes and checks:

```text
exact source subject + exact artifact subject
  + canonical semantics + observation profile
  + target-semantics dependencies
  + reconstructed obligations + certificates
  + disclosed admissions
  → checked refinement claim
```

The producer does not choose the obligation set, semantics, or observation
profile. A verifier's result is a re-derivable cache, not authority in itself.
Producer identity and reproducibility remain useful operational metadata but do
not enter the semantic verdict.

## Five roles often confused as “the bottom”

1. **Seed execution** runs the first audited Alpha artifacts.
2. **Language semantics** define what Alpha, Beta, Gamma, Delta, and Omega
   programs mean. Terminal Psi has an internal IR contract, not a rung.
3. **Compiler construction** produces the next artifact in the chain.
4. **Proof checking** validates derivations independently of their producers.
5. **Admissions** disclose the irreducible claims about hardware, firmware,
   foreign systems, and human-controlled release policy.

No implementation gains authority by occupying more than one of these roles.

## The fixed language spine

| Rung | Responsibility | Canonical meaning/status |
| --- | --- | --- |
| [Alpha](rungs/alpha.md) | minimal deterministic tape execution | written small-step semantics; audited native realizations |
| [Beta](rungs/beta.md) | small structured systems language | Alpha-rooted compiler and checked whole-artifact refinement |
| [Gamma](rungs/gamma.md) | safe definitional computation and typing | Beta-written reference interpreter/type checker |
| [Delta](rungs/delta.md) | deterministic compiler-host language | Delta→Gamma elaboration and Gamma execution; publication open |
| [Omega](omega_toolchain.md) | product compiler: target-neutral Psi phases then target realization | Omega-written source; direct Delta and self-build edges open |

The Alpha-owned [proof kernel](proof_kernel.md) is universal checker
infrastructure, not another rung. The feature subset used by `C` is an
incidental source property, not another language. The current Rust compiler in
`source/omega-rust/` is an implementation/comparator, not a rung.

## Meaning routes and compilation routes

The executable route and semantic route deliberately differ. Delta artifacts
are produced by compilation, while Delta meaning is independently available
through Delta→Gamma elaboration and Gamma's Beta-written interpreter. The join
checks the artifact rather than trusting the producer.

The same discipline reaches Omega: product source and product artifacts are
different subjects. Terminal Psi splits target-neutral semantics from target
realization, and target-dependent obligations first arise at realization.

## Proof subjects

All obligations ultimately serve a claim about execution, but mathematical
lemmas may live in intended mathematical models and provider contracts may be
proved at stronger model-theoretic consequence. Every crossing between subjects
is explicit and proved. Source markers such as `embed` and `satisfies` identify
bridge applications; they do not prove the bridge by themselves.

Matching logic or another compact kernel language may eventually encode these
judgments. That choice cannot erase the distinctions among semantic subjects,
observation profiles, target closure, and admissions.

## Irreducible trust ledger

The final formal target-machine-to-physical-machine correspondence remains an
admission. So do opaque foreign behavior and owner release policy where no
formal edge closes them. Admissions are scoped to exact subjects, compose
transitively, and remain re-decidable by every consumer. They never become
verified merely because a producer or parent package accepted them.

## Orchestration is replaceable

`tools/lattice/` may run stages, provide diagnostics, and compare artifacts.
Shell and other scaffolding are permissible while convenient. They are not
semantic owners. Source discovery, parsing, lowering, evidence construction,
and trust decisions belong to compiler or checker stages named above.

Deleting or rewriting a runner may change ergonomics; it must not change what
the chain means.

## No diversified-compilation stage

Diversified double compilation (DDC) is not a rung, gate, or release
requirement in this architecture. The audited seed and the checked exact
source-to-artifact refinement at every edge address the compiler-corruption
question directly across the whole chain. Independently written compilers and
checkers remain valuable regression oracles, but agreement supplies diagnostic
evidence only and cannot replace the checked refinement proposition.

## Open work

The ordered implementation work is in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md). The principal open edges are:

- publish the complete canonical Delta compiler from the lower rungs;
- make that compiler accept the compositional ordinary-Omega surface used by `C`;
- close the exact product source graph `C`;
- build `omega₀` from `C` with checked refinement; and
- rebuild the same `C` with `omega₀` and check the resulting product artifact.

Architecture documents define the chain. They must not grow a parallel task
queue or freeze temporary checkpoint identities as permanent stages.

The exact currently admitted and missing subjects are summarized in the
[bootstrap chain manifest](chain_manifest.md).

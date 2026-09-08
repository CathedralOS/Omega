# Whole-chain minimization

This contract governs retention and engineering comparisons for the selected
[bootstrap chain](README.md). It does not authorize changing a language,
admission, observation contract, or chain edge. The
[owner-escalation rules](#owner-escalation)
apply before implementing an architectural alternative.

## Objective

Minimize the complete human-audited path to the first full Omega compiler, not
an individual file or rung. Count semantics, native seeds and admitted tapes,
mutable state, compiler/evaluator/checker source, proof obligations and rules,
certificates, resource profiles, wire formats, permanent tests, and host tools.
Complexity nearer Alpha costs more because later edges inherit it.

## Retention test

A feature, runtime mechanism, checker rule, sidecar, or tool must serve an exact
customer and reduce total audit cost:

| Owner | Customer |
| --- | --- |
| Beta | Gamma evaluator |
| Gamma | Delta compiler or derivation checker |
| Delta | Epsilon implementation |
| Epsilon | First Omega compiler `D` |
| Omega `D` | First complete Omega compiler |
| Omega-written `C` | Production compiler and its self-host closure |

Familiarity, historical investment, compatibility, current existence, and
hypothetical reuse are not retention reasons. Intermediate self-hosting is not
a customer. Beta's exact self-reconstruction supports its admitted root; it is
not a reason to require self-hosting at the other intermediate rungs.

For an addition, name the exact customer source or required proof obligation;
compare deletion/deferment, consolidation, and a source refactor before adding
machinery. Include evaluator, compiler, checker, and proof costs, deterministic
failure/resource behavior, and focused positive and mutation controls. Remove
the mechanism when its customer disappears, after reviewing its consumers and
preserving any still-required assertions in their selected owners.

Closures, macros, general garbage collection, polymorphism, continuations,
exceptions, packages, and ambient effects do not enter the small functional
rungs by convenience. Reconsidering an exclusion requires a whole-chain
comparison and the applicable owner decision, not a local extension.

## Constraints on comparisons

Preserve the selected contracts: deterministic written semantics; exact seed,
source, and tape identities; closed source envelopes; explicit bounded profiles;
distinct invalid-source, authored-trap, incomplete-capacity, and internal-
contradiction outcomes; exact successful bytes and atomic artifact publication;
independent reconstruction of checked propositions; and no host semantic stage
or source-specific accelerator.

Execution and human-auditable proof closure are both customers. A checked
encoding equation does not prove that the evaluator implements its language.
Admitting a root or moving a checker into readable source does not discharge
that root's correctness assumptions. State exactly which trusted program,
definition, or admission each proposed certificate would remove or retain.

Compare candidates on the same frozen source closures, resource profiles,
accepted subsets, and admissions. Small positive/negative language cases and
valid/invalid derivations are early controls, not substitutes for the complete
next-compiler closure and eventual Omega self-host source. Record:

- semantic cases, state, invariants, proof rules, and resource arguments;
- source/artifact/certificate sizes, retained state and cumulative allocation;
- reconstruction/checking/production time, depth, and memory;
- permanent validation and host tooling; and
- reviewer effort to explain control flow and locate seeded semantic defects.

Distinguish measured peaks, conservative bounds, constructed recipe costs, and
unmeasured work. Lines and bytes are inventory, not an auditability score.
Private capacities are engineering choices; changing one requires a coherent
containment and failure argument, not an isolated constant edit. Sharing terms
does not establish sharing of state-dependent proofs or full-certificate fit.

## Completion and continuation

The minimization program closes when every retained mechanism has a named
customer, every selected edge reconstructs exactly, and the chain rebuilds
offline from its audited root and repository bytes. No alternate compiler,
compatibility adapter, or machine-readable sidecar remains without a consumer.
This is an acceptance condition, not a claim that the chain is complete.

Apply the [repository scope checkpoints](../AGENTS.md#scope-checkpoints) before
expanding support machinery. A paused strategy does not block independent
bootstrap work, authorize abandoning proof obligations, or permit a replacement
rung. Unapproved alternatives belong in
[the comparison proposal](../wiki/proposals/bootstrap_chain_alternatives.md),
not beside the selected chain as another authority.

## Owner escalation

Stop the affected implementation for an owner ruling rather than silently
changing architecture when:

- representative interpreted-D-to-omega0 or omega0-to-omega work has prohibitive
  time, memory, or tape size after ordinary algorithmic/diagnostic cleanup;
- Alpha appears too weak or verbose, including pressure for a new opcode,
  widened encoding, or higher-language primitive;
- certificates or checking remain prohibitive after DAG sharing, compositional
  lemmas, and duplicate-evidence removal;
- a special native accelerator, source-pattern substitution, tape-hash shortcut,
  or other jet appears necessary;
- target ABI, object, runtime, or hardware behavior leaks into Gamma, Delta, or
  Epsilon instead of Alpha realization or the Omega product boundary;
- an edge requires an older rung or host semantic transformation outside the
  explicitly selected source/evaluator composition in the [contract](CONTRACT.md);
- realistic compiler source exhausts a private bound, requires undefined Alpha
  behavior, or cannot receive an explicit fail-closed resource profile;
- an edge appears to require a new trusted axiom or checker rule;
- conforming Alpha realizations disagree on identical tape and input;
- legacy retention requires a second accepted chain, duplicated semantic owner,
  or permanent compatibility adapter; or
- implementation pressure would weaken language meaning, observations, subject
  identity, or fail-closed behavior.

These criteria do not grant permission to add complexity. Until the owner rules,
language semantics and Alpha's instruction set remain fixed and the affected
edge stays open. Existing explicitly selected evaluator composition is not an
undeclared older-rung dependency. A proposed alternative must compare complete
audit cost, preserve required proof obligations, and receive the applicable
owner decision before implementation.

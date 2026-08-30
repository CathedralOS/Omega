# Omega Architecture

These notes describe compiler structure rather than language syntax.

The language guide answers "what does Omega mean?" Architecture docs answer
"which pipeline stage owns which meaning, and how does that meaning change as it
lowers?"

## Documents

- [Repository Layout](repository_layout.md): workspace/folder shape and placement rules.
- [Pipeline Architecture](pipeline/pipeline.md): semantic spine, durable stages, and the normalized questions every stage should answer.
- [Terminal Psi Architecture](pipeline/terminal_psi.md): the Psi-to-Omega
  boundary, why the bootstrap graph/control representations are not portable,
  and the terminal producer, verifier, interpreter, and Omega consumers.
- [Codegen Representation Cleanup](codegen_representation_cleanup.md): standing plan to remove re-declared representations and annotation-only stages so the backend obeys the Architecture Rule below.
- [Whole-Program Assumptions](whole_program_assumptions.md): tracked inventory of where the backend assumes whole-program compilation, against the eventual separately-compiled-component story.
- [Semantic Taxonomy Representation](semantic_taxonomy_representation.md):
  migration from lossy booleans/bitsets to the settled domain, machine,
  multiplicity, reach-row, and termination-plan semantic forms.
- [Authority Values And Boundary Evidence](../design_briefs/authority_values_and_boundary_evidence.md):
  transparent runtime authority carriers, routed qualification evidence,
  receipt-backed fact origination, and checked resource transformations.
- [Terminal Psi, Fuel, And Resource Provisioning](../design_briefs/canonical_ir_fuel_and_resource_provisioning.md):
  terminal-Psi identity and evidence, deterministic compiler-service budgets,
  restricted fixed-work certificates, and capability-provisioned spatial
  resources.

## Architecture Rule

The same semantic concepts should be visible across the compiler, but they should
not be forced into one mega-IR. Each stage should use the form that matches its
resolution level while preserving stable links back to the shared semantic spine.

## Published Identity Law

Every published identity is owned by a small, deterministic normalizer. The
prover may gate legality, discharge obligations, or enable erasure and
optimization; it may never redefine a published identity.

This applies uniformly to normalized domain/type identity (decision 19),
machine/requirement-binding contract identity (decision 20), and reach-row plus
direct-invocation contract identity (decision 22). Decision 23 applies the same law to termination contracts while
placing ranking witnesses behind an implementation-evidence firewall. Solver
timeouts, tactic selection, path-sensitive discoveries,
or a stronger future prover may turn rejection into acceptance, but cannot
silently alter an exported interface hash, monomorphization key, or component
compatibility decision.

Compilers may share arithmetic and canonicalization libraries between the
normalizer and prover. They must not share semantic ownership: normalization
answers what the published object *is*; entailment proves propositions about
that already-fixed object.

## Review Law: Projections Are Not Axes

Coincidence of projections in one blessed instance is not identity of axes.
Before collapsing two concepts, test counterexamples outside the most familiar
core instance and require their composition, inference, and weakening laws to
coincide. This rule has prevented domain denotation from collapsing into
operator agreement, resource obligations into flow facts, and scheduler reach
into suspension.

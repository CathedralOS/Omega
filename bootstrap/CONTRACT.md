# Bootstrap edge contracts

[Source owners](README.md) locate the selected chain;
[minimization](MINIMIZATION.md) governs retention and architectural changes.
This contract states required assurance, not a claim that the edges are complete.

Bootstrap construction evidence is distinct from
[optional application PCC](../wiki/spec/proofs/publication.md). Psi/native proof
publication flags neither enable nor waive this chain's required evidence.
Sharing checker machinery must reduce whole-chain audit and implementation cost,
including isolation of any shared subset; uniformity alone is not a reason to
expand the bootstrap checker. Logical soundness and construction correctness
remain separate obligations.

## Selected execution chain

The audited native Alpha VM and admitted, self-reconstructing Beta compiler tape
form the cold-start floor. Beta encodes the Beta-written Gamma evaluator as
`gamma_evaluator_bytecode.tape`. Gamma directly evaluates the Gamma-written
Delta compiler, which emits canonical Gamma source. That same evaluator runs
the resulting Delta-written Epsilon evaluator over exact Epsilon source D.

D implements Omega. Given the package-resolved Omega compiler source closure C
and ordinary target `alpha_bootstrap`, interpreted D emits
`omega0_compiler_bytecode.tape`. Omega0 recompiles the same C for the same target
to produce `omega_compiler_bytecode.tape`. There is no compiled D tape or Epsilon
evaluator tape. Alpha serialization at this edge belongs to the Omega target,
not Epsilon or its Delta implementation. Below Omega, Beta alone encodes Alpha.

Each language implementation accepts only its own language; an older rung does
not parse its successor's successor. Canonical lower-language receipts and
their selected evaluator composition are explicit semantic dependencies, not
host transformations. Intermediate self-hosting is not required.

## Source subjects

Epsilon v1, the incidental Omega forms used to author C, and full product Omega
are three different contracts. D and C must implement the same complete Omega
language. Conservative authoring of C is not a dialect or permission to narrow
the language accepted by omega0. D may generate slow code; this does not weaken
source meaning. A source manifest cannot become a file or AST-shape allowlist.
For a proposed C facility, distinguish its use by C, exact acceptance by omega0,
implementation for users, and whether an adjacent tool actually belongs to C.

Beta, Gamma, Delta, and Epsilon implementation source consists only of HT, LF,
CR, and printable ASCII. Their language owners define tokens and closed escapes.
The envelope includes comments and is checked before tokenization at exact byte
offsets. Arbitrary program data and output bytes are not limited to this envelope.
Enforce it over exact manifested source closures, not filename suffixes. Even
comment changes alter raw subject identity and invalidate dependent evidence.

D is an exact manifested Epsilon closure; C is independently package-resolved
Omega. The [standalone compiler request](../wiki/spec/build/compiler_request.md)
owns their sealed compilation question and outcome protocol. Target-specific
product dependencies remain symbolic until Omega realization. Psi's
[Terminal product](../wiki/spec/terminal-psi/product.md) is a publishable boundary,
not another bootstrap rung.

The completed chain rebuilds offline from repository bytes and its audited root.
Host tools may invoke, stamp, compare, validate declared byte inventories, and
report. They cannot discover semantic closure, parse or lower accepted source,
manufacture certificates, or decide admission. Temporary reference implementations
need a named diagnostic role and deletion condition; they leave when the checked
edge subsumes that role. Network, package installation, host Unicode services,
or historical compilers are not completed-chain prerequisites.

## Refinement and admission

Every accepted edge binds exact source closure, canonical adjacent-language
receipts and executing Alpha tape, source and Alpha semantics, input,
observation/resource profiles, evidence schemas, independently reconstructed
obligation, checked certificate, and transitively disclosed admissions.
An artifact or certificate producer cannot choose the meaning or a weaker
question under which its artifact is accepted. Reconstruct parsing, names,
types, and operational semantics from the subjects; a producer's AST is not
authority. Replay requires every subject, semantic, profile, and schema join.

Beta encoding equality establishes identical initial Alpha programs and
lockstep behavior under the same input and resources. The
[full encoding subject](proofs/beta_encoding/ACCEPTANCE.md) does not prove that
the Gamma evaluator implements Gamma. The evaluator chain separately owes exact
Epsilon execution refinement. Both Omega compilation edges independently owe
Omega-source-to-`alpha_bootstrap` refinement. A later fixed point cannot repair
an open earlier obligation; byte equality between the two C tapes is not required.

Reconstruction, reproducibility, native pedigree, diversified compilation, and
agreement with Rust are diagnostic evidence, not compiler-correctness proofs.
Reports distinguish verified facts, admitted claims, and provenance, naming the
exact subjects/profiles rather than claiming unqualified correctness. Admissions
remain visible through dependency closure and are reconsidered under each
consumer's policy.

## Non-lockstep refinement

Source and Alpha have deterministic small-step semantics. For general compiler
refinement, present constructive total step functions with terminal self-loops
and traces defined by primitive recursion over natural numbers. Relate states
at nondecreasing synchronization points: one source step may match zero or more
Alpha steps. Every unmatched step must be observationally silent and decrease
a well-founded rank over the related state pair, so neither side can hide
infinite internal work. Determinism, progress, observation, synchronization, and
rank obligations remain explicit checked theorems.

This is the required general refinement strategy, not an implemented extension
of the [finite ground-equality checker](proofs/checker/README.md). That checker
currently has no induction, coinduction, open lemma schemas, or transition-system
rules. A new trusted rule requires a concrete expressiveness failure and owner
decision; certificate verbosity alone does not justify it. Checked intermediate
relations and DAG-shared lemmas may reduce evidence size without becoming extra
executables or permanent compiler dependencies.

The Omega observer is [TerminalTraceV1](../wiki/spec/terminal-psi/observations.md).
Compiler products separately compose sealed inputs, exact diagnostic/artifact
bytes, and product resource outcomes. Execution timeouts and incomplete checking
are not semantic divergence. Capacity profiles must account jointly for framed
source, tape, certificate, retained state, scratch, stack, and checking demand;
accepting a maximum tape alone does not establish that it is provable.

## Native realization

Alpha seeds have independent realization obligations for the same tape semantics.
Stamping adds a length and host container, not a new compiler identity. Where
execution is requested, retain tape-format and exact seed/container custody.
An optional general Alpha-to-native realization requires separate checked
translation validation and cannot replace the canonical tape proposition.
Source-, function-, workload-, or tape-hash-specific native substitutions are
forbidden. Formal-target-to-silicon admission remains separate deployment evidence.

# Optional proof-carrying products

## Product selection

Psi PCC and native PCC are independent requested products. Both are off by
default. They do not select a different compiler pipeline or weaken ordinary
parsing, typing, ownership, effects, proof-obligation or transformation checking.
Without a portable certificate, a receiver trusts the producer for checks it
does not independently repeat; ordinary output is not independently PCC-verified.

The root's authoritative `build.omg` selects the two requests through ordinary
typed Build configuration. The intended interface is:

```omega
builder.pcc.psi = true;
builder.pcc.native = true;
```

These fields describe the required interface, not implemented support. Omission
means `false`; dependency metadata cannot enable or override root selections.
Compiler implementation owns exact API/CLI plumbing, not the semantic defaults.
Requests are retained in normalized Build and cross-invocation realization inputs.
They grant no receiving authority and do not select the receiver's policy.

| Requested evidence | Published proof product |
| --- | --- |
| Neither | Ordinary requested artifacts, without PCC companions. |
| Psi only | The Psi artifact and its `.proof` companion. Native output, if requested, carries no implied native PCC claim. |
| Native only | The native artifact and its standalone `.proof` companion. No separate Psi product is required. |
| Both | Both artifact/companion pairs. Internal proof-production work may be shared. |

A Psi PCC request retains a Psi artifact even during source-to-native
compilation. A stop that excludes native production cannot satisfy a native PCC
request; conflicting product selections reject as configuration errors, not
source invalidity. A later native producer can request native PCC independently
of whether its input was published with Psi PCC. Its input trust and ordinary
checking requirements still apply.

Native proof construction may compose Psi-level reasoning with lowering
preservation, transform that evidence into a native-specific certificate, or
prove native obligations directly. No particular construction strategy or
retention of the original Psi proof objects is required. There is no assumed
size or cost ordering among the four publication choices.

## Sidecar placement and identity

The initial distribution form is a separate file, not an embedded executable
section. Append `.proof` to the complete artifact filename:

```text
program.psi       program.psi.proof
program.exe       program.exe.proof
program           program.proof
```

These are filename examples, not a new required Psi artifact extension. Native
sidecars sit beside the actual executable; in a macOS bundle this is beside the
inner `Contents/MacOS/<executable>`. Package publication accounts for that
additional file. A native proof concerns the identified executable and its
declared execution environment, not automatically the whole application bundle.

The companion carries an explicit Psi/native product kind, exact artifact
content commitment, semantic/checker profile identities, offered guarantees
with all premises, required definitions/evidence, assumption closure and exact
dependency inventory. A filename, producer signature or digest alone establishes
neither a claim nor permission to execute. Proof replacement does not rename
unchanged program bytes; proof identity and program identity are separate.

Bind native evidence to the published bytes after all byte-changing finalization.
Permitted loading, relocation and placement must have explicit checked semantics;
an exact file hash does not establish correctness of a different loaded image.
Later byte changes invalidate the association unless new checking establishes
the required correspondence. Producer and receiver report artifact and companion
byte sizes separately. Compression and internal proof sharing are engineering
choices; embedded distribution is not part of this initial contract.

Stage and validate every requested artifact/companion pair before reporting
successful publication. Failure may leave diagnostic staging data, but cannot
silently downgrade the deliverable to an uncertified success. Publication must
not associate a stale sidecar with newly written bytes. Receivers always verify
the content binding rather than treating adjacency as evidence.

## Receiver-owned requirements

Both products distinguish receiver-required safety guarantees from
producer-published application guarantees. The receiver fixes its requirements;
the producer cannot choose a weaker question by supplying annotations or an
easier contract. Reconstructing obligations from an authored contract alone
does not establish that the contract meets the receiver's needs.

A receiver selects a versioned policy package and its exact concrete
configuration through ordinary package review and pinning. The policy names:

- Required guarantee definitions and their exact semantic identities.
- Artifact/entry subjects and input, entry-state and environmental conditions.
- Accepted semantics, checker and evidence profiles.
- Permitted mathematical assumptions for each claim role and physical authority.

Guarantees resolve in that package or its pinned dependencies. Standard memory,
ownership and effect guarantees have specified operational meanings; a
same-named producer declaration cannot replace them. Additional requirements
use ordinary contracts and predicates over the established program model, not
a separate policy DSL or arbitrary trusted validation callback.

Third parties may author reusable policies. Authority comes from independent
receiver selection, not author identity or inclusion in the offered artifact.
The policy package, dependency closure and concrete configuration commitments
enter acceptance evidence. Ordinary review is not a proof of policy adequacy or
axiom consistency. [Physical permissions](../build/permissions.md) supply the
authority portion, not object confinement or all of memory safety.

All premises are part of the offered claim. The checker establishes that the
offered guarantee meets the receiver's requirement under the receiver's admitted
conditions, using exact equality or checked implication as appropriate. It must
not accept a safety claim conditioned on an unavailable or stronger precondition.
Assumption authorization follows the
[complete declaration closure](foundation.md#assumptions-and-calculus-identity),
independently of conversion and erasure. Policy cannot redefine Omega's language
rules or relabel a weaker claim as the standard safety guarantee.

## Standalone checking and producer annotations

A native receiver needs the binary, companion and its explicitly installed
accepted dependencies, not source, producer memory, a separate Psi artifact or
a separate Psi PCC product. Intermediate definitions, invariants and mappings
may occur inside the evidence when needed. A digest of missing Psi material is
not a substitute for a checkable argument.

Every omitted dependency is enumerated by exact semantic/content identity.
The receiver must independently possess, validate and accept the identified
material. Version ranges, nearby versions and guessed semantic compatibility
cannot satisfy this requirement. There is no implicit network retrieval or
producer-checkout dependency. A separately checked migration is explicit and
does not make mismatch an automatic fallback. Missing material prevents
acceptance; it does not establish that the program is unsafe.

Producer annotations may propose byte partitions, instruction rows, invariants,
control-flow relationships or correspondence to mathematical operations. The
checker validates them against the exact artifact and fixed semantics. Required
executable coverage, incoming edges, indirect targets, premise availability and
composition cannot be reduced by those proposals. Byte coverage alone does not
establish behavior or valid instruction entry. There is no promise to rediscover
missing invariants; required evidence may be a condition of PCC admission.

The common [mathematical kernel](foundation.md) checks application proof
evidence. [Artifact verification](../terminal-psi/verification.md) independently
establishes the questions and their operational interpretation. A native claim
requires native semantics/preservation evidence, not merely a valid Psi theorem.
Wrong metadata rejects or prevents completion; metadata is not a trusted party.

## Outcomes

PCC production preserves the compiler's outcome distinctions:

| Outcome | Meaning for the requested proof product |
| --- | --- |
| `Complete` | Every requested artifact/evidence pair is produced and validated. |
| `Reject` | Invalid request/evidence, violated checking/admission rules, or an established violation of the required guarantee. Name which subject failed. |
| `Incomplete` | A named resource/search limit or unsupported valid case prevents completion; no claim that the unproved conclusion is false. |
| `InternalFailure` | A compiler/checker malfunction or violated implementation invariant; no artifact authority. |

Failure to find a proof is not proof of falsehood. Rejecting a submitted proof
does not refute its conclusion. Failure to admit assumptions does not refute
the theorem either. Contradictory branch premises may legitimately discharge an
unreachable path; inconsistent admitted axioms may derive falsehood. Neither is
automatically an implementation failure. Diagnostics distinguish failed source
checking, invalid proof, unavailable evidence and receiving-policy refusal.

Receiver admission never reports success without its required evidence. These
semantic outcomes do not invent new wire tags or host exit codes; their physical
representation follows the [compiler request](../build/compiler_request.md) and
the owning consumer interface.

## Bootstrap and foreign proof producers

Bootstrap construction evidence, Psi PCC and native PCC have distinct subjects.
The bootstrap [encoding acceptance contract](../../../bootstrap/proofs/beta_encoding/ACCEPTANCE.md)
fixes a concrete source-to-tape equation, not universal compiler correctness or
application safety. Application PCC selections neither enable nor waive required
bootstrap evidence.

Sharing a checker, format or isolated kernel subset is justified only when it
reduces whole-chain audit and implementation burden while covering the required
claims. Isolation and dependencies must themselves be justified. Do not expand
the bootstrap checker into general mathematics merely for uniformity; do not
preserve separate machinery merely because it exists. Construction correctness
and the general kernel's logical soundness remain distinct obligations.

Foreign tools may translate proofs into the common calculus without becoming
trusted checkers. A compatible statement is not sufficient: definitions, rules,
conversion and assumptions need a checked interpretation. Demonstrate one
concrete import before claiming interoperability. No full-corpus import or new
foundational axiom is implied by this publication contract.

## Implementation scope

This is the required product contract, not a claim of implemented general PCC.
`PCC-PRODUCT-PUBLICATION`, `PROOF-CERTIFICATION-BRIDGE` and
`PCC-CANONICAL-SEMANTIC-LEDGER` on [TASKS.md](../../../TASKS.md) own delivery.
The selected [inductive profile](inductive_profile.md) and quotient rules still
need checked justification and implementation. Evidence depending on unfinished
rules cannot claim completion. Bounded publication and coverage checks can proceed
without inventing a new calculus or policy language.

# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-24.

## Q1 — Should trait requirements admit named witness contracts?

Concrete machines use named `requires`/`ensures` contracts as erased witness
input/output lanes. Public trait requirement syntax currently does not admit the
binding form, validation does not define conformance or call forwarding for it,
and checked state-signature facts therefore retain no evidence term. The shared
package projector could encode such a term, but no authored Omega declaration
can coherently produce one today.

Choose whether named evidence is part of a trait requirement's public callable
contract. If it is, specify erased lane ordering, call syntax, conformance
inheritance, default realizations, and whether a renamed `requires` alias remains
local while a renamed `ensures` selector changes public output identity.

Recommended direction: keep trait requirements fact-only until a concrete use
needs abstract witness transport. Then extend ordinary signature syntax and
checked call/conformance semantics together, reusing the existing positional
evidence lanes. A valid larger alternative is to add that complete feature now.
Tempting but wrong alternatives are to expose the latent optional binding field
only to package review, synthesize evidence terms after checking, or invent new
package-only syntax.

## Q2 — What does a boundary clause mean on an abstract requirement?

Boundary syntax distinguishes host and named boundary levels before semantic
lowering, but the state-signature path collapses them to one undifferentiated
`Boundary` contract kind and carries no proof facts. Package review therefore
cannot preserve the authored level or explain how the clause differs from a
boundary trait, exact service reach, or an admitted qualification route.

Choose whether abstract requirements need this clause at all. If retained, its
host/named level, authority effect, inheritance, provider relationship, and
comparison identity must survive as structural checked semantics.

Recommended direction: do not admit boundary clauses on trait requirements;
use boundary-trait identity, service reach, and explicit checked establishment
routes for the one-purpose language's actual boundaries. A coherent alternative
is to retain an exact nominal boundary-level identity through checking when a
real external contract requires it. Tempting but wrong alternatives are to
encode the word `boundary`, treat host and named levels as equal, or infer the
missing level from a service name during package projection.

## Q3 — What compiler/toolchain provenance seals a package instance?

Review orchestration now binds exact compiler-consumed package/toolchain bytes
and the producer executable bytes observed before and after closure review.
That is useful review provenance, but it neither identifies the complete
compiler/toolchain source closure nor proves that the observed executable is
the process image that produced the rows. `CompilerIssuedPackageReview`
therefore correctly remains review-only and cannot yet seal `PackageInstance`.

Choose the exact portable producer provenance required for accepted package
evidence. It must specify:

- the compiler, verifier, evidence-schema, standard-library, target-package,
  and bootstrap/toolchain distribution closure that enters identity;
- whether source closure plus a reproducible-build relation is mandatory, or
  whether an admitted binary/toolchain commitment is a distinct trust tier;
- how a verifier establishes that the executing producer corresponds to the
  committed artifact without pretending an ordinary process can attest its own
  loaded image;
- which parts are compatibility identity versus trust/provenance metadata; and
- how independently bootstrapped or substituted toolchains compare without
  claiming that provenance certifies honesty or proves an audit occurred.

Recommended direction: define a versioned toolchain-closure commitment rooted
in exact source and schema identities, then allow either independently checked
reproduction or an explicit admitted-binary trust tier to bind the executing
producer. Keep capability/API comparison bytes independent of this envelope.
Do not treat a path hash of the current executable, a self-reported version,
PCC, or an audit-attestation string as proof of producer identity or honesty.

## Q4 — How does `build.omg` name its package-scoped filesystem roots?

The build executor already gives each package an immutable source root and a
fresh writable staging root, and the checked interpreter enforces those grants.
Package source cannot name either root portably, however. Relative paths resolve
against the compiler process's working directory, while the only successful
filesystem build test embeds a temporary host absolute path into generated
Omega source. A checked-in package fixture therefore cannot honestly read its
own input and write its staged output without depending on ambient host layout.
The compiler now normalizes paths that pass existing physical grants into
closed Source/Output identities plus canonical relative bytes; that secures the
evidence precursor but deliberately does not invent the package-facing name.

Choose the portable build-filesystem surface. It must:

- preserve the package's explicit `reaches FilesystemHost` ceiling and local
  admission rather than making filesystem authority ambient;
- name the immutable source and writable staging roots without exposing host
  absolute paths or the compiler's current working directory;
- map every accepted path to exactly one grant root, reject traversal and
  symlink escape before host access, and retain the stable rooted spelling in
  observation evidence;
- define cross-platform path bytes and the behavior of operations that return
  paths, including `canonicalize`, `read_link`, and final-path queries; and
- let generated outputs enter compilation only through an explicit staged-tree
  handoff after successful evaluation and evidence custody.

Recommended direction: give the build-time filesystem provider a fixed virtual
namespace with compiler-owned roots such as `/source` and `/output`. Package
code continues to use the ordinary canonical `FilesystemHost` operations; the
build evaluator maps those virtual roots to its private physical grants and
never reveals the mapping. Returned paths are rewritten into the same virtual
namespace or reject when no lossless rooted representation exists. This adds no
package grammar and gives canonical observation transcripts stable path bytes.

A coherent larger alternative is a typed build-directory capability supplied
through the ordinary `Build` value, with relative operations rooted by
construction. It may be preferable if implementation shows that virtual path
spellings repeatedly recreate host-path ambiguity. A narrower acceptable first
rung is to expose only the operations required by the generated-file fixture
through that typed value, then grow it from concrete use.

Tempting but wrong alternatives are to embed host absolute paths into package
source, change the compiler process's working directory, treat arbitrary
relative paths as source- or output-relative by operation, expose an unrestricted
real filesystem provider and rely on post-hoc evidence, or call a denied
operation sufficient coverage for a fixture whose purpose is successful
generation.

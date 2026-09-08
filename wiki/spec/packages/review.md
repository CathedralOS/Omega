# Compiler-derived package review

[Acceptance](acceptance.md) owns the project's decisions and transaction.
This contract defines the facts presented for that decision, not a second
acceptance authority or an executable package format.

## Complete checked findings

After successful checking, derive a complete report for the exact source graph
and target. It includes public API contracts, declared and inferred reach,
reachable implementation authority, selected providers, opaque executable
supplies, accepted assumptions, and relevant build/generated-source facts.
An unused dangerous public API remains reviewable independently of actual
selected-program reach.

Unknown authority, unresolved boundary ownership, omitted transitive effects,
and unsupported proof forms reject rather than becoming empty rows. Generic
obligations remain explicit until their substitutions can be checked. Only
the consuming project's acceptance resolves its blocking changes; dependency
acceptance does not supply that decision.

Read each fact from the earliest coherent compiler-owned representation that
establishes its meaning, then join the facts after successful checking. Typed or
resolved state may own structural identity; checked facts own acceptance,
effects, proof validity, exact witnesses, and assumptions. A preliminary pass
may discover consumer bindings needed for final checking; only final findings
enter comparison. This is input resolution, not a trust distinction between
two results of the same process.

Projection is collection, not another public program stage. Compiler-private
joins may evolve with the compiler. Neither format stability nor an additional
consumer alone justifies another IR. Later validation may repeat a useful
invariant without making the later representation the only permissible source.

## Semantic identity

- Package-owned declarations carry exact package ownership, not import aliases
  or unqualified names. Authored toolchain declarations bind their canonical
  toolchain-relative path and source-byte commitment.
- Generic binders normalize by their declared semantic positions. Compiler
  builtins, carry permissions, value domains, and layout forms use closed typed
  atoms, not nominal owners invented from their diagnostic labels.
- Public types, applications, and contracts retain complete structural meaning.
  Unresolved nominals, malformed canonical values, missing selections, and
  unsupported structural forms cannot be encoded as exact review identity.
- Constants retain checked carrier and canonical value, not display strings.
  Closed conformance arguments retain declaration, ordered arguments, subject,
  target-trait application, and exact checked call substitution.
- Contracts and mathematical bundles retain exact subjects, laws, witnesses,
  and dependencies. Their source/encoding migration follows the
  [proof contract](../proofs/contracts.md); unsupported forms remain explicit
  failures, not partial canonical rows.

Public data includes generic shape, properties, fields/cases/payloads, relevance,
and stable numbered/retired identities. Quotient identity binds the carrier
family and normalized public relation; choosing another equivalence proof does
not redefine the mathematical object. That proof's assumptions and selected
conformance remain admission evidence. Domain identity includes carrier,
binders, index arguments, checked facts, and closed establishment roles/routes.
Transparent aliases normalize to their exact qualified meaning.

Trait and callable contracts retain complete lifetime/type/const/static-machine
telescopes, parameter modes/types, result, parents, requirement signatures,
contracts, reach, direct invocations, suspension, blocking, termination, and
progress subjects. Published may-ceilings are not observations of the current
body. Default-realization availability is public contract data; private body IR
does not become the evidence format. A body/source change remains visible even
when these normalized policy values are unchanged.

## Source explanations

Source locations explain a row but are not its semantic identity. Retain exact
compiler-consumed byte spans under canonical package/toolchain-relative UTF-8
paths. Changed-row conflicts bind the old and new locations actually displayed.
Capture a location with its exact declaration or authored occurrence before
lowering erases that relationship; do not reconstruct it from reduced names,
diagnostic text, fingerprints, or a later source scan.

Generated declarations use their real authored derivation origin. Compiler-
derived facts carry a closed reason and no invented source location. Canonical
sorting keeps semantic rows and explanatory custody paired. Missing, duplicate,
or contradictory required custody rejects. The current source roles and
cross-representation joins are documented
[beside capture](../../../omega-rust/omega/packages/review/evidence/capture.md).

## Distinct evidence roles

Each package-owned bodyless external realization, including an unused private
leaf, has a separate executable-supply trust row. Its self-contained key binds
the exact callable/signature and tagged requirement application; its value
retains the exact mechanism. Trait conformance, operator overload, and top-level
boundary requirement are different requirement roles. Disclosure neither
selects that provider nor proves its implementation satisfies the contract.

Provider selection, representation availability/selection/demand, and supplied
terminal permissions remain distinct facts. A grant does not prove a mechanism
was exercised or admitted by the receiving target. Compiler-derived application
demand does not prove complete physical coverage. See
[provider selection](../build/provider_selection.md),
[opaque representations](../build/opaque_representations.md), and
[boundary realization](../terminal-psi/boundary_calls.md).

Terminal/native evidence is required for claims about emitted execution,
externally supplied code properties, lowering/ABI guarantees, native resources,
or a profile explicitly requiring final-code replay. Ordinary source installation
does not require native emission when its review facts are already established.
A row without Terminal evidence makes no Terminal claim; a generic completeness
bit cannot substitute for this distinction. A project may accept a disclosed
assumption, never an invalid proof.

Recheck acquired content and candidate/project identity before publication.
Build-generated source belongs to the checked candidate and remains in custody
through its use. Compiler transcripts, replay records, and certificate caches
are not ordinary lock acceptance. Existing ledger wrappers do not mandate an
additional lock-certification or package-promotion workflow.

## Target-dependent public identity

A public constant or type application depending on sealed target semantics or a
selected realization retains that exact dependency, not only its folded value.
Independent artifacts compose only with compatible applications. Adding/removing
or changing a public dependency changes API compatibility even when all current
targets happen to compute the same scalar. Private dependencies instead change
content/target identity and require rebuilding or relinking.

Diagnostics retain producer/consumer closures and the origin chain through
aliases, constants, generic applications, and plans. Target selection chooses
declarations; it cannot splice fields/cases into an existing nominal type.
Different field sets use distinct ABI schemas behind a portable requirement.
Different sizes, offsets, padding, and alignment of one schema remain layout facts.

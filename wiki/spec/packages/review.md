# Compiler-derived package review

[Acceptance](acceptance.md) owns the project's decisions and transaction.
This contract defines the facts presented for that decision, not a second
acceptance authority or an executable package format.

## When review runs

Under the owner ruling of 2026-10-03, full-package review and its
discovery/bound passes belong to explicit review, acceptance, and update
workflows. Ordinary compilation does not re-review an accepted package: it
compares each occurrence's source and compiler input identities with the lock
and consumes the exact accepted bindings on a match. This includes the root
and dependency Product/Build occurrences. A mismatch rejects with the package
name and `omega update`, rather than running an inline audit.

Ordinary dependency product checking follows the consumer's imported modules
and their transitive imports, not each dependency's package root. The compiled
program still undergoes normal language, proof, representation, and native
checks. Unlocked check-only observation can report findings without creating
acceptance; missing accepted state stops native production before Build.
See [ordinary compilation](acceptance.md#ordinary-compilation).

## Complete checked findings

In an explicit review workflow, after successful checking derive a complete
report for the exact source graph and target. It includes public API contracts,
declared and inferred reach,
reachable implementation authority, selected providers, opaque executable
supplies, accepted assumptions, and relevant build/generated-source facts.
An unused dangerous public API remains reviewable independently of actual
selected-program reach.

[Assumed binding contracts](../build/assumed_binding_contracts.md) retain their
**Extremely high risk - assumed binding contract** classification transitively.
Show the exact contracts and selected implementation, the assumed entry/exit
facts and authority, consuming root/runtime scope, dependency path and acceptance
status. A checked supplier body does not make its caller-facing relationship
proved. Wrapping, inlining and erasing the evidence cannot remove this finding;
the selected implementation remains visible rather than being reported as an
ordinary checked satisfier of the stronger requirement.

This full fresh report is distinct from retained project acceptance. The lock
keeps exact admission-claim/external-realization callable rows, external
supplies, dangerous capabilities, terminal permissions, and accepted restricted
build requests, plus the compiler identity and exact occurrence-scoped accepted
semantic binding inputs needed by ordinary compilation. These bindings retain
schema/selected-plan digests and explicit permission rows, not a full provider
review snapshot. Complete-row meaning
is retained for those risks; other API, provider, representation, dependency,
and symbolic-demand findings remain fresh audit material. Comparing acceptance
does not reconstruct an old compiler report or compile historical source.

Install/update audits distinguish product/runtime findings from actions requested
on the build host. Before any restricted build action, derive its request from
the checked build closure, including transitive helpers and dependency build
activations. Show the originating package, dependency path, operation, logical
scope, bounds, applicable profile/target, and accepted-versus-candidate change.
Keep suspicious changes and unchanged-authority source changes visible for code
audit; only explicit project decisions accept blocking requests.

This pre-execution request report does not claim a complete final product review.
If further findings depend on unaccepted execution, mark them unavailable/pending
and follow [restricted-build acceptance](acceptance.md#restricted-build-acceptance).
An audit must not perform the very action it is asking the owner to approve.
An inspection command never grants new authority; existing acceptance still needs
actual executor grants before any restricted evaluation. Final generated-source
and product findings remain independently checked.

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

## Critical service audit policy

Every explicit service reach is propagated and available in the full report.
Criticality selects mandatory highlighting and acceptance obligations, not
which effects exist. A provider interface is not critical merely because it
is a boundary trait or requires Build selection.

The standard critical set is a fixed catalog of exact `omega::core` service
identities: `FilesystemContentRead`, `FilesystemContentWrite`,
`FilesystemMetadataQuery`, `FilesystemMetadataMutation`, `DirectoryEnumeration`,
`FilesystemNamespaceMutation`, `ProcessInput`, `ProcessOutput`, `ProcessExit`,
`MachineControl`, `PortIo`, `InterruptControl`, `InterruptEntry` and
`RootMemoryAccess`. These identify the existing authority categories, not new
physical mechanisms. Capability-only declarations may be empty boundary traits;
they need no provider simply to be named in `reaches`. Match canonical declaration
identities, including ownership, never their spelling. A same-named package
declaration cannot acquire core classification. The corresponding physical
authority classes remain independently reported even when no abstract core
service was declared; `ProcessExit` corresponds to process termination.

The consuming root can add an exact custom boundary-trait identity:

```omega
Build::audit_service<MotorControl>(builder);
```

This compiler-owned Build operation takes the activation's original mutable
Build value explicitly and adds the selected identity to the root's critical
audit set. It is policy configuration, not acceptance or provider selection.
Only executed root configuration or helpers acting with its delegated Build
value can add entries; a dependency's own build cannot select the consumer's
policy. Duplicate registrations are idempotent. Additions combine by union;
no call removes a core or previously registered identity. Matching uses
normalized reach including boundary parents. Ordinary traits, strings and
unresolved identities reject. Registered services remain restrictable through
ordinary service exclusions; registration is not required to use those exclusions.

Install, update and explicit review prominently surface core-critical and
root-registered reach with exact callable/package ownership, transitive call
context, target/purpose and accepted-versus-candidate state. New or widened
critical findings require project acceptance under the existing comparison
rules; unchanged accepted findings remain visible without recurring approval.
Unregistered custom reach stays in the full report. Checked-body reach and a
conservative public ceiling remain distinguishable, including overdeclaration.

Root audit selections enter the exact review/acceptance inputs. Registration
itself accepts no finding, grants no runtime or build-host authority, and cannot
relax inherited restrictions. Removing a root registration by editing source
is a review-visible policy change, not a way to reuse an earlier acceptance.
Policy evaluation obeys existing pre-effect Build admission: auditing never
executes an unaccepted action to discover whether it should be accepted.

Selected mechanisms retain their independent physical-authority classifications
and foreign/assumed-supply trust findings. Empty reach or an unregistered wrapper
cannot hide filesystem writes, device access or an assumed binding. Unknown
authority remains unknown/rejected as required above, not benign. A checked
pure numeric provider has no service effect merely due to selection; an opaque
numeric provider may still carry a trust finding.

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
  Closed conformance arguments retain implementation declaration, ordered arguments,
  target-trait application, and exact checked call substitution.
  They have no data subject. Remove the legacy subject metadata with the
  [named-implementation migration](../language/conformances.md#data-and-code-are-independent);
  retaining an empty subject slot does not establish the new identity model.
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
[beside capture](../../../omega-rust/omega/packages/review-evidence/capture.md).

## Distinct evidence roles

For each public callable and the build machine, keep the published reach ceiling
separate from checked-body transitive reach and its preselection concrete base.
That base excludes unresolved installation-selected upper bounds; it is not
final-provider evidence. Bodyless supply has an explicit no-checked-body case,
not a realized copy of its ceiling. Checked supply requires a body; accepted,
requirement, and external supply forbid one; boundary supply may have an adapter.
Underdeclaration rejects. Dangerous overdeclaration is visible audit-recommended
slack with exact callable/service ownership, not an invented body observation.

Under [optional reach ceilings](../language/effects.md#declarations), omission
on a checked body requests inference, whereas a present empty clause is a
zero-service bound. Preserve that distinction in public signatures, comparison
rows and diagnostics. An unannotated public API still publishes its complete
checked summary; an explicit ceiling remains visible alongside its derived
reach. Boundary requirements must retain explicit ceilings, including empty,
and an opaque provider does not acquire inferred-body evidence. Changing an
annotation never hides actual transitive demand or grants manifest permission.

A bodyless guarantee is an explicit accepted claim; a claim-free boundary symbol
is not. Retain the complete published envelope as a distinct trust finding.
Authority establishment, provider guarantees, executable mechanisms, assumptions,
and ABI compatibility are independent axes. Core and physical classifications
come from admitted compiler-owned metadata on exact identities, never
package-controlled names. Custom critical-service additions come only from the
root audit selections above and do not reclassify physical mechanisms.

Each package-owned bodyless external realization, including an unused private
leaf, has a separate executable-supply trust row. Its self-contained key binds
the exact callable/signature and tagged requirement application; its value
retains the exact mechanism. Ordinary trait implementation, boundary-trait
implementation and operator overload are different requirement roles. Disclosure neither
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
are not ordinary lock acceptance.

Unproved source contracts reject in Psi checking, before any package review or
policy decision. Every normal return, including a tail call to another
machine, owes the caller's guarantee. Accepted axioms remain disclosed
assumptions; project policy cannot make an invalid proof valid.

An arithmetic entailment engine's stand-down is not a final checking verdict:
the ordinary checker can prove the guarantee through another route, including
transparent propositions and explicitly assigned proof evidence. Package review
does not re-ask those engine-local questions; source contract checking owns
them.

Reports separate mechanical checking from admissions and from
producer/signature/audit metadata. A signature records custody, not review quality;
a kernel establishes only its exact checked logical claim. Observing pathname
bytes at `current_exe()` does not identify the loaded compiler image and enters
neither package review, lock, conflict, cache, nor admission identity.
The accepted compiler input identity instead combines a build-produced digest
embedded in the compiler with the exact selected toolchain library content.
Review retains that identity across checking and verifies it again before lock
proposal. Compiled-review cache keys use the same identity; inability to identify
these inputs is a cache miss, not a weaker developer-version key. This identity
invalidates stale inputs and makes no compiler or audit attestation.

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

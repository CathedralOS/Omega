# Package review evidence

`record` holds the package policy that can be retained in a lock. `encoding` projects decisions, encodes canonical bytes and text, and recovers them under the caller's resource limits. Lock recovery, policy differences and comparison of retained consent use these records directly. Their versions and coordinates belong to the [schema](EVIDENCE_SCHEMA.md) and [policy baseline](POLICY_BASELINE.md).

`TerminalPermissionAcceptance::compare_policy` compares a complete proposed policy with the retained lock rows for the same package and target. It checks the version and exact coordinate and meaning of each Terminal permission and component assumption. The application must separately establish that the proposal belongs to its current checked product before execution. Recovery of saved bytes alone confers no execution permission.

## Issuing a fresh package review

The [package review contract](../../../../../wiki/spec/packages/review.md) requires one complete review tied to the same source, Build, target and execution. The [candidate pass](../candidate/passes_occurrences.rs) consumes the in-memory project handoff. The fields it receives have distinct issuers.

| Review fields | Required source |
| --- | --- |
| Source consumption commitment, consumed units, accepted semantic bindings | `CheckedSourceCustody` retains the exact frozen trees and the `PackageCompilationSubject`. Source identity includes the selected source closure. |
| Build machine, evaluation usage, observed inputs and outputs, restricted requests | `select_build_machine` and `admit_build_machine` establish the selected Build. `PreparedBuildEvaluation` and the evaluated configuration own its completed evaluation. A request is a `build_consent::RestrictedBuildRequest`, including its bounds and grant root. |
| Build handoff | `PackageBuildHandoff` belongs to package compilation. Its package, target, source commitment and selection observation must match this exact occurrence. |
| Service and hosted-exit candidates | An accepted semantic binding must carry the exact supplier's declaration, compiled source and checked service schema. A nomination is independent of the consumer's later decision. |
| Whole-package policy and its canonical byte count | The source publication must cover the complete declared inventory, including unused public declarations. Native selection must match the same source and target for provider and permission rows. The encoder measures the completed baseline against the caller's budget. |

An explicit package review opens the Product from the Build that resolved that package occurrence. The compiler checks the full declaration inventory and selects providers for the requested target. The candidate pass joins the resulting policy to the source commitment, purpose, execution profile and Build handoff before retaining it. Entryless libraries follow the same path. [Product policy projection](../../../compiler/build/planning/package_review/mod.rs)

### Target-entry proposals

A fresh nomination carries the supplier's checked declaration, role and service schema. Discovery passes it to consumers through their package compilation inputs. Within the same invocation, the package manager can reuse an earlier review when its inputs and permissions still match. [Review passes](../candidate/passes_occurrences.rs)

## Encoding

The [normalized policy baseline contract](POLICY_BASELINE.md) describes the
receipt-free components kept in the lock. Detailed encoding lives here; it adds
no audit or certification step.

`encoding/` owns canonical framing and bounded recovery. It reads only stable
records. Capture and encoding trust the compilation they
read; only recovering stored bytes validates their structure.

The external supply component records an exact checked machine's external
executable supplies. It keeps the complete callable signature, requirement,
binding, target and producer identity, and leaves out evaluator accounting and
reconstruction receipts. Its bounded `OMEGA-EXTERNAL-SUPPLY-POLICY` encoding is
version 10. Callable and external rows keep the target context the author wrote,
independent of the selected activation. Exact family-member handles keep it
across selection's marker changes. Inactive private leaves stay
disclosed, with bindings normalized for their own target; disclosure cannot
select them as providers. Pure-body tags without a catalog entry stay inert
data. The complete baseline checks that callable and supply contexts match and
the normalized binding target is exact. Capture needs the original checked
signatures: the legacy review shape cannot recover a missing result, complete
nested progress routes, or actual conformance lifetime arguments once those
distinctions have been projected away.
`recover_canonical()` restores typed signatures, contracts, expressions and
bindings without an old checkout or running the compiler. Recovery rejects
unknown vocabulary, truncated or trailing fields, and noncanonical encodings.
A retired tag, such as the string-backed import bootstrap (tag 0 of both the
selected-provider binding and the external-supply binding), is rejected by name
and never reused, so component versions stay the same and every value the
current toolchain can produce keeps its exact bytes. Callers can lower the byte,
aggregate-element, owned-storage and nesting limits. This component
is not an accepted-lock record or an acceptance decision.

The representation component records the complete representation policy of one
package in the checked closure. It keeps opaque declarations, full public
conformance availability, every independently rederived selection, and actual
calling demands separate. An unused placement-only selection stays visible, just
like an unused checked-copy selection. The selecting package owns its choices
and demands even when the opaque declarations and carriers are foreign. The
authoritative build's empty lifetime parameter list is distinct from a called
requirement's list. Its name, spans and compiler receipts stay out of policy
identity.

The `OMEGA-REPRESENTATION-POLICY` version-14 encoding embeds full calling and
conformance meanings under one recovery budget. It checks exact ownership,
producer and selection associations, and that every opaque use is covered
within each kept calling application. This is inert policy, not native evidence
or an accepted lock.

The selected provider component records the selected closure owned by one exact
checked root activation and target. It records typed schema and realization
associations, authored selections, and grants. Service signatures keep
structured type ownership and static contracts even without a calling plan. With a calling plan, the exact selected schema application and declaring
requirement join the complete calling component again. Service reach, direct
invocations, progress-profile subjects and establishment routes keep their
exact declaration owners, independent of the readable service strings.
Evaluated imports and syscalls keep producer and binding meaning, not
receipts. Table bindings keep the exact data identity that their
leaf's first parameter borrows; installation rows keep both the published
ceiling and the checked realization reach.

An operator or top-level requirement signature without static parameters is
borrowed from the checked compilation. With static parameters, capture
instantiates through short borrowed reads of plan-layout rows, schema
definitions and the matched node, rebuilding handle spans only where members
change. Both paths keep the source association,
lifetime validation and complete signature projection.

The `OMEGA-SELECTED-PROVIDER-POLICY` version-24 component places grants inside
their complete normalized plan and links atomic family coordinates by canonical
plan index. Recovery checks complete family coverage and typed structural
associations under one shared resource budget. Generic declaration families
stay separate from actual D29 demands and realization evidence. This component
is not an acceptance decision; lock recovery and transactions are separate work.

`project_checked_terminal_permission_policy` records supplied permissions
independently of selected providers or demand. Each permitted service keeps its
complete method schema in declaration order, including unpermitted methods, root static and lifetime parameter lists, structured signatures, and
checked calling context when present. Generic declarations keep symbolic
parameter relations, not an invented closed provider or calling application. An explicit empty class set differs from no permission. The bounded
`OMEGA-TERMINAL-PERMISSION-POLICY` version-13 component shares the service codec
and one recovery budget with nested contracts and calling applications.

Existing accepted schema digests are inputs to compiler matching, not baseline
payload. In particular, UEFI's semantic-only accepted schema still leaves out
its separately owned calling receipts. Keeping checked calling context in the
baseline neither changes that key nor grants target-entry ABI authority. No policy component replaces native permission containment or proves
that a supplied permission was accepted or used.

The callable component records boundary, public, selected-build, private
admission-claim and private external-callable surfaces for one exact checked
root and target. Private external leaves keep their own outer contracts and
operational promises even when unused; an equal supplied requirement and
binding do not imply equal authored leaf policy.
The `OMEGA-CALLABLE-POLICY` version-18 component keeps full static and lifetime
signatures, ordered contracts, exact identities of overloaded callables, and
actual conformance lifetime arguments with their equality partition. Published
reach, direct-invocation, suspension, blocking and termination promises stay
separate from checked summaries. Public suspension
and blocking summaries use their published ceilings, not effects found only in
the body. Service-reach dependencies separately keep a finite union of concrete
services and machine-binder positions in the complete static parameter list.
The existing binder contract owns the exact trait and requirement identities;
structural binders cannot appear as variables. Before projection, capture
compares the checked dependency contents with one independently inferred
request plan. Recovery rejects out-of-scope, structural, repeated or
unordered variables. Extracting a private helper or renaming a binder does
not change this dependency, but adding a fixed service does, even when its
conservative bound was already present. This is policy for open generic
callables; it does not replace evidence about specialized applications or
establish propagation in Terminal Psi.
Crash guards, including guards on nested static machines, keep typed
expressions and exact foreign declaration owners. The checker's inferred body
summary separately supplies conservative crash causes when its call closure is
complete; a missing summary is marked explicitly, not shown as an empty cause
set. Guard refinements and private proof sites are left out. Progress premises
keep entry-relative subjects and establishment requirements. Mutation keeps the
checked entry write frame and whether it is complete, not private state
coordinates. Caller-local capability flows and the exact reachable-machine flow
union are separate; private helper names and the authority
of unreachable helpers affect neither policy identity.

Recovery restores typed records under one budget for nested signatures and
expressions, checks scope and structural associations, and requires exact
canonical re-encoding. Neither encoding nor recovery restores proof or crash
derivation tables, grants authority, or certifies an assumption.

`project_checked_package_policy` builds a `PackagePolicyBaseline` for one exact
checked root package and target. Its version-33 `OMEGA-PACKAGE-POLICY` envelope
holds all seven public declaration families, the normalized components above,
every package-owned external supply (including unused private leaves),
dangerous authority and ceiling slack, semantic dependencies, and D29
application links. Private semantic consumers are grouped under their package
without losing dependency kind or exposure; exact attached cleanup declarations
stay visible. D29 symbolic demands keep the exact producer callable and
type-binder mapping. Closed applications link to canonical selected-plan
positions and keep their authored realization or closed intrinsic, without
specialization state names or execution commitments.

Public and nested policy signatures distinguish no result from a declared empty
data result, keep typed crash guards and complete progress establishment
routes, and keep lexical static and lifetime scopes. Those signature-bearing
component schemas have their own versions, declared in `src/encoding/mod.rs`;
callable policy is version 18 and includes authored target context. Structural
predicate paths keep fixed indexes as numbers, distinct from fields and cases.
An old policy version needs fresh comparison, not an upgrade guessed from lossy
records.

The whole baseline uses one budget for bytes, elements, allocations and nesting,
with no nested envelopes or budget resets. Recovery checks that components
agree on package and target, declaration scopes, canonical ordering and kept
cross-component associations, then exact re-encoding, without an old checkout.
This is not a lock or a policy decision: the manager's
[lock format](../../lockfile/README.md) joins it to source pins,
the graph and past decisions. Source locations, proof certificates,
stand-down and discharge records, and replay receipts are left out.

`PackagePolicyBaseline::canonical_text()` writes the same complete traversal as
named fields, explicit optional values, ordered sequences, decimal scalars,
quoted byte strings, and named variants with their wire tags. Only digests use
hexadecimal. The text format is versioned separately from the binary baseline
and components. `recover_text_with_usage()` rebuilds the
bounded canonical scalar stream within an enclosing document's budget, applies
the existing typed recovery, and checks every text byte by streaming the
recovered value through the same named traversal.

`PackagePolicyBaseline::canonical_rows_with_limits()` turns that complete policy
into comparison rows, each with its own version. Every row keeps its semantic
coordinate, complete canonical value and named readable text; a digest does not
replace its meaning. Coordinates use exact declaration identities, not report
positions. Selected provider plans, their families and closed D29 applications
stay one association row because their canonical plan indices mean something
only together. Symbolic demands stay keyed separately by operator, producer
callable and type-binder mapping.

Rows separate initial trust requirements from changes to an existing
compatibility contract. Ordinary initial API and checked callable contracts do
not invent assumptions. Admission claims, external supplies and their outer
contracts, dangerous authority, and explicit permissions keep their review
requirements. Representation policy without claims, and ceiling slack,
recommend an audit without becoming blanket approval requirements. These
are inert findings, not decisions or proof of review.

The manager's compact lock acceptance stores only those risk-bearing rows,
including their complete readable meaning. Full compatibility rows stay fresh
audit material, not stored snapshots of past API or representations; changes to
them alone do not need project approval. Source replacements and root-role
changes stay separate decisions owned by the manager. Recovering or comparing compact
acceptance never compiles old source.

Projection bounds the complete row table, the kept key, binary and text buffers,
the aggregate structural traversal, and nesting. It measures output before
reserving storage and reports usage so a manager can share limits across both
complete graphs. Sizing and writing both consume the structural budget.
Changing a field name, variant label, order, escape or container breaks an
accepted text record even if its binary scalars stay the same.

Expanded text is limited to 32 MiB; canonical binary stays limited to 4 MiB.
Text recovery charges rebuilt binary storage, typed allocations and canonical
binary verification scratch against the same 64 MiB owned-storage limit. Text
verification borrows its input and creates no second expanded text buffer.
Markup depth is limited separately from semantic recursion. The manager
combines the policy with source pins, graph edges and
past project decisions; readable policy is not itself a lock.

`recover_text_with_usage()` returns that same recovered baseline with plain
charges for owned bytes and aggregate elements. An enclosing document subtracts
these charges before recovering its next baseline, so separately bounded
children cannot reset the document's remaining resources. Charges include the
requested rebuilt-binary reserve, typed allocations and canonical binary
scratch; they are not an exact heap measure or an acceptance claim.
`canonical_text_with_element_count()` likewise returns the serializer's element
charge from the same traversal, so aggregate writers can enforce the same limit
without recovering or walking each baseline a second time.

`validate_package_membership()` visits every typed package identity in the
complete policy traversal without allocating canonical output. The semantic
identity owners also visit nested runtime types and compiler-owned framed
signature, lifetime-domain, callable and binder identities. Literal text and
authored paths are not searched for package-like substrings. The
containing source graph supplies the membership test; this needs no direct
dependency edges, rebuilds no foreign declarations and grants no authority.
One budget for lowerable nodes, depth and requested unescape storage covers
both identity grammars. The returned plain usage lets enclosing locks charge
later baselines instead of resetting their aggregate resources. A refusal is a
`PackagePolicyMembershipFailure`: the error class and, for `MalformedIdentity`,
a bounded window of the innermost identity text where reading stopped, so a
failed lock names the declaration rather than only the rule. Running out of a
limit or naming an unknown package is never reported as malformed.

The runtime type grammar's tags are the closed list in
`language_semantics::type_identity::vocabulary`. The renderer writes tags only
through it, and the grammar reads each tag or refuses it with a reason. A form
added to one side but not the other therefore fails to compile, or fails the
round-trip test in `typed-trees`, instead of showing up when a lock is built.

The crate root exports `project_checked_package_policy` and
`project_checked_terminal_permission_policy`. The manager owns comparison and
policy; the crate admits no package or executable operation.

Exact component vocabulary and revision notes live in
[`EVIDENCE_SCHEMA.md`](EVIDENCE_SCHEMA.md).

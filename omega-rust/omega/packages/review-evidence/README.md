# Omega Package Review Evidence

This crate records checked compiler facts for package review. Its output is
inert evidence: not package admission, an accepted lock, or proof of an audit.

Capture reads the immutable `CheckedCompilation` product directly from
`products`. Production review does not depend on the compiler coordinator or Build
evaluator; running the compiler is a test-only dependency for fixtures.

[`permission_acceptance.rs`](src/permission_acceptance.rs) compares the root
Product's permission consent, as supplied by the package manager, with freshly
checked service declarations. Compilation calls it before native production;
it neither rebuilds a full review nor creates consent. Schema,
package, target, row coordinates and complete readable meanings must match.

`PackageReviewInput` borrows the candidate's typed trees and the
`CheckedCompilation` holding them. It refuses a candidate that is not the
accepted source, a non-portable judgment, an incomplete declaration subject,
or a selected Build without an accepted judgment. It grants no acceptance.

Start at [`src/lib.rs`](src/lib.rs), then follow the question you are asking:

```text
src/
├── lib.rs       public entrance
├── record/      what stable package-review facts exist
├── capture/     how checked compiler state produces those facts
└── encoding/    how facts are canonically encoded and recovered
```

The ratified package workflow reviews compiler-derived reachability, unsafe API
and assumption facts. The lock records pins, the dependency graph,
accepted baselines and decisions; the project trusts whoever lands it. Compiler
proof and reach checks and native artifact checks keep separate jobs.
The [review specification](../../../../wiki/spec/packages/review.md) defines
those distinctions; [capture joins](capture.md) maps the compiler inputs and the
source each one explains.

## Record

`record/` is the review vocabulary, free of source handles. Begin with
`record.rs`; its children group package identity, public signatures and
contracts, authority and behavior, representation commitments, complete package
records, and policy rows. It neither inspects compiler state nor encodes
stored bytes.

The representation record marks package-owned opaque data as `Unbound`, and
separately records which opaque, conformance and carrier forms each public
producer candidate makes available. Availability chooses nothing for a consumer
and may coexist with `Unbound`. [Opaque consumer demand](../../../../wiki/spec/build/opaque_representations.md)
belongs to the consumer that selects it and exists only where a value actually
crosses a boundary by value at run time. A demand keeps the exact boundary
requirement application, the complete checked shape graph, every opaque carrier
occurrence and its semantic path, the replay-validated physical placement, the
target and calling policy, and strong selection and boundary-plan commitments. Checked compilation records the exact carrier shape root as it
materializes each opaque; capture does not infer occurrences from equal layouts
or an aggregate digest. Package composition joins each real foreign opaque use
to the producer's declaration and availability rows, tied to the immutable
source the consumer compiled against, and requires
the selected application to equal the producer's own selection at the exchange.
It does not need a certified `PackageInstance` or a lock-promotion stage.

## Capture

`capture/` is the only part that reads compiler state. Begin with `capture.rs`;
`input.rs` builds the input, `policy.rs` the whole-package baseline,
`declarations.rs` public declarations, `callables.rs` machine contracts and
behavior, `package_review.rs` review evidence, `runtime_permissions.rs` consent
rows, and `semantic_bindings.rs` the target-entry binding candidate. Capture may
build records; records never depend on capture. Capture refuses an unsupported
nonempty declaration or authority inventory rather than return a partial review.

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
[lock format](../package-manager/src/lock/README.md) joins it to source pins,
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

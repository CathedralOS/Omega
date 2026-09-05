# Omega Package Review Evidence

This crate records checked compiler facts for package review. Its output is
inert evidence: it is not package admission, an accepted lock, or proof that an
audit occurred. Start at [`src/lib.rs`](src/lib.rs), then follow the question
you are asking:

```text
src/
├── lib.rs       public entrance
├── record/      what stable package-review facts exist
├── capture/     how checked compiler state produces those facts
├── encoding/    how facts are canonically encoded and recovered
└── ledger/      how rows and supported open results are reconstructed locally
```

The ratified package workflow uses compiler-derived reachability, unsafe API,
and assumption facts for review. The lock records pins, the graph, accepted
baselines, and decisions; the project trusts whoever lands it. The existing
ledger and manager promotion machinery described here is not a requirement
for certified lock acceptance and is to be simplified where it duplicates
compiler checks. Compiler proof/reach and native artifact checks retain their
independent roles.

## Record

`record/` is the source-handle-free review vocabulary. Begin with
`record/mod.rs`; its children group package identity, public signatures and
contracts, authority and behavior, representation commitments, complete
package records, and canonical rows. It does not inspect compiler state or
encode persistence bytes.

The representation projection records package-owned opaque data as `Unbound`
and separately records each public producer candidate as exact
opaque/conformance/carrier availability. Availability accepts no consumer
choice and may coexist with `Unbound`; it says only what the producer exposes.
D26 consumer demand is owned by the selecting consumer and exists only for an
actual runtime by-value crossing. It retains the exact boundary requirement
application, complete checked shape graph, every opaque carrier occurrence and
its semantic path, replay-validated physical placement, target and calling
policy, and strong selection and boundary-plan commitments. Checked compilation
records the exact carrier shape root while materializing each opaque; capture
does not infer occurrences from equal layouts or an aggregate digest. Compiler
composition still needs the immutable foreign-source join; it does not require
a certified `PackageInstance` or lock-promotion stage.

## Capture

`capture/` is the only compiler-facing branch. Begin with `capture/mod.rs`,
then follow `package/` for whole-package assembly, `api/` for public
declarations, `callables/` for machines and realizations, `providers/` for
selection, `behavior/` for operational facts, `contracts/` for checked facts,
`semantics/` for typed identity, or `source/` for authored custody. Capture may
construct records; records never depend back on capture.

Within `contracts/expressions/`, `projection/mod.rs` retains recursive custody
and routes value forms, operator forms, calls, and members into named semantic
leaves. Its siblings own the narrower checked-resolution joins reused by that
descent.

`capture/providers/symbolic_demands.rs` retains the first producer-side D29
exchange form: an authored named boundary use in a public generic callable
whose operator type binders map directly to callable type binders. The row is
package-qualified and blocking, but intentionally carries no provider,
realization, coverage, Terminal, native, admission, or audit claim. Foreign
specialization and final closed substitution remain downstream composition
work.

## Encoding and ledger

`encoding/` owns canonical framing and bounded recovery. It consumes only
stable records and does not inspect compiler IR. `ledger/` owns the distinct
local reconstruction question: recovered producer rows remain inert until the
selected local compiler reconstructs the complete row set and requires exact
equality.

External executable supplies expose a separate `policy_projection()` retaining
the complete callable signature, requirement, binding, target, and producer
identity while omitting evaluator accounting and reconstruction receipts. Its
bounded `OMEGA-EXTERNAL-SUPPLY-POLICY` component encoding is version 1.
`recover_canonical()` restores typed signatures, contracts, expressions, and
bindings without an old checkout or compiler execution. Recovery rejects
unknown vocabulary, truncated or trailing fields, and noncanonical encodings;
caller ceilings can lower the hard byte, aggregate-element, owned-storage, and
nesting limits. The existing full-review encoding and validators are unchanged.
Integration into the complete normalized policy baseline remains separate;
this component is not an accepted-lock record or an acceptance decision.

`project_checked_representation_policy` captures complete representation policy
for one package in the checked closure. It keeps opaque declarations, full
public conformance availability, all independently rederived selections, and
actual calling demands separate. An unused placement-only selection remains
visible just like an unused checked-copy selection. The selecting package owns
choices and demands even when opaque declarations and carriers are foreign.
The authoritative build's empty lifetime telescope is distinct from a called
requirement's telescope. Its name, spans, and compiler receipts do not enter
policy identity.

The `OMEGA-REPRESENTATION-POLICY` version-1 encoding embeds full calling and
conformance meanings under one recovery budget. It checks exact ownership,
producer/selection associations, and complete opaque-use coverage within each
retained calling application. This is inert policy, not reconstructed native
evidence or an accepted lock. The older representation-TCB review rows and
their replay-bearing encoding remain unchanged.

`project_checked_selected_provider_policy` captures the selected closure owned
by one exact checked root activation and target. It independently validates
typed schema/realization associations and reharvests authored selections and
grants. Its service signatures retain structured type ownership and static
contracts even without a physical calling plan. With a calling plan, the exact
selected schema application and declaring requirement rejoin the complete
calling component. Service reach, direct invocations, progress-profile subjects,
and establishment routes retain exact declaration owners independently of the
readable service strings. Evaluated imports and syscalls retain their producer and
binding meaning, not evaluation receipts. Table bindings retain exact attached
data identities; installation rows retain both the published ceiling and the
checked realization reach.

The `OMEGA-SELECTED-PROVIDER-POLICY` version-1 component places grants inside
their complete normalized plan and links atomic family coordinates by canonical
plan index. Recovery checks complete family coverage and typed structural
associations under one shared resource budget. Generic declaration families
remain distinct from actual D29 demands and realization evidence. Neither this
component nor the existing review encoding is an accepted package baseline;
baseline composition, lock recovery, and transactions remain separate work.

`project_checked_terminal_permission_policy` retains supplied permissions
independently of selected providers or demand. Each permitted service retains
its complete declaration-ordered method schema, including unpermitted siblings,
root static and lifetime telescopes, structured signatures, and checked calling
context when present. Generic declarations retain symbolic parameter relations;
they do not invent a closed provider or calling application. An explicit empty
class set is distinct from no permission. The bounded
`OMEGA-TERMINAL-PERMISSION-POLICY` version-1 component shares the service codec
and one recovery budget with nested contracts and calling applications.

Legacy review and policy capture share exact accepted-service and inherited
requirement joins. Existing accepted schema digests remain compiler matching
inputs, not baseline payload. In particular, UEFI's semantic-only accepted
schema still omits its separately owned calling receipts. Retaining the checked
calling context in the baseline neither changes that permission key nor grants
target-entry ABI authority. No policy component replaces native permission
containment or proves that a supplied permission was accepted or exercised.

`project_checked_callable_policy` captures boundary, public, selected-build,
and private admission-claim surfaces for one exact checked root and target.
The `OMEGA-CALLABLE-POLICY` version-1 component retains full static and lifetime
signatures, ordered contracts, exact overloaded callable identities, and actual
conformance lifetime arguments as well as their equality partition. Published
reach, direct-invocation, suspension, blocking, and termination promises remain
distinct from retained checked summaries. Public suspension and blocking
summaries conservatively use their published ceilings, not body-only effects.
Crash guards, including nested static
machine guards, retain typed expressions and exact foreign declaration owners.
The checker's inferred body-summary owner separately supplies conservative crash
causes when its call closure is complete; an unavailable summary is explicit,
not an empty cause set. Guard refinements and private proof sites are omitted.
Progress premises retain entry-relative subjects and establishment requirements.
Mutation retains the checked entry write frame and its completeness, not private
state coordinates. Caller-local capability flows and the exact reachable-machine
flow union are separate; private helper names and unreachable helper authority
do not enter either policy identity.

Capture rejoins the retained checked owners and authored declarations before
normalization. Recovery restores typed records with one shared budget for nested
signatures and expressions, checks scope and structural associations, and
requires exact canonical re-encoding. Neither operation recovers proof or crash
derivation tables, grants authority, or certifies an assumption. The existing
review encoding is unchanged; complete baseline composition remains separate.

The supported result lanes do not pretend to prove a bodyless accepted claim,
grant dangerous authority, validate externally supplied executable code, or
exercise or admit a terminal-authority permission. `ledger/results.rs` rejoins
each typed compiler fact to its canonical row and assigns `OpenRootAdmission`,
`OpenLaterDischarge`, or one concrete `Discharged` result. The first discharge
class matches an exact compiler-owned assumption certificate to the reviewed
callable, contract/fact coordinates, and strong contract commitment, then
independently reconstructs and checks it through the proof kernel. Missing,
unsupported, malformed, duplicated, or nonjoining certificates leave the
obligation open or reject. The manager may propagate remaining open obligations
to a consuming root, but root policy cannot admit a later-discharge obligation.

The source-handle-free discharge now has its own canonically encoded review
row beside the still-blocking stand-down row. Fresh capture and ordinary-ledger
replay independently recheck its exact semantic/kernel question before the
local result closes that obligation. These checks concern an actual compiler
proof question, not certification of lock acceptance. Neither row
is a root-policy decision or package-admission authority.

The crate root exports `project_checked_package_review` for ordinary checked
review and the separately non-executable
`project_non_executable_quotient_package_review` for the bounded proof-only
total-direct `define` and position-preserving transport-backed `lift`
correspondences. The manager owns comparison and policy; neither entrance
admits a package or executable operation.

The canonical review schema is version 130, row schema version 88, and
canonical-row recovery envelope version 23. Exact vocabulary and revision
notes live in
[`EVIDENCE_SCHEMA.md`](EVIDENCE_SCHEMA.md).

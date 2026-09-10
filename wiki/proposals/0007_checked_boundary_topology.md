# 0007: Checked boundary topology

Status: proposed, not ratified or implemented. This document proposes a complete
first semantic slice, including its restrictions and failure behavior. Examples
use candidate library APIs, not existing deployment APIs or new Omega grammar.
No experiment or deployment-enforcement result is claimed.

## Problem and decision

An application owner wants API, Authorization, and Billing developed and built
independently, then connected so API cannot invoke Billing without going through
Authorization. Adding an alternate connection must fail before installation.
This should be a checked property, not a package-layout convention.

Propose an ordinary services library used from a composition project's
`build.omg`. It builds a finite service-instance graph from verified component
interfaces, records owner-selected policies, and publishes a checked deployment
plan. Installation must establish that the running connections refine that plan.

Keep `reaches` unchanged. It reports service sets; it does not retain path order,
instance identity, or intermediate participants. Do not introduce nested traits,
`reaches Billing through Authorization`, or graph restrictions inside every
machine. No application concept such as Billing becomes compiler vocabulary.

The first slice checks possible invocation/message-delivery routes between
component instances. It is not an information-flow, request-authorization,
termination, availability, or distributed-consistency theorem.

## Ownership

| Owner | Responsibility |
| --- | --- |
| Service package | Ordinary requirement traits, checked implementations, imports/exports, protocol and resource contracts. |
| Composition project's `build.omg` | Exact component selections, instance names, endpoint bindings, transport selections, and required policies. |
| Services library | The author-facing builder, graph algorithms, named policy machines, diagnostics, and protocol-specific adapters. |
| Psi | Verify source authority/call closure and retain independently checkable component interfaces. No deployment or network execution. |
| Portable artifact verifier | Reconstruct endpoint inventory and authority closure; validate imported component evidence and graph construction. |
| Omega composition/publication | Sequence component admission, canonical graph construction, policy evaluation, and plan publication. No policy-specific graph algorithm duplicated in Rust. |
| Installer and selected runtime/OS providers | Establish exact artifacts, endpoint identities, confined transport, and binding lifetime before admitting activation. |

The first-party services package provides the reference implementation. Third
parties may provide different checked policies and transports through the same
boundary. No plugin loader or service-specific compiler registration is required.

## Existing owners and required additions

These are integration boundaries, not arguments that the feature already exists:

- [Build declarations](../spec/build/declarations.md) and
  [execution](../spec/build/execution.md): ordinary build machines, admitted host
  observations, source snapshots, and publication custody.
- [Service reach](../spec/language/effects.md): service identities and transitive
  summaries. A row cannot answer which route exercised a service.
- [Component publication](../spec/build/component_publication.md): closed
  imports/exports, service bindings, installed envelopes, and replacement.
- [Authority](../spec/resources/authority.md): endpoint authority must be
  established, not fabricated by copying fields or deserializing bytes.
- [Provider containment](../spec/build/provider_selection.md#executable-trust-and-containment)
  and [permissions](../spec/build/permissions.md): opaque execution and physical
  authority remain visible. A Network class is not endpoint confinement.
- [Psi publication](../spec/terminal-psi/product.md): separate consumption and
  independent reconstruction, not trust in a producer-written graph.
- [Protocol proofs](../spec/language/concurrency.md#protocol-proofs): reach is not
  liveness; richer event-model extraction is separate from this proposal.

Required additions are a closed endpoint/authority inventory, a build-owned
composition result, replayable library-policy evaluation over a verified graph,
and installation binding/confinement evidence. Existing component declarations
and carriers should be extended, not copied into a parallel component system.

## Authoring surface

Composition is an application role, not a fourth package/workspace role.
`builder.application` and dependencies retain their direct-discovery rules.
The composition-only output is a deployment plan; it need not invent a native
ProgramEntry merely to publish that plan. Ordinary applications may also attach
a composition plan to their outputs. This is a proposed build-product extension.

Illustrative `build.omg`:

```omega
machine build(builder: &mut Build) {
    builder.application("payments-deployment");

    let plan = topology.begin(builder);

    let api_code = topology.component(builder, "artifacts/api.psi");
    let auth_code = topology.component(builder, "artifacts/auth.psi");
    let billing_code = topology.component(builder, "artifacts/billing.psi");

    let api = plan.instance(api_code, "public-api");
    let auth = plan.instance(auth_code, "authorization");
    let billing = plan.instance(billing_code, "billing-primary");

    let api_auth = plan.import<Authorize>(api, "authorization");
    let auth_entry = plan.export<Authorize>(auth, "requests");
    let auth_billing = plan.import<Charge>(auth, "billing");
    let billing_entry = plan.export<Charge>(billing, "requests");

    plan.connect(api_auth, auth_entry);
    plan.connect(auth_billing, billing_entry);

    plan.only_via(api, billing, auth);
    plan.finish(builder);
}
```

The example abbreviates checked result handling. The API's proposed contracts
are below; implementations must not silently replace fallible steps with traps,
implicit unwraps, hidden allocation, or ambient filesystem access. Library
names/imports and ordinary storage provisioning are omitted from the sketch.

| Operation | Proposed contract |
| --- | --- |
| `begin` | Create activation-local composition state under the supplied Build authority and explicit storage provision. Register a pending required product, without retaining an exclusive loan of Build across later calls. |
| `component` | Read through admitted BuildSource, freeze exact bytes, and independently admit a component. Returns checked success/error data. Paths are locators, never evidence. |
| `instance` | Add one instance of admitted code; duplicate names or foreign-build component handles reject. Multiple instances may share code but not mutable state or instance identity. |
| `import<R>` / `export<R>` | Resolve one public endpoint label within the exact component interface; check its direction and complete closed requirement application against `R`. Return `Import<R>` / `Export<R>` or a checked error. |
| `connect` | Record a directed binding between same-plan endpoints. Check contracts and the selected transport realization, not just equal `R` spelling. |
| `only_via` | Record the reference library's named policy and its exact instance arguments. It does not evaluate the unfinished graph. |
| `finish` | Freeze the plan, admit all dependencies, evaluate all registered policies, and attach the result to Build output. Consumes the plan; no activation occurs. |

Endpoint labels are public interface lookup labels, not global names or identity
proofs. Resolution replaces them with exact slot identities. Renaming a public
label is an interface change; source paths and display names are not contract
identity. An artifact advertising two endpoints with the same label/direction
rejects. A component cannot be inspected to access private declarations.

Build-owned handles and scratch cannot escape evaluation. `finish` publishes an
owned immutable result, not pointers into compiler state. A failed or unfinished
composition registers no successful product. The build's normal result/report
path propagates errors; ignoring a mandatory composition failure cannot publish
the application as composition-checked. This is a required Build-output contract,
not a dependency on accepting a new assertion keyword.

Separately built application artifacts may be read from admitted immutable build
inputs; they do not become source dependencies on application-role packages.
Artifact compatibility never requires merging their source trees. Component
packages may alternatively produce the same admitted inputs through existing
build staging. Generated artifacts cannot alter the build's already-admitted
source or dependency discovery.

## Component contract and identity

An admitted component supplies the following facts, reconstructed from its
portable semantics, selected boundary obligations, and retained evidence:

1. Exact component semantic subject, requirement/schema versions, and public
   endpoint inventory. Each endpoint binds a nominal slot, direction, complete
   requirement application, contract, and source attribution where retained.
2. Every executable entry: exported requests, startup, timers, callbacks, and
   externally registered roots. An undeclared background entry is not harmless.
3. All outgoing communication authority, including possible callback delivery,
   native/foreign provider effects, and authority reachable from stored state.
4. Endpoint movement/duplication constraints and the lifetime of each binding.
5. A completeness verdict plus exact checked or explicitly admitted assumptions
   relating code and physical mechanisms to that inventory.

Source correspondence and transport/installation facts have separate identities.
Different native realizations of the same component are permitted only with the
ordinary semantic correspondence or explicitly accepted opaque-code contract.
An executable filename, digest, reproducible build, or signature alone does not
prove the inventory complete.

Within a candidate plan, `InstanceKey = unique authored instance name` and
`EndpointKey = (InstanceKey, exact component slot/application identity)`.
Canonical records using those local keys bind all exact component subjects;
hashing the complete records then establishes the plan subject. External
references use `(plan subject, local key)`. The subject never occurs inside its
own hashed records. An installed occurrence additionally binds the installation
generation, so two installations of one plan do not share runtime authority.
Names are scoped selectors backed by commitments, not global authorities.
Changing code or bindings creates a different deployment subject without
pretending it is a new source-language trait.

## Graph semantics: deliberately conservative

The v1 graph has one vertex per component instance, including explicitly modeled
external participants. An invocation import bound to an export creates a directed
edge from the importing instance to the exporting instance. Message delivery and
callback invocation grants create edges in their actual delivery direction.
Each edge retains endpoint, contract, binding, and transport attribution.

Treat every granted outgoing endpoint as usable from every entry and state of its
component. This intentionally overapproximates causal behavior. No per-function
path extraction, request correlation, dynamic condition solving, or inspection of
private branch patterns is needed. It can reject an architecture whose safety
depends on internal routing partitions: split it into separately confined
components or use a future separately justified more precise analysis.

The claimed property is about **permitted communication paths**, not which path
a particular request actually takes. For every admitted runtime component-to-
component invocation/message event, the corresponding edge must exist in the
graph. Then any sequence of such edges is covered by its path policies.

There are no implicit connections from equal traits, package imports, matching
strings, physical co-location, or shared brokers. Matching endpoint contracts
establish compatibility, not a connection. Synchronous returns are part of their
request channel, not a new invocation grant. Reverse callbacks require explicit
reverse edges. Consequently these policies do not assert that data can flow only
along the directed edges; response-data confidentiality is outside their meaning.

Every demanded import must have exactly one binding. Duplicate bindings reject;
v1 has no last-write-wins, load balancing, optional disconnected imports, or
implicitly selected default connection. A router is an ordinary explicit
component with its own imports. Multiple endpoint edges may connect the same
pair of instances; path checking may deduplicate adjacency while retaining all
binding evidence. Self-connections are allowed and remain visible.

The first slice uses a fixed finite instance roster. Runtime instance creation,
discovery, arbitrary endpoint delegation, and hot rebinding are not silently
modeled by a wildcard vertex. They require a newly checked plan. Static finite
replica sets can be enumerated in build code; policy selectors resolve to that
exact roster and are frozen before checking.

## Policy contracts

Policies are ordinary selected library machines over the immutable checked graph.
They execute under hermetic evaluation: no host observation, runtime authority,
mutation of the graph, or dependence on evaluator traversal order. They must
terminate within the verifier's admitted resources. The plan records exact policy
code, arguments, and dependencies; a stored success flag is not evidence.

The reference library supplies these operations with the following semantics:

| Policy | Required property |
| --- | --- |
| `no_route(S, T)` | No directed finite path from any instance in S to any instance in T. |
| `require_route(S, T)` | For every instance in S, at least one instance in T is reachable. This asserts possible connectivity, not runtime progress. |
| `only_via(S, T, V)` | At least one S-to-T path exists, and every S-to-T path visits an instance in V. |
| `only_via_ordered(S, T, [V1, ..., Vn])` | At least one S-to-T path exists, and every such path visits the required groups in that order. Other intervening instances are permitted. |

Selectors are explicit instance sets; a helper can construct them by component
identity or owner-authored labels. Empty sets, nonexistent members, foreign-plan
handles, overlapping source/target/via sets, empty ordered lists, and repeated
ordered groups reject instead of vacuously satisfying a routing requirement.
Rejecting overlaps bounds the first API; it does not claim graph theory cannot
define them. `only_via` explicitly requires a path so disconnected deployments
cannot accidentally pass. Deliberate disconnection uses `no_route`.

Path checking includes cycles, is independent of runtime simulation, and never
enumerates arbitrary-length paths. `only_via` checks reachability in the full
graph and non-reachability after removing V. The ordered form uses a finite
product graph with an index for the next required group and a rejecting state
for an out-of-order visit. Visiting an already-passed group is allowed; arriving
at T before completing the list rejects. These definitions specify ordered
traversal, not necessarily direct adjacency between the intermediaries.

The reference implementations require checked correctness against these
definitions. Ordinary BFS/worklist implementations and compact adjacency storage
are sufficient; no SAT/SMT subsystem or compiler-owned graph query language is
required. Custom policies are explicitly owner-selected programs, not ambient
discovery. A policy that simply returns success establishes only its own trivial
predicate, not the reference `only_via` guarantee.

Every registered policy must succeed. Policy order cannot affect graph identity
or the set of verdicts. Multiple policies combine by conjunction. Negation and
Boolean combinations can be ordinary library computations, but this does not
add Boolean `reaches` expressions to machine contracts. The reference routing
operations above are the first supported author-facing policy set.

Policy programs consume a stable ordinary graph value, not compiler pointers.
The reference result is `Satisfied` or `Violation` carrying the policy identity
and diagnostic witness. Failure to execute/verify is a separate unsuccessful
composition outcome. The independent consumer obtains the policy program from
its exact admitted artifact, not a name resolved against its own library version.
Build registers the required policy set under the root owner's authority. Omitting
a policy changes owner intent and the plan; the checker cannot infer a business
rule the owner never supplied. That remaining policy-authoring responsibility is
not the same as relying on developers to obey a supplied rule.

## What makes the graph trustworthy

A library-created graph is not automatically an executable program model.
The independent composition validator must reconstruct the endpoint inventory,
the complete outgoing authority set, and every binding before policy execution.
It must reject extra unaccounted communication paths, not merely verify each
listed edge. Completion uses the conservative all-entries/all-imports model,
so it does not need to trust a library assertion about internal call paths.

Application components receive only their bound endpoint capabilities. They
cannot construct a raw endpoint, change its target, leak one to an unmodeled
participant, or acquire a broader network/process/shared-memory capability without
invalidating this profile's confinement admission. Integer descriptors, a Network
reach class, or a domain name do not establish object-level confinement.

Opaque in-process code, raw networking, dynamic loading, shared mutable channels,
filesystem mailboxes, inherited handles, and external callbacks must be covered
by a checked narrowing contract or explicitly admitted enforcing containment.
Unknown authority rejects a topology-checked installation. A generic framework
wrapper cannot hide the broader underlying mechanism from provider review.

A multiplexed transport adapter may hold broad network authority only inside a
separately checked/admitted mediation boundary. Its contract must restrict each
client to the installed route table, authenticate endpoint/instance identity,
and prevent retargeting or exporting its underlying credentials. Shared transport
is not treated as an all-to-all application router only when that restriction is
established. Otherwise it is an explicit broadly connected participant or rejects.

Authority-bearing endpoints cannot appear in ordinary serialized payloads in v1.
Sending equal bytes does not transfer domain provenance, binding authority, or
linear custody. Sessions and callback handles needing additional connections
must use predeclared, accounted endpoints; unsupported dynamic delegation rejects.
Ordinary reply data remains governed by its type/codec contract.

This is endpoint confinement, not full covert-channel freedom. Shared clocks,
resource contention, and permitted ordinary response data can communicate facts
without a new endpoint invocation. No claim here excludes those channels. If
files, shared memory, or a broker are used as explicit application message
transports, their delivery authority must nevertheless be modeled; calling them
storage instead of networking cannot evade the graph.

## Cross-binary transport and request authorization

A wire binding checks complete operation/schema identity and an explicitly
selected codec/adapter. Equal method names, argument sizes, or nominal trait
spellings do not establish wire compatibility. A native pointer/borrow ABI is
not a wire protocol. The adapter must justify serialization, error outcomes,
ownership transfer, cancellation, retry, and lifetime behavior for the specific
requirement. No universal remote-call equivalence is assumed.

Network failure is an ordinary contracted outcome. Reconnection may restore only
the same admitted logical binding; redirects, discovery, or failover to another
instance require re-admission. Duplicate delivery and reordering follow the
selected protocol contract; topology checking supplies no exactly-once guarantee.

Passing through Authorization does not prove a payment was approved. A separate
library contract should make `Charge` require evidence for the exact account,
amount, operation identity, and any relevant expiry/replay policy. Only authorized
issuer routes establish it. At a remote arrival, authenticated protocol validation
must establish the local qualified value anew. Compile-time erasure does not
authenticate a message; deserialization is not evidence introduction.

The same distinction applies to privacy. Returning a String derived from a
secret, or choosing a public reply based on a secret, is not prevented by a
connection graph. Information-flow control, declassification, timing channels,
and noninterference are not implied or claimed by this proposal.

## Publication and installation

The pipeline is sequential:

```text
admitted component artifacts + admitted build inputs
    -> frozen instance/binding graph
    -> independently checked graph + successful policy evaluations
    -> published deployment plan
    -> installation evidence for exact artifacts and connections
    -> permitted activation
```

Build execution constructs the request; the ordinary publication coordinator
owns validation and output. Policy checks happen after the graph is complete,
not during each `connect`. No partial successful deployment artifact is emitted.
Plan generation does not contact services, install software, acquire credentials,
or grant runtime authority. Those actions need separately admitted installer
authority, even if one CLI later sequences both operations.

The canonical plan is an owned companion to the component products, not target
deployment facts inserted into Psi's executable semantics. It retains:

- Exact semantic-schema identity and all component/evidence dependencies.
- Frozen instance roster and public endpoint bindings.
- Selected codecs, transports, placement/confinement requirements, and provider
  assumptions. Physical addresses may be supplied at installation under those
  constraints; logical targets cannot change.
- Exact policy programs/arguments and evaluation evidence.
- Artifact-correspondence requirements and installation/replacement envelope.

Use the existing canonical artifact/evidence discipline: stable semantic IDs,
canonical ordering by instance/endpoint keys, and exact versioned field/tag
definitions. Duplicate or unknown entries reject; no timestamps, host paths, or
allocator IDs enter semantic identity. Debug labels and source attribution are
separate. The implementation must provide encoder/decoder fixtures before
claiming portable publication; this proposal does not assign ad hoc wire numbers
outside that owner. A schema mismatch requires reconstruction and fresh admission,
not guessed compatibility or a trusted JSON success report.

An installer independently validates the plan, reconstructs the graph, and
reruns the selected terminating policies or checks equivalent evidence through
the existing proof verifier. It then installs exact code and endpoint bindings,
establishes provider/OS confinement, and admits activation. Its receipt binds the
plan, actual artifact identities, physical endpoint mapping, confinement/provider
evidence, execution domain, and installation generation. A signature can identify
an accepted issuer but cannot replace semantic or containment validation.

Expose two distinct results: `composition checked` and `installation admitted`.
The first can be produced on a build host without network access. Only the second
supports the runtime claim, conditional on the named OS/transport/hardware trust.
Unknown, unsupported, resource-exhausted, and assumption-unaccepted outcomes never
become either success verdict by omission.

Activation is gated on successful admission, not necessarily on transactional
filesystem creation. On failure, release acquired endpoints/storage through
ordinary cleanup and do not activate the new installation. Already published
installations remain separate and unchanged. The v1 replacement procedure stops
and quiesces the affected installation, checks the new plan, and starts the new
generation. It claims no uninterrupted service or state migration. Existing
independent-component machinery may later justify narrower live replacement,
but simultaneous old/new graphs must then satisfy an explicitly checked
coexistence policy; two individually safe plans need not have a safe union.

## First executable realization

The first cross-binary customer uses three local processes connected by
installer-created private pipes, not an unspecified cloud orchestrator. The
same logical plan must work on Windows and macOS through target-owned adapters:
anonymous pipe handles on Windows and private pipe descriptors on macOS. The
installer supplies only the explicitly assigned endpoints to each process and
closes its unused duplicates. General handle inheritance is disabled.

Use one dedicated request/response pipe pair per binding and one outstanding
request per binding. No broker, global endpoint lookup, DNS, open listener,
connection pooling, automatic retry, or callback transfer is needed for this
acceptance. Replies stay on the binding's response channel. The selected codec
uses a bounded frame and exact operation schema; invalid length, operation,
payload, premature EOF, or peer failure produces an explicit protocol/transport
failure and closes that binding. No reconnect to a different peer is attempted.

The three component programs have checked code and only these scoped transport
imports. The trusted installation/OS contract establishes process/executable
identity, private-handle mapping, and startup admission. Target pipe primitives
and any unverified loader behavior are named provider assumptions. This profile
does not claim to confine arbitrary malicious native code that can invoke host
syscalls outside the checked closure; such code requires separately established
OS sandboxing or rejects. A broad assumption that "the application obeys the
diagram" is not acceptable in place of the checked closure.

The installer first validates all products and allocates the endpoints. Application
entry is held behind the runtime's installation gate until the exact mappings
and authority are admitted. Failed setup closes resources and leaves no admitted
new application activation. Tests must attempt an ungranted endpoint invocation
and substituted pipe mapping, not merely draw the intended graph.

This route chooses a concrete minimum while preserving the ownership split:
pipe/codec adapters are library/provider code; the compiler does not acquire
API/Auth/Billing concepts. Remote encrypted transport is a different selected
realization, requiring authenticated peer/binding identity and the same authority
constraints. It is not credited as implemented by the local-pipe test.

## Diagnostics and acceptance

Errors identify instance, endpoint, component subject, policy, and the relevant
authored binding/source span where available. A bypass reports a concrete path,
including cross-binary bindings. The reference checker chooses a shortest witness
with canonical-key tie breaking; it checks all policies in canonical order.
Unavailable/unknown confinement reports the unaccounted authority, not a fabricated
path. Internal contradiction remains distinct from invalid composition or an
unsupported provider/profile. This proposal adds no arbitrary compiler reason codes.

Required acceptance cases:

| Case | Expected result |
| --- | --- |
| API -> Authorization -> Billing, all imports accounted | Composition succeeds; installation still requires enforcement evidence. |
| API -> Billing bypass added | `only_via` rejects with the bypass path. |
| Indirect bypass through a logging/helper service | Reject; every component import participates, not only request handlers. |
| Same package instantiated twice, one allowed and one forbidden route | Distinguish instances; do not merge by code or trait identity. |
| Two compatible endpoint types with no connection | No edge invented; an unbound demanded import rejects. |
| Disconnected S/T, empty selector, overlapping via set | Reject the misleading `only_via` policy input/result. |
| Cycles and ordered intermediaries | Terminating check; reject an out-of-order or avoiding path. |
| Broad transport hidden behind an ordinary wrapper | Reject without exact mediation/confinement evidence. |
| Raw endpoint fabrication, serialized endpoint, unlisted callback | Reject; no implied delegation or reverse connection. |
| Modified component, policy, binding, codec, or transport after check | Reject stale evidence; reconstruct against the new subject. |
| Instance declaration order or binding order changed | Same normalized semantic graph and policy result. |
| Cross-machine consumer with no source trees | Reconstruct and check from admitted artifacts and owned plan. |
| Wrong runtime peer, substituted native code, insufficient confinement | Installation rejects even if the proposed graph passes. |
| Check succeeds but installation fails partway | No admitted new activation; ordinary cleanup, no false success receipt. |
| Co-located vs isolated vs remote realization | Same logical policy, separately justified enforcement/transport contracts. |
| Charge approved for another amount / replayed request | Authorization protocol rejects independently of a passing topology check. |
| Secret-dependent reply over a permitted route | Not claimed safe by topology; no false confidentiality verdict. |

Test on supported developer hosts; mark remote/OS enforcement tests unavailable
when their providers are absent, not passed by a graph-only test. The first end-
to-end customer is the three-component payment example, including an actual
blocked bypass at the selected runtime boundary. A graph helper test alone does
not close it.

## Alternatives and scope settlement

| Alternative | Assessment |
| --- | --- |
| Package discipline and manual reach audits | Useful today, but insufficient for instance-specific enforced wiring. |
| Capability construction without graph policies | Valid when the permitted construction itself expresses the complete restriction. Keep it as the simpler option for small systems. Graph policies earn their place when independent wiring edits must preserve a global rule. |
| Compiler-owned topology keywords and graph algorithms | Viable but unnecessary for this slice: ordinary build calls and checked policy machines suffice. The compiler still owns model fidelity. |
| Precise per-entry internal dependency summaries | Viable later if conservative instance graphs reject a concrete needed architecture; not required to prove this bounded claim. |
| Arbitrary program/network topology extraction | Much larger than the construction-controlled customer; not the first implementation. |
| Just add `reaches A through B` or Boolean service rows | Wrong abstraction: service-set summaries have discarded paths and instance identities. |
| Trust library-authored endpoint inventories or success receipts | Unsound: a correct graph algorithm can prove a false model. |
| Treat same trait/schema as an automatic network route | Confuses compatibility with installed connectivity. |
| Put transport credentials or mutable deployment state in compile-time constants | Violates authority, evaluation, and installation separation. |

The proposal chooses the conservative finite graph, ordinary library policies,
explicit build composition, and separate installation admission. It does not
leave syntax alternatives or enforcement to an unspecified future framework.
It deliberately excludes dynamic topology, arbitrary endpoint transfer, detailed
internal causality, confidentiality, and availability guarantees. Unsupported
uses reject rather than weakening the stated guarantee.

Remaining implementation work, contingent on acceptance: finish component
closure/evidence support, add the owned composition build result and exact
codec, implement the reference policy library, integrate publication replay,
and implement one real contained transport/installation route. Do not add these
as execution-board prerequisites before ratification. No known owner choice is
left unanswered within this slice; newly discovered semantic conflicts belong
in [OWNER_QUESTIONS.md](../../OWNER_QUESTIONS.md), not silent implementation policy.

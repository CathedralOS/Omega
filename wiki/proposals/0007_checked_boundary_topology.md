# 0007: Checked boundary topology

Status: proposed, not ratified or implemented. This is a reference-package design
and a customer for [0008: Build dependency scopes and isolated inputs](0008_build_dependency_scopes.md),
not a compiler-owned topology subsystem. Examples use candidate library/build
APIs, not existing deployment APIs or new Omega grammar. No experiment or
deployment-enforcement result is claimed; open integration questions remain below.

## Problem and decision

An application owner wants API, Authorization, and Billing developed and built
independently, then connected so API cannot invoke Billing without going through
Authorization. Adding an alternate connection must fail before installation.
This should be a checked property, not a package-layout convention.

Propose an ordinary topology package used as a build-only dependency. It builds a
finite service-instance graph from verified component interfaces, checks
owner-selected policies, and serializes a package-defined deployment plan through
generic staged output. Its verifier and installer are ordinary checked programs
using existing component and provider admission. Installation must establish that
the running connections refine the checked graph. Build publication alone does
not establish either model completeness or runtime confinement.

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
| Composition project's build entry and helpers | Exact component inputs, instance names, endpoint bindings, transport selections, policies, and a required artifact output. |
| Topology package | Builder, graph algorithms, policy contracts, diagnostics, artifact schema/codec, and plan verification. |
| Existing Psi/component verifier | Establish component identity, complete interfaces, and authority/call closure under the existing component contract. No graph policy or deployment execution. |
| Generic Build facilities from 0008 | Admit build-only code, expose scoped inputs, account for required outputs, and publish exact bytes with provenance. No topology-aware success verdict. |
| Package-owned installer and runtime adapters | Verify the plan, select admitted providers, establish bindings, and sequence activation/cleanup under existing installation rules. |
| OS/transport providers | Supply exact confinement and endpoint identity guarantees, with checked or explicitly accepted assumptions. |

The reference package has no privileged status. Third parties may supply checked
policies, codecs, transports, and installers under the same general component
contracts. No plugin loader, topology-specific Build field, policy registry, or
service-specific compiler registration is required. Missing generic component
facts belong in their existing owners, not a parallel topology evidence system.

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

0008 owns the proposed build-only dependency context, isolated inputs/staging, and
required artifact publication. Existing component publication already requires
closed imports/exports, all entries, outgoing authority, and installation
obligations. This package consumes those facts; their current specification is
not a claim that every producer or public consumer API is implemented.

Start with explicitly supplied verified component descriptions. If an essential
fact cannot be obtained or replayed, identify that exact missing general-purpose
API and its independent customer before extending the compiler. Do not add a
topology pipeline stage, a second component census, or a mandatory graph-shaped
record to Terminal Psi merely to host this package.

[Reflection](../spec/language/reflection.md) can reduce typed endpoint/schema
boilerplate for explicitly authorized types. It is optional: explicit endpoints
and verified artifact descriptions suffice for the first slice. Reflecting type
fields or a requirement does not prove that a binary has no other communication
authority, grant access to private code, or establish installed endpoint identity.

## Authoring surface

Use an ordinary application build with the generic artifact-only output intent
from 0008, or attach the plan as a required companion to an executable. No
composition package role or topology-specific output kind is introduced.
The helper package is a build dependency, not an implicit product dependency.
Its revision remains part of build provenance; nothing here makes it callable
from application source or selects it as a runtime provider.

Illustrative root `build.omg` (candidate APIs and abbreviated error handling):

```omega
use build_support::deployment;

machine build(builder: &mut Build) {
    builder.application("payments-deployment");
  builder.build_depend_as("topology", Source::Path {
    location: "../topology"
  });
  builder.artifact_only();

  let inputs = deployment_inputs(&mut builder.source);
  let output = builder.output.require("payments.plan");
  compose_payments(inputs, output);
}
```

The local deployment helper imports `topology::policies` in its own source file.
`deployment_inputs` captures the three exact component artifacts from permitted
snapshot inputs and supplies explicit policy/storage configuration. The topology
helper receives those values and one output obligation, not the root builder,
resolver credentials, or general host filesystem access. Extra runtime transport
packages are declared independently by their actual consumers. Package builds do
not get installer authority merely because their output describes installation.

The package algorithm is ordinary code, schematically:

```text
admit api, authorization, and billing component descriptions
create three distinct instances
bind API.authorization to Authorization.requests
bind Authorization.billing to Billing.requests
require only_via(API, Billing, Authorization)
freeze the graph and verify the complete bindings and policy
serialize the verified result and complete the supplied output
```

Public labels resolve to exact requirement applications from admitted interfaces.
Typed convenience methods may name `Authorize` and `Charge` through separately
declared protocol dependencies; equal names are never compatibility evidence.
Storage sponsorship and checked error propagation are omitted only from the
sketch, not the contract. No implicit unwraps or ambient filesystem access follow.

| Operation | Proposed contract |
| --- | --- |
| `begin` | Create ordinary package-owned graph state under explicit storage provision. No Build authority or global compiler state is required. |
| `component` | Consume supplied immutable artifact bytes through the existing component verifier. Return an owned verified description or checked error; paths and caller-written metadata are not evidence. |
| `instance` | Add one instance of admitted code; duplicate names or unadmitted component handles reject. Multiple instances may share code but not mutable state or instance identity. |
| `import<R>` / `export<R>` | Resolve one public endpoint label within the exact component interface; check its direction and complete closed requirement application against `R`. Return `Import<R>` / `Export<R>` or a checked error. |
| `connect` | Record a directed binding between same-plan endpoints. Check contracts and the selected transport realization, not just equal `R` spelling. |
| `only_via` | Record the reference library's named policy and its exact instance arguments. It does not evaluate the unfinished graph. |
| `finish` | Freeze the graph, validate complete bindings, and evaluate registered policies. Return an owned package result or error; no Build publication or activation occurs. |
| `publish` | Serialize a successful package result into the supplied generic output obligation. Build records bytes/provenance, not a topology theorem. |

Endpoint labels are public interface lookup labels, not global names or identity
proofs. Resolution replaces them with exact slot identities. Renaming a public
label is an interface change; source paths and display names are not contract
identity. An artifact advertising two endpoints with the same label/direction
rejects. A component cannot be inspected to access private declarations.

Package-owned results contain no pointers into compiler state. A failed or
unfinished composition cannot construct the package's checked result through its
normal API. The generic required-output obligation prevents silently dropping a
requested plan and reporting successful completion. Neither mechanism makes
arbitrary serialized bytes authoritative: the independent package verifier and
installer must reject forged success, omitted owner-required policies, and stale
evidence. No compiler-specific composition verdict or new assertion keyword is
needed.

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

The first reference-package slice supplies two operations:

| Policy | Required property |
| --- | --- |
| `no_route(S, T)` | No directed finite path from any instance in S to any instance in T. |
| `only_via(S, T, V)` | At least one S-to-T path exists, and every S-to-T path visits an instance in V. |

Selectors are explicit instance sets; a helper can construct them by component
identity or owner-authored labels. Empty sets, nonexistent members, foreign-plan
handles, and overlapping source/target/via sets reject instead of vacuously
satisfying a routing requirement.
Rejecting overlaps bounds the first API; it does not claim graph theory cannot
define them. `only_via` explicitly requires a path so disconnected deployments
cannot accidentally pass. Deliberate disconnection uses `no_route`.

Path checking includes cycles, is independent of runtime simulation, and never
enumerates arbitrary-length paths. `only_via` checks reachability in the full
graph and non-reachability after removing V. This covers indirect routes and
cycles without interpreting component bodies. Ordered intermediary groups,
connectivity queries, richer Boolean policy combinators, and more precise
entry-specific analysis can be independent package extensions when a customer
needs them; none is a compiler primitive or prerequisite to the first slice.

The reference algorithms require correctness evidence against these definitions
under the ordinary proof/admission rules. Running a typechecked BFS and receiving
success is not itself a proof that its implementation decides `only_via`.
Ordinary worklists and compact adjacency storage suffice for the algorithm; no
SAT/SMT subsystem or compiler-owned graph query language is required. Custom
policies are owner-selected programs, not ambient discovery. A policy that simply
returns success establishes only its own trivial predicate, not the reference
guarantee. Missing or assumed algorithm correctness must remain explicit.

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
The root author supplies the required policy set to the package. The installer
joins the plan to independently accepted owner intent, not a requirement set that
an untrusted plan writer may choose for itself. Omitting a policy changes that
intent; a valid plan for a weaker policy is not sufficient. Build provenance helps
identify inputs and outputs but does not infer a missing business rule.

## What makes the graph trustworthy

A library-created graph is not automatically an executable program model.
The package's independent plan verifier obtains the endpoint inventory and
complete outgoing authority from admitted component descriptions, then reconstructs
every binding before policy execution.
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

The topology package constructs and validates its plan. Generic Build publication
records completed staged output; it does not reconstruct the graph, select policy
code, or rerun topology algorithms. Policy checks happen after the graph is
complete, not during each `connect`. No partial successful deployment artifact is
emitted through the package's publication API.
Plan generation does not contact services, install software, acquire credentials,
or grant runtime authority. Those actions need separately admitted installer
authority, even if one CLI later sequences both operations.

The package-defined plan is an ordinary owned artifact, optionally a companion
to component products, not a new compiler product kind or target deployment facts
inserted into Psi's executable semantics. Its versioned schema retains:

- Exact semantic-schema identity and all component/evidence dependencies.
- Frozen instance roster and public endpoint bindings.
- Selected codecs, transports, placement/confinement requirements, and provider
  assumptions. Physical addresses may be supplied at installation under those
  constraints; logical targets cannot change.
- Exact policy programs/arguments and evaluation evidence.
- Artifact-correspondence requirements and installation/replacement envelope.

The package codec follows existing artifact/evidence discipline: stable semantic IDs,
canonical ordering by instance/endpoint keys, and exact versioned field/tag
definitions. Duplicate or unknown entries reject; no timestamps, host paths, or
allocator IDs enter semantic identity. Debug labels and source attribution are
separate. The package must provide encoder/decoder fixtures before
claiming portable publication; it owns its wire numbers without a compiler schema
registry. A schema mismatch requires reconstruction and fresh admission,
not guessed compatibility or a trusted JSON success report.

The package-owned installer independently validates the plan, reconstructs the
graph, and reruns the selected terminating policies or checks equivalent evidence through
the existing proof verifier. It then installs exact code and endpoint bindings,
establishes provider/OS confinement, and admits activation. Its receipt binds the
plan, actual artifact identities, physical endpoint mapping, confinement/provider
evidence, execution domain, and installation generation. A signature can identify
an accepted issuer but cannot replace semantic or containment validation.

Expose distinct package results: `composition checked` and `installation admitted`.
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
same logical policy must work on Windows and macOS through target-owned adapters:
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
| Cycles and indirect avoiding paths | Terminating check; reject a path avoiding the required intermediary. |
| Broad transport hidden behind an ordinary wrapper | Reject without exact mediation/confinement evidence. |
| Raw endpoint fabrication, serialized endpoint, unlisted callback | Reject; no implied delegation or reverse connection. |
| Modified component, policy, binding, codec, or transport after check | Reject stale evidence; reconstruct against the new subject. |
| Writer omits an owner-required policy or supplies a trivial substitute | Installer rejects the mismatch with independently accepted owner intent. |
| Build helper tries to read home files or open a network connection | No authority from the topology dependency; 0008's scoped execution refuses. |
| Plan file is published with a forged success flag | Publication is not approval; the package verifier rejects invalid evidence. |
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
| Compiler-owned topology products, keywords, and graph algorithms | Not justified: use 0008's generic build outputs and package code. Component evidence remains owned by existing component verification. |
| Reflection-driven graph discovery | Optional convenience for authorized schemas, not evidence of complete executable authority or installed wiring. |
| Precise per-entry internal dependency summaries | Viable later if conservative instance graphs reject a concrete needed architecture; not required to prove this bounded claim. |
| Arbitrary program/network topology extraction | Much larger than the construction-controlled customer; not the first implementation. |
| Just add `reaches A through B` or Boolean service rows | Wrong abstraction: service-set summaries have discarded paths and instance identities. |
| Trust library-authored endpoint inventories or success receipts | Unsound: a correct graph algorithm can prove a false model. |
| Treat same trait/schema as an automatic network route | Confuses compatibility with installed connectivity. |
| Put transport credentials or mutable deployment state in compile-time constants | Violates authority, evaluation, and installation separation. |

## Iteration and open integration questions

Keep the finite graph and the two routing policies as the first package slice.
Do not expand into distributed orchestration, automatic discovery, hot replacement,
ordered-query languages, or information-flow analysis to justify compiler hooks.

The next design work is concrete:

1. Specify the ordinary verified-component description API needed by the package.
  Which existing component facts are available without source, and which exact
  missing facts require work in their current owner? Preserve independent replay;
  reflection or a producer-written endpoint list cannot replace completeness.
2. Settle 0008's build-only scope and generic output/failure interface. Demonstrate
  the same interface with a code generator, so topology is not its only rationale.
3. Define the package's smallest codec, checked graph result, and accepted
  owner-policy input. Choose replay or existing proof evidence with explicit
  algorithm assumptions; do not introduce a new trusted policy rule.
4. Exercise one actual private-pipe installation with an attempted bypass and
  substituted peer. Account for every provider/OS premise. Plan checking without
  installation evidence remains only plan checking.

These are proposal dependencies and integration questions, not assertions that
the APIs already exist. 0008 is a proposed prerequisite to this build integration,
not a ratified requirement on current packages. Keep the graph, codec, diagnostics,
and installer in package code; add only demonstrated general-purpose missing
capabilities through their existing spec owners. No implementation tasks or
execution-board prerequisites follow before ratification.

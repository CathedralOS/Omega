# Semantic reflection and typed visitation

Reflection describes an explicitly selected type and specializes ordinary checked
machines over its members. Libraries own inspection, encoding, editing, ordering,
and migration policy. No format-specific compiler operation, expansion keyword,
AST quotation, mandatory object header, or implicit runtime registry follows.
Implementation work is tracked in [TASKS.md](../../../TASKS.md#semantic-reflection).

## Semantic descriptions

`reflect::schema<T>()` produces an owned `reflect::TypeSchema` description graph
for the selected type application. Nodes describe declarations, fields, cases,
and their relationships; handles index only that owned graph. `schema.fields()`
and `schema.cases()` borrow description records from the schema value, never
compiler/evaluator storage. Recursive references are graph edges with their
substitutions, not an instruction to recursively expand every possible instance.

A field description retains its exact declaring owner/application/member,
complete declared type, relevance, case ownership, name, and optional authored
stable number. Complete type includes domain atoms, arithmetic policy, routed
requirements, indices, reference access, and lifetime relationships. Transparent
aliases use ordinary normalization; physical shape does not determine type
identity. Retired numbers remain descriptive history rather than live fields or
cases. Source order and stable-number order remain distinct.

The schema describes qualification requirements, not their establishment on an
instance. Describing `Granted` supplies no grant, and reflecting an erased proof
field supplies no proof. Actual projected values carry their own facts, loans,
and provenance under [domains](domains.md) and [dependent values](dependent_values.md).

Descriptions have no language-level 32-field or 4096-byte cutoff. Storage and
evaluation sponsorship remain explicit ordinary limits: inability to produce a
complete description reports exhaustion or unsupported production, not a
truncated schema. Current fixed capacities in `core::layout::Schema` do not define
the semantic reflection contract.

`reflect::type_key<T>()` provides a comparable descriptive key for a fully static
type application under ordinary canonical type equality. It is not a source type,
address, cast, or conformance. Runtime-dependent applications instead retain a
schema recipe with explicit subject/parameter references. No guessed Boolean
type equality follows from matching declaration names or symbolic descriptors.
Typed visitation of dependent values remains possible with their actual subject
and equality evidence; it does not require a fully static TypeKey for every field.

Closed descriptions and retained symbolic references have deterministic encodings:
versioned descriptor roles, exact owner/declaration/application correspondence,
and exact member identity. Graph-local indices are relocated/reconstructed within
their graph, not compared as cross-artifact identities. Replay checks the bound
declaration/schema revision and every referenced application. Fingerprints index
these records; a digest, offset, numeric key, or matching bytes alone authorizes
no operation. Display names and schema numbers are not alternative type identities.

## Schema and layout

`reflect::TypeSchema` describes semantic structure. `layout::Schema` is the
target-resolved input to a layout policy. A checked correspondence joins an exact
semantic member to the applicable physical field key and validated plan; it
rejects wrong owner, application, revision, relevance, or case mappings.
There is no second offset calculator or interpretation of a semantic member index
as its byte offset.

Geometry may be queried only after the selected [layout plan](../layouts/plans.md)
is validated. A layout cannot inspect its own unfinished result; dependency
cycles reject. Erased fields have no physical field key or access decision.
Target-independent description does not silently acquire target geometry
dependencies; consumers that use geometry retain them explicitly.

## Access owner and delegation

A compiler-known reflection occurrence elaborates queries and projections under
its lexical author's ordinary dependency and visibility authority for an explicitly
selected authorized T. It does not grant generic library code the caller's access
context. Carrying
a foreign T does not authorize selecting its fields, even when its owner
publishes structural data. [Package selection](../packages/boundaries.md) remains
separate from carrying a type.

An authorized owner can author a visitor or checked projection operation and
pass it through a callable requirement or explicit conformance. The receiving
library invokes that operation; it does not obtain permission to enumerate T or
recursively inspect the types of supplied fields. Recursive descent uses each
field type's explicitly selected operation. A metadata key or editable flag is
not a delegated accessor.

An initial schema query requires authorized complete structural visibility. It
does not hide inaccessible members while claiming complete coverage. Fields and
cases inherit their semantic owner's visibility; there is no new independently
private-field syntax. Public wrappers retain ordinary public-signature exposure
rules. Private realization identities in a selected conformance do not become
consumer-nameable declarations.

Semantic opacity remains stronger than equal layout. In particular, a quotient
does not expose its representative by reflection. Only an observer licensed by
its checked [quotient contract](../proofs/quotients.md#formation-and-representation)
may do so. Routed authority, private storage, and BindingPrivate operations keep
their respective access rules.

## Typed runtime visitation

`reflect::visit_runtime_fields<T, Visitor>` specializes the selected callback for
each immediate non-erased field of a record. It takes the actual borrowed value
and the named callback's ordinary explicit context. Calls use existing exact
requirement and generic-family matching; no anonymous expression or generated
capture environment is required. The following shows the elaboration, not a new
runtime interpreter:

```text
Player { health: u32; speed: f32; }

visit_runtime_fields<Player>(player, visitor):
    invoke<u32>(visitor, health_description, &player.health)
    invoke<f32>(visitor, speed_description, &player.speed)
```

The compiler resolves the member, exact owning application, actual complete field
type, callback selection, and ordinary typed projection. It checks every selected
application and generates ordinary calls before Terminal Psi. Per-member
specialization is semantics, not an optional optimization. A runtime loop over
homogeneous FieldInfo records cannot turn a record variable or TypeKey into a
generic type argument.

Runtime visitation covers all non-erased bindings, including zero-sized linear
or authority-bearing fields. Declaration inspection includes erased bindings too,
but they receive no runtime borrow. An empty record makes no field calls. The
default order is authored declaration order, not physical placement or stable
number order. Recursive descent and alternate order require explicit library
policy. An unsupported included field rejects with its path and failed obligation.

Each call obtains a valid field subloan and fresh reborrow of the context. These
invocation loans can end before the next call; loans stored in the explicit
context retain their own lifetimes. Shared inspection neither consumes nor copies
the record or its linear fields. References are not silently dereferenced into
recursive traversal. Retention needs a valid declared relationship; ordinary
escape rejection does not require general outlives-bound syntax.

Field type binding preserves all qualifications. Exact type equality is distinct
from argument compatibility. A selected adapter may use ordinary per-atom
weakening or explicit `as` erasure where allowed; the visitor never discards
qualifications to find a callback. Predicate knowledge may be forgotten under
ordinary rules, semantic/routed meaning needs the applicable explicit erasure,
and owned claims require disposition or transfer. A descriptive encoder may
borrow an owned claim without consuming it; encoding its bits does not transfer
custody or enable reconstruction of its authority.

## Sum coverage

A sum visitor has an active-case callback and field callbacks. It first reports
the selected semantic case exactly once, even for a case with no payload. It then
visits common runtime fields once and the active payload's runtime fields, each
in declaration order. Two nullary cases therefore remain distinguishable. A
record visitor alone is not a complete sum serializer.

Compile callback applications for every admitted case; execution projects only
the selected payload after testing the actual discriminator. Semantic case
identity, authored wire numbers, and runtime discriminator values remain distinct;
the encoding library chooses its wire tags. Active-case evidence stays valid
while payload loans are live. Retired cases are metadata, not executable cases.

Partial consumers may use an explicitly declared ordinary result to stop or omit
members. Stopping follows ordinary return/loan rules, not a nonlocal return from
the callback's enclosing author. A full-coverage operation cannot report success
after silently excluding fields or cases.

## Explicit operation selection

Encoding/inspection policy is an explicitly selected ordinary machine specialized
per discovered member. Its type rules choose reusable defaults; exact member
rules provide exceptions. A 40-field record with five field types need not author
40 selections. Generic adapter families may cover further declared applications
without enumerating each capacity or qualification combination.

For an encoding library, the policy shape is:

```text
encoding_policy<Value>(field: FieldInfo,
                       selection: &mut EncodingSelection<Value>)

    if field is the special timestamp member:
        selection.select<UnixTimestampEncoding>()
        return
    apply the explicitly selected type rules
```

FieldInfo describes the exact member; the compiler supplies Value statically.
EncodingSelection is a library-specific instance of a scoped typed selection
receiver, not a runtime conformance lookup object. `select` names an exact
conformance application or exact machine appropriate to the receiver's declared
requirement. An encoder data type alone is not a request to discover a map.
All generic conformance arguments retain their ordinary explicit-selection rules.

The receiver pins the member, qualified field type, and required operation.
Each included field has exactly one valid selection on every successful policy
completion. Missing and multiple selections reject; selecting twice is not
last-write-wins. Explicit policy failure need not fabricate a selection. A
partial-coverage contract may instead permit an explicit exclusion, with its
coverage retained. A skipped callback due to failure is not a successful omission.

These obligations may be expressed with ordinary state contracts or linear
selection obligations, but a mutable receiver alone does not establish them.
The finalized selection set is independently checked for coverage and full
requirement refinement, not merely a chosen operation's name/signature.

Authored control flow determines member-override precedence and ordering of
overlapping type rules. A table helper rejects duplicate member overrides and
defines its type-rule ordering or disjointness contract explicitly. There is no
ambient default, most-specific search, import-order precedence, or arbitrary
tie-break. The same carrier may use different encodings at different members.

Member keys originate from the authorized schema. An ordinary static name lookup
may find a member or return failure; subsequent selection retains the resolved
exact key, not the string as identity. This needs no field-declaration generic
binder such as `member<Player::health>()`. Display labels and policy flags remain
ordinary data. Forged or stale key/type pairs reject before projection.

## Selection evaluation and retained results

Selection uses the same staged per-member invocation machinery as encoding, with
different arguments and evaluation eligibility. The policy sees descriptions and
its static policy environment, not a future runtime object's contents. It selects
an encoder but does not execute runtime output or device access.

The typed selection operation is new compiler-recognized behavior, not provider
selection reused unchanged. It records an exact static declaration/application
choice inside the evaluation-local receiver. It cannot mutate global compiler
state, acquire Build authority, append provider-plan rows, or bypass visibility
and conformance checking.

The evaluation root owns the receiver, invokes the policy, and freezes its
completed choices into an ordinary owned symbolic result snapshot. The author's
policy need not return first-class conformance evidence. Ordinary hermetic
evaluation returns the snapshot; no mutable argument, evaluator reference, host
address, or out-of-band compiler mutation escapes. The records describe choices,
not proof by construction. Validation resolves their exact declarations and
rechecks member/type/requirement correspondence and authorized scope before
generating calls. Snapshot manufacture does not grant selection authority.

Policy code, static inputs, selections, target dependencies actually consulted,
and schema revision participate in retained derivation dependencies. Live runtime
context values cannot influence compile-time selection. No general facility for
returning runtime machine symbols or treating conformance evidence as ordinary
generic value atoms is introduced.

## Contracts, recursion, and resources

Every generated call retains ordinary preconditions, post-state, effects,
suspension/blocking acknowledgements, failures, crashes, and custody. A finite
field set does not prove callback termination. Fixed callback ceilings work
without variable [callback contract forwarding](../../../OWNER_QUESTIONS.md#q2--callback-contract-forwarding),
which remains a general owner question rather than a reflection-specific rule.

Recursive derivation is keyed by the exact type/application, selected operation
contract and realization, and static policy dependencies. Re-entry to the same
derivation refers to its pending identity rather than allocating another encoder.
Every obligation in the resulting dependency component must still be discharged;
a pending entry is not an assumed proof. Ever-growing distinct specializations
do not become a finite graph by dropping their arguments. Resource exhaustion
or unsupported production reports failure rather than emitting partial coverage.

Runtime recursion follows ordinary [tail-position rules](termination.md#calls-and-productive-loops).
Nested serialization that returns to process siblings uses explicitly provisioned
pending-work storage or a valid tail-recursive formulation. Libraries own work
item/adapter representation, capacity, exhaustion, progress, and reference custody.
No hidden recursive stack is generated. Cyclic/shared object traversal separately
chooses whether to follow, identify, or omit references; a recursive schema alone
does not establish that policy.

## Runtime descriptions and Placed access

An explicit `reflect::metadata<T>()` request materializes owned copy-eligible
descriptive data under ordinary constant rules. It may be a library projection
from TypeSchema rather than another primitive. Direct visitation does not retain
a complete runtime table; names actually used by generated code still cost data.
Runtime lookup selects among already retained checked operations, not arbitrary
addresses or new type arguments. Registries and adapter inventories are explicit
library/build products with ordinary storage and lifecycle contracts.

Selected runtime interfaces separately expose descriptions, getters, setters,
factories, serialization, or invocation. Availability of one does not imply all.
Ordinary explicit dynamic conformances retain instance/table custody and borrowed
lifetimes; there is no accidental static-lifetime requirement on all inspectable
values. Recursive adapter retention follows exact dependencies, not a global
inventory of every known type.

Ordinary RAM borrows remain ordinary field projections. Packed/fragmented fields
without an addressable reference reject the borrowing route unless an explicitly
selected copied-read operation supplies the required value and contract. Do not
invent a temporary referent or change backing/lifetime identity.

[Placed inspection](../resources/placed_access.md) uses the same semantic member
identities but invokes its exact authorized access operations. Schema enumeration
does not read devices. A general inspector cannot silently issue Take, atomic
loads, or MMIO transfers. Explicitly selected device operations retain ordering,
effects, supply, residency, extent, and loan evidence. BindingPrivate restrictions
do not prevent delegation through an already authorized public accessor contract,
but names or metadata cannot manufacture that access. Recasts do not bypass it.

## Construction and editing

Decoded documents are not instances of the described type. Library migration and
field parsing produce candidates; owner-authorized construction establishes
predicates, erased terms, routed evidence, and resources. Encoding selection does
not prove decoding is an inverse or that a grant can be reconstructed from bytes.
Stable format numbering, defaults, version migration, and reference identity
remain codec/library contracts.

Mutable reflection is not an automatic counterpart of borrowing inspection.
Owners supply checked setters or whole-object update operations with declared
failure/partial-update and invariant behavior. UI clamps and editable flags are
policy, not proof. Metadata processing executes no getter, constructor, setter,
or external operation merely to describe it. Logical collections expose selected
element operations rather than implicitly traversing allocator bookkeeping.

## Compiler ownership

Psi owns authorized schema queries, semantic member correspondence, typed policy
selection, and per-member elaboration into ordinary checked data/calls before
Terminal Psi. Selection and execution share callable-family checking but retain
their distinct evaluation and evidence obligations. Replay joins declaration,
member, application, access scope, policy selection, and operation; corrupted
descriptions or same-spelled user APIs cannot acquire compiler privileges.

Omega/targets retain layout realization, native adapters, ABI, and emission.
There is no new boundary trait or opaque admission merely because an operation
is compiler-known. Logical schemas, optional library plans, and runtime tables
do not authorize source splicing, new types, arbitrary method invocation, or a
second layout system. Additional owner-level semantic choices belong in
[OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md), not a surviving proposal.

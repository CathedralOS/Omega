# RFC: Semantic reflection through data and ordinary machines

Status: open design proposal. No reflection API, schema carrier, typed visitor,
runtime adapter, or metadata format below is approved or implemented by this RFC.
Examples are candidate source and algorithm sketches, not executed tests.
Existing generics, type equality, layout, and access rules remain unchanged.

## Customer and recommendation

Application and library authors want inspectors and serializers that handle
records and active sum payloads without repeating every member by hand. Runtime
editors, save-data migration, and logical collection inspection are additional
customers against which to evaluate the mechanism, not promises of the first
implementation. Their policy, encoding, and object lifecycle belong to libraries.

Prefer ordinary data and code with reusable typed member access. Make direct
typed visitation the first candidate: statically select members and specialize
an ordinary callback machine for each member's type, then execute the checked
operations on actual values. Metadata queries and policy planners remain ordinary
data processing. Plans are optional library products, not the required route for
every inspector, serializer, comparison, or hasher.

Static positions request compile-time evaluation. There is no proposed `expand`
keyword, special member-loop syntax, `const machine` species, or `is_build_time()`
branch. A runtime loop over constant metadata does not acquire heterogeneous
typing automatically; the per-member specialization contract is explicit below.

Core ownership does not imply `boundary` or a new service reach. Ordinary library
machines implement policy and data manipulation; only irreducible compiler
queries, typed projection, and staged visitation need compiler-known semantics.
This is not an opaque trust admission. A same-spelled user machine cannot acquire compiler
access, and a core declaration does not grant access to private objects.

Handwritten inspectors and existing core synthesis remain the simpler baseline.
A concrete inspector and serializer must reuse the proposed mechanism without
adding a compiler opcode for each library format. Moving a feature behind a
function call does not make its semantics free. Direct visitation may avoid a
whole-plan carrier for simple clients; its implementation cost remains unmeasured.

The named-callback examples below are comparators, not a permanent restriction
on reflection's authoring tools. Evaluate the companion
[lambda and captured-environment proposal](anonymous_machines.md) alongside typed
visitation, including generic callback families and per-visit lifetimes. A missing
implementation does not by itself justify excluding a useful general language
facility, and proposed lambda support does not make the remaining typed-access
contract disappear.

## Existing boundaries

| Owner | Reuse | Not implied |
| --- | --- | --- |
| [Generics](../spec/language/generics.md) | Static application identity, type equality, and exact value bindings. | Converting runtime metadata to a type argument. |
| [Semantic evaluation](../spec/language/evaluation.md) | Eligible ordinary machines compute pure data and plans. | Reading a future runtime object's contents during compilation. |
| [Layout](../spec/layouts/plans.md) | Compiler-supplied Schema, authored planning, validated geometry. | Backing, readable content, or field-access authority. |
| [Placed](../spec/resources/placed_access.md) | Layout/access demand, admitted supply, loans, and exact accessor operations. | Raw references to fields merely because an offset is known. |
| [Recasts](../spec/layouts/recasts.md) | Checked ordinary representation-compatible views. | Bypassing Placed access restrictions. |
| [Dynamic dispatch](../spec/terminal-psi/dynamic_dispatch.md) | Explicit conformance, instance, operations, and custody. | Universal invocation by name or mandatory RTTI. |

The current [core layout schema](../../source/library/core/layout.omg) supplies
field keys, size/alignment, kind, authored identities, cases, and tombstones.
Its fixed capacities are private implementation limits. It neither exposes
general predicate syntax nor establishes general typed field visitation.
The [Self-only generator direction](../spec/language/evaluation.md#trait-bodies-and-generators)
is also narrower than the explicit-T queries proposed here. That scope extension
requires approval; the RFC does not reinterpret existing trait defaults.

## Candidate APIs and data

`reflect` below denotes a candidate core module, not a new namespace rule.
Use exact compiler-known declaration identities for primitive queries. All names,
signatures, and data shapes below remain proposals.

| Call | Result or purpose |
| --- | --- |
| `reflect::schema<T>()` | Immutable declaration schema for the exact static application T. |
| `reflect::type_key<T>()` | Opaque canonical type key for equality and correspondence in planning. |
| `schema.fields()` / `schema.cases()` | Homogeneous borrowed sequences of field/case description records. |
| `reflect::visit_fields<T, Visitor>(&value, &mut context)` | Statically specialize an ordinary selected callback for each field and check its typed borrow and call. |
| `property_policy(schema)` | Example ordinary authored machine returning application policy data, not a compiler primitive. |
| `reflect::metadata<T>()` | Explicitly materialized owned descriptive data, if requested. |

Schema fields contain exact member keys, type keys, optional authored schema
numbers, names, relevance, and case ownership. They are data records of one type,
so an ordinary machine can traverse them. A type key supports equality under
the same canonical rules as `T == U`; it is not a source type expression,
an address, a layout equivalence claim, or a cast permission. A Boolean comparison
of keys alone does not make a metadata variable into a typed field projection.

Sequences here mean ordinary collections of description records. Arrays, borrowed
slices, and eligible owned collections can serve that purpose under ordinary
evaluation rules. No conversion into proof-side `Seq(items)` or `Bag(items)` is
required, and no reflection-specific collection intrinsic is proposed. Existing
mathematical libraries may express coverage or ordering laws where useful;
they do not supply the missing descriptor-to-typed-member connection.

Use handle-backed schema graphs rather than recursively copying descriptions.
Queries describe only the exact selected subject, not a package-wide ambient
inventory. Recursive types produce graph edges, not infinite traversal.
Declared ranges retain static endpoints or symbolic subject dependencies; there
is no solver for the tightest satisfying bound. Names are descriptive bytes,
not a second name resolver.

## An inspector and serializer

Start with two customers of the same staged operation. The following candidate
source assumes full authorized visibility of Player's fields:

```omega
data Player {
    health: u32;
    speed: f32;
}

machine inspect_player(player: &Player, output: &mut InspectorOutput) {
    reflect::visit_fields<Player, show_field>(player, output);
}

machine show_field<Value>(
    field: FieldInfo,
    value: &Value,
    output: &mut InspectorOutput
)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.show_u32(field.name, value)
        Value == f32 -> output.show_f32(field.name, value)
    }
}
```

The proposed visitor elaborates typed operations equivalent to:

```text
show_field<u32>(health_description, &player.health, output)
show_field<f32>(speed_description, &player.speed, output)
```

The member and Value type are selected during compilation. FieldInfo describes
that exact member; it is not the source of authority for the borrow. The player
and output are runtime arguments. Compilation does not read a future player or
execute its output operations. Adding a field unsupported by show_field rejects
the invocation with the member path and unmet callback constraint.

A serializer selects a different ordinary callback:

```omega
machine serialize_player(player: &Player, output: &mut RecordOutput) {
    reflect::visit_fields<Player, write_field>(player, output);
}

machine write_field<Value>(
    field: FieldInfo,
    value: &Value,
    output: &mut RecordOutput
)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.write_u32(field.name, value)
        Value == f32 -> output.write_f32(field.name, value)
    }
}
```

RecordOutput is an example library sink responsible for encoding and framing,
not a compiler-defined format. These sketches omit the sinks' declarations,
resource and failure contracts, and exact callback-binding signature; they are
not claims that the current generic machine parameter rules express visit_fields.
All output preconditions, failures, service reach, suspension, and resource needs
must propagate from the concrete selected machines. Finite field traversal does
not by itself prove that the callback terminates or that an invocation is eligible
for semantic evaluation. A serializer's partial-output behavior is its own contract.

The numeric branches only illustrate type refinement. The mechanism must also
support callbacks using explicitly selected ordinary conformances for user-defined
field types, without a compiler-maintained roster of printable or encodable types.
Neither callback needs an InspectionStep enum or a compiler instruction for its
output operation.

## Typed access and staging

The candidate record operation visits each immediate field once in authored
declaration order; a zero-field record makes no calls. Recursive descent is not
implicit. A format wanting stable-number order instead makes that selection
explicit. Unknown or erased fields reject this initial all-fields borrowing route
rather than silently disappear; a broader policy must state its coverage.

Per-member specialization is a semantic requirement, not an optimizer hint.
For every selected member, the compiler establishes its declaring type and exact
application, derives its actual field type, checks the callback application,
and constructs the ordinary typed projection and call. A runtime type-key test
cannot substitute for these checks. Any later explicit member-selection API must
also reject a forged key/type pairing or a key from another application.

Shared field borrows and the mutable context obey ordinary loan rules. Calls
are sequenced, not implicitly parallel. An escaping subloan must retain its real
parent lifetime and cannot conflict with later calls or mutation. Reading metadata
does not copy, consume, or authorize mutation of the reflected value. A low-level
typed projection operation may underlie visitation; whether it also needs a public
standalone API should be decided from concrete customers, not assumed here.

An ordinary loop over homogeneous FieldInfo data is sufficient for policy
calculation. It is not sufficient to give one runtime loop variable a different
source type on each iteration. The proposed core visitation operation supplies
that staged connection while the callback stays ordinary code. No user AST,
source-string generation, expansion keyword, or closed inspector bytecode is
introduced by this proposal.

## Anonymous callbacks as a candidate

A general lambda facility could bind a runtime context directly, avoiding a
separately named callback and explicit output parameter at every visit:

```omega
let show_field = machine<Value> [output = &mut output](
    field: FieldInfo,
    value: &Value
)
where Value == u32 || Value == f32;
{
    transition {
        Value == u32 -> output.show_u32(field.name, value)
        Value == f32 -> output.show_f32(field.name, value)
    }
};

reflect::visit_fields<Player>(player, &mut show_field);
```

This is candidate syntax from the [anonymous-machine RFC](anonymous_machines.md),
not a second approved overload. Its intended elaboration calls one statically
known generic body at each field type while reborrowing the same captured output
environment. The runtime player and output are not inspected during compilation.
Capturing output does not capture or copy the player, and a callback cannot retain
a field loan beyond its proven lifetime.

Compare this route with named callbacks, explicit stateful visitors, and library
plans before choosing the public API. The generic callable-family contract, exact
conformance selection, receiver mode, and lifetime/effect forwarding need joint
design. Ordinary callbacks can return pruning or early-stop results for library
walkers; an anonymous body need not introduce reflection-specific control flow.
Noncapturing lambdas alone are not sufficient evidence for the stateful walker,
and generic lambdas alone do not supply checked member projection.

## The compiler contribution is explicit

Three pieces are new, even though they are exposed as calls and data:

1. A deterministic semantic schema query for an explicitly selected type.
2. Exact schema/member correspondence through evaluation, elaboration, and replay.
3. Typed projections and staged per-member callback applications, followed by
   ordinary generated-body checking.

The existing canonical-constant rules do not automatically admit compiler arenas,
opaque pointers, or arbitrary handles as generic atoms. Direct visitation can
derive its members from T without requiring a whole user-authored plan as a
generic argument. It does not eliminate the correspondence obligation. Local
handles may serve indexing inside compilation; retained schema references need
independently resolvable owner, application, member, and semantic-revision
correspondence. A digest or matching integer key alone cannot grant authority.
The schema carrier and any static member-key application need their own defined
identity rules; no new canonical generic atom is assumed here.

The target-neutral schema and typed elaboration belong to Psi and must resolve
to ordinary checked behavior before Terminal Psi. Layout realization and native
adapter emission keep their existing owners. Inspection, encoding, and editing
policy do not become compiler semantics.

## Application policy and optional plans

Field types do not decide whether an application saves, edits, displays, or
replicates a property. Associate ordinary authored policy with exact member keys.
The following symbolic member-selection spelling is provisional, as is the
policy carrier:

```text
health_policy = PropertyPolicy {
    member: reflect::member<Player::health>(),
    label: "Health",
    editable: true,
    saved: true
}
speed_policy = PropertyPolicy {
    member: reflect::member<Player::speed>(),
    label: "Speed",
    editable: false,
    saved: true
}
```

A member key identifies the compiler-resolved declaration and exact owning
application; authors do not assign an integer, offset, or name hash. Enumeration
already supplies these references in FieldInfo. Explicit selection should resolve
the same declaration as ordinary field access, reject nonexistent members, and
obey visibility. Admitting a field declaration as the argument above requires a
new binding contract; existing type/value/machine arguments do not imply it.
Canonical retained identity and access authority remain distinct.

An ordinary property_policy machine can filter and organize these records.
Separate editing, save, and replication policies may refer to the same member;
there need not be one universal flag record. No new annotation syntax or
compiler-known SaveGame/Editable behavior is proposed. Labels and format names
are data, not durable member identity. An editable flag grants no write authority,
and a UI clamp is not evidence that a domain predicate holds.

These entries illustrate exceptions, not required repetition of the schema. A
policy machine can derive labels and default read-only exposure, then override
selected members. An editor may display speed read-only rather than hide it;
saved is relevant to a different consumer. Owners still authorize exposure and
operations independently of those presentation choices.

String lookup is an ordinary algorithm over names and member references, not
another presumed compiler primitive. An eligible find_member(schema, name)
invocation can run during compilation and return a member or a handled lookup
failure. For runtime names, ordinary compile-time code can instead build fixed
entry arrays and name bytes for a linear, sorted, or generated-hash lookup table.
Runtime lookup returns an entry for an already-retained checked operation, not a
new type argument. Temporary storage belongs to evaluation; retained table data
uses ordinary constant materialization. Runtime filtering can further narrow
display or operation choices but cannot create unavailable access authority.

Logical properties can expose an owner-authored getter or setter rather than a
storage field. Selecting a description performs no getter, attribute constructor,
device operation, or object construction. Invoking an operation is a separate
checked call with its own effects, failures, and lifetime contract. A collection
adapter should expose its logical elements, not automatically recurse into its
allocator and capacity bookkeeping.

Plans remain useful for selected ordering, reusable editor tables, and existing
Layout/AccessPlan customers. They are ordinary library data processed by ordinary
machines wherever possible. A static selection feeding typed access still needs
validated correspondence and a defined application/encoding contract; a runtime
selection uses already-retained checked adapters. Neither route makes arbitrary
runtime metadata into a type parameter. Do not add a compiler consumer for each
format, or disguise arbitrary code generation as an InspectionPlan.

## Layout and Placed integration

Declaration schemas provide semantic membership. Layout provides target-resolved
geometry for the same exact members. Reuse the existing schema identities and
key joins rather than create an independent offset calculator. Geometry may be
queried only after the selected layout is validated. Planning a type cannot query
its own unfinished layout; dependency cycles reject. Static type structure and
target geometry remain distinct dependency stages.

An ordinary layout or access planner can traverse field records without special
syntax. The integration is algorithmically:

```text
schema = compiler-supplied schema for T
layout = validated selected Layout plan
access = AccessPlan::inaccessible(schema)
walk schema's field records using ordinary states
    map each exact declaration key to its physical field key
    add explicitly chosen access to access
return the plan for normal validation
```

Existing AccessPlan operations implement plan construction. The exact key join
must reject wrong schema/application/revision combinations. Erased fields have
no physical key or access decision. Requesting read permission does not establish
admitted storage supply or prove actual content is valid.

Ordinary RAM stays ordinary T and its checked lvalue borrows. Packed/fragmented
fields may lack an addressable shared reference; the initial consumer rejects
those projections unless it has an independently specified copied-read operation.
It must not invent a temporary reference with the wrong backing or lifetime.

Placed inspection is a separate consumer over the same declaration identities:

```text
visit_placed_fields<P, T, AccessPolicy, Visitor>(view, context)
```

This is an algorithmic signature, not an approved generic form. Each selection
uses the existing accessor for the exact P/T field and invokes only its authorized
operation. Selecting a field performs no device read. The actual view carries
supply, provenance, residency, range, and loan evidence; metadata or an authored
policy supplies none. Inaccessible and BindingPrivate access stays restricted, and
ordinary recasts cannot bypass it.

A general read-only inspector must not silently invoke External Take, atomic
loads, or repeatable MMIO reads. Device-aware inspection needs an explicitly
selected access policy and operation contract, including ordering and effects.
This reuses Placed operations rather than inventing reflection-specific memory
access. It can follow the ordinary-record prototype rather than block it.

## Active cases and recursion

A sum-aware visitor must check callback applications for every reachable case,
handle common fields once, and dispatch on the runtime discriminator before
projecting the selected payload with its exact case evidence. Compiling those
applications does not execute all payload reads. Retired IDs are metadata rather
than cases, and wire numbers do not replace runtime discriminants. Missing cases
or implicit field omissions
reject a full-coverage inspector. Runtime mutation must not invalidate case
evidence while payload subloans remain live.

Recursive schema graphs do not by themselves solve recursive codec derivation.
A recursive consumer must reuse the selected machine/conformance application
instead of infinitely instantiating it again. Runtime traversal separately needs
its own progress, resource, and object-identity policy. Shared or cyclic object
graphs require an explicit choice to follow, identify, or omit references; a
field list does not authorize following every pointer or duplicating resources.
The design lab must distinguish compilation of recursive types from traversal
of recursive or cyclic values.

## Runtime descriptions and operations

Runtime names and descriptions are a separate explicit projection:

```omega
const PLAYER_DESCRIPTION = reflect::metadata<Player>();
```

This returns an owned, copy-eligible data snapshot under ordinary constant
materialization, not references into compiler memory. Metadata may be an ordinary
projection from the schema rather than an independent primitive. Direct visitation
need not materialize a runtime interpreter or complete metadata table.
Names used by the generated inspector are nevertheless retained as program data;
there is no claim of zero code/data cost. Primitive type tags in descriptive data
do not become authoritative runtime type objects.

Compile-time reflection can support two library strategies: specialize a consumer
into typed code, or materialize descriptions and selected checked adapters for a
shared runtime consumer. A runtime editor may need the latter; a static serializer
may prefer the former. Neither strategy is universally required or presumed faster.

A runtime-selected object can expose an ordinary Inspectable conformance whose
visitor and description agree on its type. It does not accept an arbitrary
address plus a claimed type ID. Runtime availability must distinguish descriptive
names, readable properties, setters, factories, serialization, and invocation.
Selecting one does not retain or authorize all the others. Selected operations
retain their transitive code/data dependencies, including required child adapters;
recursive dependencies must not cause infinite table construction.

Registries and callable adapters are explicit library/build products, not mandatory
per-object headers or an ambient inventory of every package declaration. A known
type or ordinary direct call does not alone promise runtime reflective availability.
Plugin discovery, loading, and registration need an explicit inventory and lifecycle
contract; reflection cannot discover code absent from the artifact or loaded system.

Per-property getters, checked setters, downcasting, and invocation by name need
their own instance, lifetime, callable, and ownership contracts. Borrowed downcasts
must preserve the borrow; a fallible owning conversion must preserve or explicitly
account for the original owner on failure. Describing borrowed data must not acquire
an accidental static-lifetime restriction from the runtime registry's carrier.
Runtime metadata cannot become a static member argument or raw callable address.

## Documents, migration, and construction

A dynamic document representing Player is not a Player. Editors and save systems
may operate on a separate, explicitly stored document and later request conversion:

```text
document = decode_saved_data(bytes)
document = migrate_player_document(document)
candidate = parse_player_fields(document)
player = Player::create(candidate, required_resources)
```

This is an algorithm sketch; each step can fail normally and must account for
storage and cleanup. Reflection can remove repetitive field handling without
establishing the constructor's predicates or supplying its resources. Defaults,
renamed or retired members, schema versions, and reference identity are codec and
migration contracts, not inferred from equal layouts or matching type names.

Patching a live object is a separate owner-authorized operation. The owner defines
validation, change notification, and whether failure leaves the object unchanged
or permits a specified partial update. A read-only inspector does not imply a
universal apply operation. No descriptor can mint a qualification, duplicate a
linear resource, bypass a private setter, or turn an arbitrary document into a
valid instance merely by claiming its type key.

## Visibility, ownership, and evidence

Queries and consumers obey source dependency and visibility rules. A generic
library does not inherit a caller's private access. The initial exact schema
query either has authorized complete structural visibility or rejects; it does
not silently hide fields and pretend to enable complete serialization. Owners
may explicitly export descriptions or checked wrappers. Rules for a richer
owner-scoped generator remain a separate access decision, not an implicit friend
privilege. Private issuer metadata retained by verification grants no reflective
selection authority.

Borrowed inspection does not move the record, duplicate linear fields, or mint
qualifications. Reading a copy-eligible value is different from borrowing or
consuming a resource. Any omitted field in a broader policy needs an explicit
coverage choice; an erased field has no runtime borrow even when describable.
Mutable reflection is not part of the first route: use owner-authored setters
that preserve invariants and expose ordinary failure outcomes. Reflected method
knowledge does not waive requires, effects, progress, or custody.

Schema queries and planning are admitted by the existing hermetic evaluation
rules, with compiler-defined primitives where necessary. No extra boundary
trait or opaque admission is required just because an API lives in core. Runtime
output, device access, and foreign calls independently retain their real boundary
contracts. Compiler-known operation identity is not inferred from spelling.

Generated bodies need ordinary verification and declaration-to-member-to-call
correspondence, including a policy or plan when used. Neither visitation nor a
plan bypasses the checker. Dependencies retain selected types, member identities,
conformance/callable selections, access context, and
target geometry when actually consulted. Relevant declaration changes invalidate
cached plans and derived artifacts. Schema numbers, display names, local handles,
type identity, and artifact commitments keep their distinct purposes.

## Lessons from other systems

These comparisons motivate candidate mechanisms and design-lab customers, not
adoption of another language's syntax or safety model. The research inspected
documentation and source, not executed prototypes or comparative benchmarks.

| System and evidence | Consequence for this proposal |
| --- | --- |
| [Zig 0.15.1](https://ziglang.org/documentation/0.15.1/#field) and [D member access](https://dlang.org/spec/traits.html#getMember) | Metadata discovery and typed member access compose with ordinary code. Staged heterogeneous iteration still needs a semantic mechanism, not an optimization assumption. |
| [C++26 reflection P2996R13](https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2025/p2996r13.html), [adoption status](https://wg21.link/P2996/status) | Constant-evaluated data processing can select typed members. Its unchecked accessibility and splicing rules are not Omega authority rules. Adoption does not establish universal compiler support. |
| [Unreal properties](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-uproperties) and [object handling](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-object-handling-in-unreal-engine) | Editing, saves, replication, defaults, and reference tracing need different policy/lifecycle contracts. Editor metadata specifiers are distinct from runtime property behavior; neither requires a mandatory Omega object model. |
| [.NET JSON generation](https://learn.microsoft.com/en-us/dotnet/standard/serialization/system-text-json/reflection-vs-source-generation) and [trimming](https://learn.microsoft.com/en-us/dotnet/core/deploying/trimming/prepare-libraries-for-trimming) | Generate specialized code or metadata for a shared consumer. Reflective availability needs explicit operation/dependency retention, not an all-or-nothing metadata switch. |
| [.NET CustomAttributeData](https://learn.microsoft.com/en-us/dotnet/api/system.reflection.customattributedata) | Inspect recorded descriptions without executing constructors or getters. Operation effects are separate from descriptive discovery. |
| [Rust Any](https://doc.rust-lang.org/std/any/index.html) and [experimental type_info](https://github.com/rust-lang/rust/issues/146922) | Stable Any provides exact-type testing/downcasting, not structural enumeration. Preserve borrow/ownership rules; do not import its static type bound into all inspection. The nightly experiment still has unresolved design questions, not a settled lifetime-aware solution. |
| [Bevy Reflect](https://docs.rs/bevy_reflect/0.19.1/bevy_reflect/) | Logical collections, registered operations, dynamic documents, and patch/conversion workflows are concrete customers. Representing a type is not being an instance of it. |
| [Scala 3 Mirror](https://docs.scala-lang.org/scala3/reference/contextual/derivation.html) | Small structural facilities can support library-defined derivation. Recursive consumers must reuse selected instances; schema graph construction alone does not close that obligation. |

Type synthesis deserves a separate comparison, not an implied reflection feature.
[Zig MultiArrayList](https://github.com/ziglang/zig/blob/0.15.1/lib/std/multi_array_list.zig)
and the C++26 struct-of-arrays example provide concrete columnar-storage customers.
First distinguish storage and typed-view problems served by Layout/Placed from
problems that actually require a new semantic field/type set. Ordinary evaluation
does not gain permission to splice declarations through this RFC. Do not add
type synthesis, a replacement layout mechanism, or runtime type construction
without that separate design decision.

## Design lab and alternatives

No experiment has run. These cases would discriminate the proposed mechanism:

| Case | Expected result |
| --- | --- |
| Player inspector and serializer | Both specialize ordinary callbacks through the same mechanism, with no format-specific compiler operations, record copy, or mandatory object header. |
| Captured generic lambda | Reborrow one environment across differently typed visits; compare with named callbacks without relaxing field-lifetime or effect checks. |
| Added unsupported field | Diagnose the member and unmet callback constraint; an explicit policy omission cannot masquerade as full coverage. |
| User-defined field type | Use its explicitly selected conformance without adding a compiler opcode. |
| Forged member/type pairing | Consumer rejects before generating an unsafe projection. |
| Explicit member name and string lookup | Symbolic binding rejects nonexistent members; ordinary lookup handles failure and runtime tables invoke only retained adapters. |
| Runtime metadata used for static member selection | Reject; use a retained runtime adapter rather than converting a type key into a generic argument. |
| Schema or member reference outlives evaluator storage | Retain canonical symbolic identity, not a dangling host pointer. |
| Ordinary metadata filtering | Traverse ordinary arrays/slices or eligible collections without a required proof-sequence conversion. |
| Layout query while that layout is forming | Reject dependency cycle, not observe a partial plan. |
| Sum with linear payload | Only the active case is inspected with ordinary subloans. |
| Borrowed field and retained subloan | Preserve actual lifetimes and reject conflicting later access; no blanket static-lifetime requirement. |
| Recursive codec and cyclic object graph | Reuse recursive applications during derivation; require a separate runtime traversal and identity policy. |
| Erased field | Describe when authorized; no physical key or runtime borrow. |
| Placed MMIO FIFO | Metadata processing performs no transfer; Take is not read. |
| BindingPrivate accessor | Metadata does not enable unauthorized use. |
| Private record and owner-approved wrapper | Generic code gains no caller privilege; the owner can deliberately expose a checked operation without exporting all storage. |
| Logical collection and computed property | Inspect elements through the selected adapter; metadata discovery executes no getter or allocator operation. |
| Editing versus save policy | Apply separate member selections; flags and UI bounds grant neither mutation authority nor domain facts. |
| Runtime range endpoint | Preserve symbolic dependency instead of fabricating a static bound. |
| Runtime metadata not requested | No generic metadata retention obligation; emitted calls/literals still have costs. |
| Shared runtime consumer | Retain only selected descriptions/adapters and their required dependency closure; do not retain all methods merely because T is known. |
| Migrated document or failed patch | Validate through owner operations; account for resources and the declared failure behavior without minting an instance from a type key. |
| Equal-layout nominal types | Remain different; member substitution cannot cross-cast them. |
| Columnar storage | Identify what Layout/Placed already supplies and any genuinely missing type construction before proposing synthesis. |

Compare these alternatives without adding syntax prematurely:

- Plain hand-authored visitors: least compiler machinery, but duplicate member
  selection. Keep them as the semantic and performance comparator.
- Anonymous generic machines with captured context: evaluate the general callable
    proposal jointly with reflection, not as an excluded future convenience or an
    assumed implemented feature.
- Compiler-mediated typed visitor as a core machine: the first candidate. Specify
    per-member generic instantiation, projections, and conformance selection. Compare
    its source and generated behavior with both handwritten customers.
- Optional library plans: useful for policy selection or shared consumers, but
    not a mandatory compiler instruction language for every reflective algorithm.
- Runtime descriptor interpretation: appropriate for dynamic editors with checked
    adapters. Compare its retained data, code size, and indirection with specialization
    instead of presuming one strategy best for every client.
- General AST/code quotation or a special expansion keyword: not proposed.
  Establish a concrete failure of data and ordinary machine composition before
  reconsidering a new syntax system.

## Decisions before implementation

Specify the first schema-query inventory, exact key/equality and retained-identity
contracts, and staged callback signature. Resolve how generic machine parameters
bind per-member types and explicit conformances, how context and subloans are
threaded, and how complete callback effects and failures compose. Direct visitation
is the recommended first candidate, not a ratified extension to generic calling.
Include anonymous generic callbacks and ordinary captured environments in that
comparison. The [lambda proposal's open contracts](anonymous_machines.md#decisions-before-implementation)
identify related binding, receiver, lifetime, and storage decisions; current
named-only implementation limits are not the acceptance criteria.

The proposed first experiment pairs the public numeric inspector with the record
serializer above, compared with handwritten code. Both must reuse the same typed
mechanism, diagnose unsupported fields, and retain ordinary checking. Then test
user-defined conformances, recursive types, active sum payloads, and owner-approved
private access before calling the mechanism general. A passing two-field inspector
alone is not sufficient acceptance evidence.

Runtime tables require a separate concrete adapter, retention, and instance-lifetime
contract. Documents and patching require owner-controlled validation and failure
semantics. Placed visitation retains the exact existing access contracts. Optional
static plans need a defined canonical carrier only where a chosen client actually
uses them as static arguments. Type synthesis and arbitrary invocation remain
separate designs. No implementation experiment, execution-board commitment, or
change to normative reflection restrictions is approved merely by this RFC.
No new source keyword is proposed.
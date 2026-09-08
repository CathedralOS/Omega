# RFC: Semantic reflection through data and ordinary machines

Status: open design proposal. No reflection API, schema carrier, inspection-plan
consumer, or runtime metadata format below is approved or implemented by this
RFC. Examples are candidate source and algorithm sketches, not executed tests.
Existing generics, type equality, layout, and access rules remain unchanged.

## Customer and recommendation

Application and library authors want read-only property inspection of records
and active sum payloads without repeating every member by hand. Serializers can
reuse declaration discovery, with their own encoding and evolution contracts.
Neither customer requires a macro language, runtime type construction, arbitrary
method invocation, or access to the compiler's private AST/IR.

Prefer ordinary data and code: query a typed semantic description, construct a
plan using an ordinary machine, and validate that plan at an explicitly selected
consumer. Static positions request compile-time evaluation. There is no proposed
`expand` keyword, special member-loop syntax, `const machine` species, or
`is_build_time()` branch. Computation inside the planner uses ordinary states,
transitions, helpers, and data operations.

Core ownership does not imply `boundary` or a new service reach. Ordinary library
machines implement policy and data manipulation; only irreducible compiler
queries and plan consumption need compiler-known semantics. This is not an
opaque trust admission. A same-spelled user machine cannot acquire compiler
access, and a core declaration does not grant access to private objects.

Handwritten inspectors and existing core synthesis remain the simpler baseline.
The proposed plan consumer earns its cost only if a concrete inspector and
serializer can reuse it without proliferating a compiler opcode for each library
format. Moving a feature behind a function call does not make its semantics free.

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
| `numeric_plan(schema)` | Ordinary authored machine returning an inspection plan or explicit rejection data. |
| `inspect<T, Plan>(&value, &mut output)` | Consumer of a static plan; validate and elaborate its selected typed operations. |
| `reflect::metadata<T>()` | Explicitly materialized owned descriptive data, if requested. |

Schema fields contain exact member keys, type keys, optional authored schema
numbers, names, relevance, and case ownership. They are data records of one type,
so an ordinary machine can traverse them. A type key supports equality under
the same canonical rules as `T == U`; it is not a source type expression,
an address, a layout equivalence claim, or a cast permission. A Boolean comparison
of keys alone does not make a metadata variable into a typed field projection.

Use handle-backed schema graphs rather than recursively copying descriptions.
Queries describe only the exact selected subject, not a package-wide ambient
inventory. Recursive types produce graph edges, not infinite traversal.
Declared ranges retain static endpoints or symbolic subject dependencies; there
is no solver for the tightest satisfying bound. Names are descriptive bytes,
not a second name resolver.

## A concrete record inspector

Start with a deliberately small data vocabulary for a read-only numeric
inspector. This is a typed plan, not source code stored as strings:

```omega
data InspectionStep {
    case ShowU32(field: FieldKey);
    case ShowF32(field: FieldKey);
}

data InspectionPlanResult {
    case Invalid;
    case Ready(plan: InspectionPlan);
    case Unsupported(field: FieldKey);
}
```

`InspectionPlan` retains the exact schema identity and ordered steps. Its builder
reserves sufficient compile-time storage for the selected finite schema under
ordinary evaluation sponsorship. Builder capacity, element ownership, and count
obligations remain checked; this is not an unbounded hidden runtime allocation.
The omitted carrier definitions are API-design work, not existing library types.

An ordinary machine examines homogeneous field descriptions and builds that plan:

```omega
machine numeric_plan(schema: DeclarationSchema) -> InspectionPlanResult {
    transition schema.is_record() {
        true -> walk(schema.fields(), InspectionPlan::for_schema(&schema))
        false -> invalid()
    }

    state walk(fields: &[FieldInfo], plan: InspectionPlan) -> InspectionPlanResult {
        transition fields.len > 0 {
            true -> choose(fields[0], fields[1..], plan)
            false -> ready(plan)
        }
    }

    state choose(field: FieldInfo, rest: &[FieldInfo], plan: InspectionPlan)
        -> InspectionPlanResult {
        transition field.is_runtime_field() {
            true -> classify(field, rest, plan)
            false -> unsupported(field.key)
        }
    }

    state classify(field: FieldInfo, rest: &[FieldInfo], plan: InspectionPlan)
        -> InspectionPlanResult {
        transition {
            field.type_key == reflect::type_key<u32>() ->
                walk(rest, plan.append(InspectionStep::ShowU32(field.key)))
            field.type_key == reflect::type_key<f32>() ->
                walk(rest, plan.append(InspectionStep::ShowF32(field.key)))
            _ -> unsupported(field.key)
        }
    }

    state ready(plan: InspectionPlan) -> InspectionPlanResult {
        InspectionPlanResult::Ready(plan)
    }

    state unsupported(field: FieldKey) -> InspectionPlanResult {
        InspectionPlanResult::Unsupported(field)
    }

    state invalid() -> InspectionPlanResult {
        InspectionPlanResult::Invalid
    }
}
```

This is a candidate body sketch. The ordinary ranking is the remaining field
count: each cycle removes one field. Its exact state ranking, schema borrow,
builder capacity, and result cleanup contracts must be discharged before it is
eligible for semantic evaluation. `FieldInfo` is copy-eligible metadata, not a
copied runtime field. Unknown or erased fields reject this inspector rather
than disappear. A zero-field record yields an empty plan. More sophisticated
policies may explicitly record omissions, but cannot call them full coverage.

Use existing static positions to request evaluation and select the consumer:

```omega
data Player {
    health: u32;
    speed: f32;
}

const PLAYER_INSPECTION = numeric_plan(reflect::schema<Player>());

machine inspect_player(player: &Player, output: &mut InspectorOutput) {
    inspect<Player, PLAYER_INSPECTION>(player, output);
}
```

The proposed consumer requires a Ready result matching Player. Unsupported or
Invalid is a compile-time consumer diagnostic, not a runtime panic or a special
`reflect::reject` construct. The producer can inspect or transform rejection data
using ordinary code. Constant evaluation builds the plan; it does not read the
runtime player. The actual value is observed only when inspect_player executes.

For Player, the consumer elaborates the equivalent of:

```text
output.show_u32("health", &player.health)
output.show_f32("speed", &player.speed)
```

Assume the output operations are explicitly defined checked library machines.
Their real effects, preconditions, and resource requirements still propagate.
The consumer proves each key belongs to the exact schema, checks its actual type
against the requested operation, and checks projection and call legality. A
forged ShowU32 step for the speed field rejects regardless of what the producer
asserted. Field names come from the matched declaration, never from a pointer
calculation supplied by the plan.

## The compiler contribution is explicit

Three pieces are new, even though they are exposed as calls and data:

1. A deterministic semantic schema query for an explicitly selected type.
2. A plan-value representation that retains exact declaration references through
   evaluation, static application, and replay without dangling compiler handles.
3. A validated consumer relating those references to typed projections and calls.

The existing canonical-constant rules do not automatically admit compiler arenas,
opaque pointers, or arbitrary handles as generic atoms. Exact schema/member
references need a defined symbolic plan carrier and canonical encoding, or a
dedicated static-plan application contract. Local handles may serve indexing
inside compilation; serialized plans need independently resolvable owner,
application, member, and semantic-revision correspondence. A digest or matching
integer key alone cannot grant authority. This carrier/application choice remains
an explicit design dependency, not an assumed existing facility.

The consumer is not merely an eager function taking arbitrary runtime plan data.
Its plan argument is static, and elaboration validates the complete plan before
ordinary generated-body checking. Type-changing field access cannot be implemented
by casting a runtime type key. Primitive projection rules may be compiler-known;
iteration, formatting policy, and plan construction remain ordinary code.

Start with the small inspector to test this division. A general version should
bind an explicitly selected visitor/codec conformance or static machines, not
add a new compiler instruction for each formatter or serialization format.
General typed visitor instantiation over selected schema members is itself a
contract to specify; it is not automatically implied by metadata enumeration.
Do not conceal an arbitrary AST-generating language inside InspectionPlan.

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
inspect_placed<P, T, Plan>(view, output)
```

Each step selects the existing accessor for the exact P/T field and invokes only
its authorized operation. Selecting a field performs no device read. The actual
view carries supply, provenance, residency, range, and loan evidence; the plan
supplies none. Inaccessible and BindingPrivate access stays restricted, and
ordinary recasts cannot bypass it.

A general read-only inspector must not silently invoke External Take, atomic
loads, or repeatable MMIO reads. Device-aware inspection needs an explicitly
selected access policy and operation contract, including ordering and effects.
This reuses Placed operations rather than inventing reflection-specific memory
access. It can follow the ordinary-record prototype rather than block it.

## Active cases and runtime descriptions

A sum-aware planner records common fields once and a finite branch plan for each
case. The consumer observes the runtime discriminator, then projects only the
selected payload with its exact case evidence. The plan does not execute all
payload reads. Retired IDs are metadata rather than cases, and wire numbers do
not replace runtime discriminants. Missing cases or implicit field omissions
reject a full-coverage inspector. Runtime mutation must not invalidate case
evidence while payload subloans remain live.

Runtime names and descriptions are a separate explicit projection:

```omega
const PLAYER_DESCRIPTION = reflect::metadata<Player>();
```

This returns an owned, copy-eligible data snapshot under ordinary constant
materialization, not references into compiler memory. A plan used only for
static elaboration need not materialize a runtime interpreter or metadata table.
Names used by the generated inspector are nevertheless retained as program data;
there is no claim of zero code/data cost. Primitive type tags in descriptive data
do not become authoritative runtime type objects.

A runtime-selected object can expose an ordinary Inspectable conformance whose
visitor and description agree on its type. It does not accept an arbitrary
address plus a claimed type ID. Per-property getter tables, checked setters,
downcasting, and invocation by name need their own instance, lifetime, callable,
and ownership contracts. Runtime metadata cannot be used as the static Plan
argument or materialized as raw callable addresses without that design.

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

Generated bodies need ordinary verification and source-to-plan-to-operation
correspondence. Plans do not bypass the checker. Dependencies retain selected
types, member identities, conformance/callable selections, access context, and
target geometry when actually consulted. Relevant declaration changes invalidate
cached plans and derived artifacts. Schema numbers, display names, local handles,
type identity, and artifact commitments keep their distinct purposes.

## Design lab and alternatives

No experiment has run. These cases would discriminate the proposed mechanism:

| Case | Expected result |
| --- | --- |
| Player's u32/f32 fields | Two exact typed calls from ordinary plan data; no record copy or mandatory object header. |
| Added unsupported field | Producer returns Unsupported or records explicit omission; no silently incomplete serializer. |
| Forged member/type pairing | Consumer rejects before generating an unsafe projection. |
| Runtime plan passed as const Plan | Reject; static elaboration cannot inspect future data. |
| Schema or member reference outlives evaluator storage | Retain canonical symbolic identity, not a dangling host pointer. |
| Layout query while that layout is forming | Reject dependency cycle, not observe a partial plan. |
| Sum with linear payload | Only the active case is inspected with ordinary subloans. |
| Erased field | Describe when authorized; no physical key or runtime borrow. |
| Placed MMIO FIFO | Metadata processing performs no transfer; Take is not read. |
| BindingPrivate accessor | Metadata does not enable unauthorized use. |
| Runtime range endpoint | Preserve symbolic dependency instead of fabricating a static bound. |
| Runtime metadata not requested | No generic metadata retention obligation; emitted calls/literals still have costs. |
| Equal-layout nominal types | Remain different; plan substitution cannot cross-cast them. |

Compare these alternatives without adding syntax prematurely:

- Plain hand-authored visitors: least compiler machinery, but duplicate member
  selection. Keep them as the semantic and performance comparator.
- Compiler-mediated typed visitor as a core machine: could avoid a separate plan
  carrier for simple traversal. It still needs explicit per-member generic
  instantiation and conformance selection; evaluate it against this plan route.
- Runtime descriptor interpretation: appropriate for a dynamic editor with typed
  adapters, but retains metadata and indirection unnecessarily for static clients.
- General AST/code quotation or a special expansion keyword: not proposed.
  Establish a concrete failure of data and ordinary machine composition before
  reconsidering a new syntax system.

## Decisions before implementation

Specify the first schema-query inventory and exact key/equality contract, the
static plan carrier and its application/encoding rules, and the consumer's
projection and callable vocabulary. Choose between direct typed visitation and
plan consumption based on concrete source examples, not keyword convenience.
Define ordinary-record visibility and coverage first; Placed access, sum plans,
runtime descriptions, and owner-published private operations extend it only with
their exact contracts. General mutation, dynamic invocation, and arbitrary code
generation can remain later designs.

The proposed first experiment is a public record inspector with u32/f32 fields
and adversarial plan validation, compared with handwritten code. It would test
whether the data/consumer split is reusable before extending to a serializer,
active sum payloads, and explicit runtime metadata. No implementation experiment,
execution-board commitment, or change to normative reflection restrictions is
approved merely by this RFC. No new source keyword is proposed.
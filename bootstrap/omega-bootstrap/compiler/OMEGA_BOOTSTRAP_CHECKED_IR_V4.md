# Omega bootstrap checked IR schema major 4

CKIR schema major 4 is the private, versioned successor for the first runtime
named-record-construction tranche. It adds one immutable, address-backed
structural value operation so an ordinary named record whose fields are runtime
values can cross the already-checked structural `Call` and `Copy` paths. It
does not add a general aggregate-expression system.

This is not an Omega ABI, an aggregate ABI, or an admission of named-record
construction to final `Ωself`. It is provisional bridge cost and correctness
evidence. Except for the overrides below, every CKIR1 rule; every CKIR2 exact-
root, opcode-10 `Call`, role-3-binding, and finite-call-graph rule; and every
CKIR3 constant-graph, opcode-11, opcode-12, interval, image, resource, status,
and publication rule in
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](OMEGA_BOOTSTRAP_CHECKED_IR.md),
[`OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](OMEGA_BOOTSTRAP_CHECKED_IR_V2.md), and
[`OMEGA_BOOTSTRAP_CHECKED_IR_V3.md`](OMEGA_BOOTSTRAP_CHECKED_IR_V3.md) remains
normative. Schema majors 1, 2, and 3 and their meanings remain frozen.

The separate
[`OMGRSW2/OMGLOW5 source relation`](OMEGA_BOOTSTRAP_RESOLVED_TO_CKIR4_V2.md)
also lowers direct nominal field receivers into this unchanged CKIR4 schema.
Nothing in that successor changes the byte-level contract below.

## 1. Versioned lowering frame and unchanged CKIR tables

The resolved-source lowerer consumes `OMGLOW4`, not an earlier OMGLOW frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW4\0"
8       u16    schema major: 4
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact OMGRSW1 length
28      u32    reserved: zero
32      ...    exact OMGCOMP || exact OMGRSW1 || exact EOF
```

`OMGCOMP` and `OMGRSW1` are unchanged. In particular, OMGRSW1 already retains
the exact source body spans, nominal record identities, declaration-ordered
fields, normalized scalar/record/array types, machine parameters, block
parameters, and role-3 same-owner call bindings needed by this tranche. Runtime
record construction is body meaning reconstructed by the lowerer; it is not a
new resolver-selected row. An implementation must not publish an `OMGRSW2`, a
field offset, or a constructor row merely for record construction information
already present in exact source plus OMGRSW1. The later OMGRSW2 identity exists
only for its genuinely broader field-receiver resolution relation.

The inherited component ceilings remain at most 267,280 OMGCOMP bytes, 524,288
OMGRSW1 bytes, and 791,600 bytes for the complete nominal frame. The greatest
simultaneously source-realizable canonical frame remains the CKIR3 value of
728,680 bytes, with the same first adjacent failure and status precedence.
OMGLOW1, OMGLOW2, OMGLOW3, and OMGLOW4 reject one another exactly.

The CKIR magic remains `OMGCKIR\0`; schema major is 4, schema minor is 0, and
target is 1 (`linux_x86_64`). The complete 80-byte header, flag meanings, table
order, row widths, dense IDs, canonical partitions, and exact-length formula
are byte-for-byte those of CKIR3. No constructor table, field-order vector,
producer layout, frame offset, or address is added. Thus the exact encoded
length remains:

```text
80
+ 24 * type_count
+ 20 * record_count
+ 16 * field_count
+ 36 * machine_count
+ 20 * machine_parameter_count
+ 32 * block_count
+ 20 * block_parameter_count
+ 24 * constant_node_count
+  4 * constant_child_vector_count
+ 40 * operation_count
+  4 * operand_vector_count
+ 44 * terminator_count
```

Consumers reject every other major or minor. Every inherited checked-
arithmetic, exact-EOF, reserved-zero, and no-publication-before-preflight rule
continues to apply.

## 2. Admitted runtime named-record construction

This tranche admits a named-record literal as an immutable structural value
when all of the following hold:

- its selected nominal record is marked `[copy]` and is recursively copyable
  under the inherited scalar, record, and fixed-array rules;
- it names every declared field exactly once and no other field;
- the record has at most four declared fields;
- each scalar field expression is assignable by the inherited carrier and
  interval rules, and its encoded value-type interval is contained in the
  destination field's interval;
- each structural field expression has the field's exact type and supplies an
  immutable structural value; and
- each nested named-record literal independently satisfies these rules.

Runtime scalar field expressions are restricted to inherited scalar literals,
machine/block parameters, or scalar places composed only from `self` and named
field suffixes and then loaded. These forms have no call, mutable effect,
arithmetic/domain trap, index trap, or unresolved evaluation-order consequence.
A structural field may use an inherited structural parameter or a nested
constructor result. This version does not add arithmetic, comparisons, casts,
indexing, calls, or other effectful/trapping expressions inside constructor
fields; a runtime fixed-array literal; a structural load or call result; or a
place-valued record literal.

Missing, duplicate, unknown, or extra fields; scalar carrier or interval
mismatch; structural nominal mismatch; a noncopyable record or descendant; or
an unsupported field expression returns 251 before CKIR publication. A
type-correct, complete five-field named-record literal returns 252. Record
declarations retain the inherited 64-field ceiling: a larger record may still
exist, be laid out, indexed through fields, copied, or transported when an
existing structural value supplies it; only runtime literal construction has
the narrower four-field ceiling.

Construction is completed into one distinct static private frame extent for
that result value before it can be consumed. It never partially writes a
source-visible destination. Dynamic re-execution may reuse that extent only
because the result cannot escape the safe consumers enumerated below.
Assignment of a runtime constructor result therefore emits the inherited
`Copy` from that completed structural value. A record assignment whose complete
right side is recursively constant retains CKIR3 canonical lowering through
`CopyAggregateConst`; it is not rewritten as a runtime constructor followed by
`Copy`. In a call-argument value context, a named-record literal uses the
runtime constructor even when its individual fields happen to be literals,
because CKIR3 does not make its constant graph source-addressable.

## 3. Canonical field binding and operand order

Named fields do not acquire positional source meaning. The lowerer first binds
every authored field name to its exact declaration field ID and rejects the
complete name/coverage matrix. Because every admitted field expression is a
pure, non-trapping leaf form, it then lowers and materializes fields in
declaration ordinal regardless of authored field order. Nested literals recur
at that field's declaration ordinal. Only after every field value exists does
the lowerer emit `ConstructRecord` with those values in the same declaration
order; no operation is emitted merely to move a scalar into that order.

This canonicalization does not rule on Omega's still-unspecified observable
evaluation order for effectful or trapping runtime record fields; those forms
reject in this version. Authored field reordering over the admitted forms
therefore produces identical CKIR body rows. For one exact accepted OMGLOW4
frame, output is byte-identical across runs and implementation routes. Renaming
or declaration permutation retains inherited semantic guarantees but need not
preserve CKIR type IDs or bytes.

The exact checkpoint opener exercised by this tranche is the ordinary runtime
shape:

```omega
self.clear(SourceId { value: runtime_scalar });
```

where the caller and `SourceUnit::clear` are attached to `SourceUnit` in one
logical module. The construction itself does not depend on field-receiver,
cross-package-call, or distinct-module-private-access semantics.

## 4. Opcode 13: `ConstructRecord`

The inherited 40-byte operation row encodes `ConstructRecord` with opcode 13:

- result kind is 1 and the reconstructed result ID is a value;
- result type is an exact kind-4 nominal-record type whose record is marked
  `[copy]` and recursively copyable;
- both immediates are zero;
- operand count equals the record's declaration field count and is at most
  four;
- operand ordinal `i` supplies declaration field ordinal `i`;
- every operand is a visible value under the inherited machine/block and use-
  before-definition rules;
- a scalar operand is carrier-compatible with its field, its encoded type
  interval is contained in the field's exact interval, and the backend retains
  the inherited defensive exact-field check before completion; and
- a structural operand has the field's exact type and denotes an immutable,
  address-backed value.

An empty copyable record has zero operands and still produces one distinct
address-backed value. A record ID is derived from the result type; encoding it
again in an immediate would be noncanonical. No field IDs appear in the operand
vector because declaration ordinal is already exact and complete.

The result denotes a completed immutable record object. It may be:

- an exact structural argument to inherited opcode-10 `Call`;
- the mode-1 structural-value source of inherited opcode-7 `Copy`; or
- an exact structural operand of a later `ConstructRecord`.

It is not a place, pointer, reference, structural return, scalar-load source,
index base, mutable object, or source of identity observation. The complete
finite acyclic call graph and all inherited argument staging rules remain
unchanged. No other opcode, operand count, immediate, or result shape is valid.

## 5. Backend storage, lifetime, and reconstruction

### 5.1 Frame-owned constructor objects

The backend first performs every inherited CKIR, copyability, call-graph,
layout, and reachable-machine validation. For each reachable machine it then
assigns the inherited receiver, value, and place slots in their existing order.
After the last place slot and before the inherited edge/call scratch extent, it
assigns one private object extent for every `ConstructRecord` result owned by
that machine, in increasing result value ID order.

Each extent is aligned to the result type's independently reconstructed
alignment and occupies `max(SIZE(type), 1)` bytes. The one-byte anchor for an
empty record has no semantic leaf. Each constructor result retains its inherited
eight-byte structural-value slot; after construction that slot contains the
address of its own distinct object extent. Object extents are never interned or
shared merely because types or operands agree.

All object extents live for the complete invocation of their owning machine.
That conservative lifetime covers later same-block consumers and synchronous
calls while the caller frame remains live. Finite acyclic calls, the absence of
structural returns, and the prohibition on escaping references prevent a callee
from retaining the private address after the caller returns. A constructor
result value ID is not a legal direct state-edge argument: inherited structural
edges retain an address, while re-executing a constructor in a cyclic block
would reuse its private extent and could mutate an earlier logical value. A
synchronous callee may forward its inherited structural parameter across its
own state edges because the caller's extent remains stable for that complete
call. CKIR carries no transitive constructor provenance after parameter
binding. Native code uses no heap, dynamic allocation, ambient storage, or
read-only-image object for runtime construction.

Constructor storage, alignment padding, ordinary slots, call/edge scratch, and
the final 16-byte alignment all count toward the inherited machine-frame and
live-stack ceilings. Every extent and the complete frame are preflighted before
text or ELF publication.

### 5.2 Object completion

Let `OBJ(v)` be the positive frame displacement whose address
`[rbp-OBJ(v)]` is the first byte of constructor result `v`'s private extent.
`FIELD(f)`, `V(v)`, `CHECK(t)`, and the fixed register roles retain their CKIR1
definitions. The canonical operation begins:

```text
4C 8D 95 -OBJ(result)   lea r10,[rbp-OBJ(result)]
```

It then visits fields in declaration ordinal. For a scalar operand `v`, it
emits `LV(v); CHECK(field type)` followed by `41 89 82 FIELD(field)` for `u32`
or `41 88 82 FIELD(field)` for `u8`/`bool`. For a structural operand it loads
the source address with `4C 8B 9D -V(v)` and walks that field's semantic scalar
leaves in inherited declaration/index order, using the existing `r11` source,
`r10` destination copy templates with the field's destination base offset.
After every field has completed, including the zero-field case, it publishes
the result address with:

```text
4C 89 95 -V(result)     mov [rbp-V(result)],r10
```

Every accepted source and valid CKIR operand type proves its interval is
contained in the destination field interval. Inherited operation checks ensure
the runtime value lies in its encoded type, so these declaration-order field
checks are non-trapping defensive refinement for valid CKIR. No admitted field
expression contains a call, effect, or possible runtime trap; opcode 13
therefore makes no unresolved source evaluation order observable. The checks
still remain part of the exact artifact and mutation relation. They precede
completion of the structural value.
Padding and the empty-record anchor are not semantic data and need not be
initialized or copied. The destination extent is distinct from every operand
extent; any contents from an earlier dynamic execution are unreachable before
reuse. Nested constructor objects are distinct earlier extents. Existing `Copy`
retains its snapshot semantics when the completed value is later installed in
a source-visible place.

The sizing pass and emission pass independently reconstruct identical object
offsets, operation lengths, block offsets, call displacements, and ELF extents.
A producer-supplied or mismatched object offset, field offset, layout size,
instruction length, or frame address is never accepted.

### 5.3 Artifact relation

CKIR3's two-/three-segment ELF distinction remains exact. Runtime constructor
objects add no file-backed bytes and do not change the constant-image root set.
A module with no opcode-11 root still uses two segments; a module with a
constant root still uses the inherited three segments. The RW segment remains
the selected owner's zero-fill storage only; constructor objects reside in the
runtime stack frame, not BSS.

The selected entry, reachable-call closure, prologue/epilogue, argument block,
block order, trap sharing, process-exit projection, image padding, and exact EOF
rules remain those of CKIR3 except for the additional precomputed frame extents
and opcode-13 instruction bytes above.

## 6. Resources, status, and publication

All CKIR3 source, declaration, type, record, field, machine, block, parameter,
constant graph, operation, operand, value, place, layout, image, frame, live-
stack, text, ELF, and evaluator ceilings remain in force. CKIR4 adds no table
and widens no aggregate arena. Its constructor-specific resource is:

| Resource | Ceiling |
| --- | ---: |
| fields/operands in one runtime named-record constructor | 4 |

In particular, encoded CKIR remains at most 2,522,192 bytes, complete derived
constant image at most 131,072 bytes, canonical text before page padding at
most 1,048,576 bytes, and an entry-bearing ELF at most 1,183,744 bytes. The
operation count remains 32,768 and the shared operation/terminator operand-
vector count remains 94,208. Four-operand constructors consume those inherited
aggregate budgets; their per-operation arity is not permission to realize a
Cartesian product beyond them.

The selected machine frame and complete live stack remain bounded by 262,144
bytes. Under the inherited mandatory 16-byte root allowance, a selected entry
frame can be at most 262,128 bytes. Constructor object extents are ordinary
contributors to this bound, not a second storage budget. The source-side
four/five constructor tooth is distinct from a record whose valid derived
layout or constructor-bearing frame first crosses its byte ceiling.

The inherited source-only and CKIR-only evaluator limits remain respectively
16 and 64 active machine frames and 65,536 dynamic block entries. Evaluator
storage for completed constructor objects is checked within its published
model; it may not turn object count into an unstated fuel or acceptance limit.

Status 0 means complete success. Malformed framing, noncanonical tables,
invalid IDs or spans, wrong constructor result/operand relation, missing or
duplicate source fields, type or mutability failure, noncopyable construction,
unsupported expression context, recursive layout, invalid call graph, or
target mismatch returns 251. A validated extent above the four-field source
constructor ceiling or an inherited source, table, CKIR, layout, frame, live-
stack, text, image, ELF, or evaluator ceiling returns 252. Arithmetic overflow
while decoding a purported encoding is 251 unless an already validated public
extent selects 252. Status is monotonic once 252 is selected.

Constructor field-name coverage, expression support, copyability, and complete
field typing are semantic checks and precede the four/five resource decision.
Thus a malformed, missing/duplicate/unknown-field, mistyped, or noncopyable
five-field literal is 251; only an otherwise source-valid complete constructor
crossing the published four-field ceiling selects 252.

The lowerer emits no CKIR byte until exact source checking, declaration-order
field materialization and operand construction, all graph/resource checks,
exact EOF, and complete output sizing succeed. The backend emits no ELF byte
until full CKIR validation, layout, constructor-object/frame assignment,
reachable-call closure, text/displacement/segment sizing, and exact EOF
preflight succeed.
Status 251 or 252 always has empty stdout.

## 7. Explicit exclusions and non-authority

CKIR4 does not add runtime fixed-array literals, positional records, record
update/spread, aggregate locals, mutable constructor values, constructor
identity, structural loads, structural returns, aggregate call results,
direct constructor-result value IDs as state-edge arguments, aggregate
transition literals, or constant evaluation of runtime expressions. It does
not add payload
sums, slices, strings, allocation, source pointers/references, recursion,
imported or cross-package calls, or private access between distinct
logical modules.

The original OMGLOW4/OMGRSW1 source relation excludes field receivers. The
versioned OMGLOW5/OMGRSW2 relation admits only direct same-module
`self.field.machine(...)`; computed, indexed, parameter, parenthesized, chained,
distinct-module, imported, and cross-package receivers remain excluded.

It also leaves unchanged the inherited exclusions of general boundary calls,
generics, domains, proofs, atomics, threads, exceptions, target generality,
optimization, debug information, and full-width authored `u32` beyond the
signed-D0 carrier. The exact `compiler/psi/source/source.omg` evidence uses a
same-logical-module, same-owner harness and does not rule on those surfaces.

Frame object offsets, padding, anchors, pointer slots, and instruction bytes are
private implementation evidence. They are not source-observable identity,
public ABI, FFI layout, wire identity, hashing input, or permission for unsafe
address use. This tranche does not decide final `Ωself` retention of named-
record literals. It measures the regular form against alternatives while
preserving CKIR3 and every earlier artifact contract.

## 8. Required evidence before use as an artifact tranche

All inherited CKIR3 producer, backend, self-build, Rust-free meaning, mutation,
resource, and lower-rooted obligations remain mandatory. CKIR4 additionally
requires:

1. The exact `compiler/psi/source/source.omg` bytes plus a same-logical-module
   harness compile through OMGLOW4. A harness machine attached to `SourceUnit`
   passes `SourceId { value: runtime_scalar }` through the existing same-owner
   structural `Call` transport to `SourceUnit::clear`, then observes the copied
   `self.id.value`; the selected Linux image exits 70 with empty stdout/stderr.
   No producer branch recognizes `SourceId`, `SourceUnit`, a filename, a field
   spelling, or the expected result.
2. General positive controls cover renamed and declaration-reordered records,
   authored-field reordering with identical canonical body rows,
   empty and one-through-four-field copyable records, nested copyable records,
   scalar literal/parameter/named-field-load children, structural parameter and
   nested-constructor children, constructor-to-`Copy`, constructor-to-`Call`,
   and explicit rejection of the opcode-13 result value ID as a direct state-
   edge argument. Repeated production is byte-identical; source renaming and
   ordering preserve behavior without requiring accidental identical type IDs.
3. Phase-isolated source negatives cover missing, duplicate, unknown, and extra
   fields; scalar carrier/range mismatch; structural nominal mismatch;
   noncopyable and recursively noncopyable records; unsupported runtime fixed-
   array literals; arithmetic, comparison, cast, indexing, call, or other
   effectful/trapping field expressions; constructor use where a place or
   structural return is required; and malformed nested construction. Each
   returns 251 with empty stdout. An otherwise valid five-field literal is the
   distinct 252 control.
4. Independent CKIR negatives mutate opcode 13's owner, result kind, dense
   result ID, nominal result type, copyability, operand start/count/order,
   operand visibility/type, both immediates, and zero-/one-/four-field boundary.
   Further mutations target constructor-object alignment, size, distinctness,
   value-slot publication, every scalar field store/range check, every nested
   semantic-leaf copy, frame extent, call argument pointer, and each affected
   rel32 or RIP-independent instruction byte. Valid-but-
   mismatched CKIR/result and CKIR/ELF pairs reject.
5. Exact and adjacent resource teeth cover four/five constructor fields,
   constructor counts against operation/value/operand aggregates, empty-record
   anchors, nested object alignment, selected-machine frame and complete live
   stack, text, encoded CKIR, ELF, active evaluator frames, and dynamic block
   entries. Evidence establishes greatest realizable related maxima rather
   than manufacturing impossible Cartesian products.
6. Native and persisted-Delta-self-built OMGLOW4 lowerers and CKIR4 backends
   publish identical CKIR and ELF bytes for every positive and agree on complete
   0/251/252 observations. The Rust-free Delta-to-Gamma meaning route
   independently reconstructs constructor values, structural argument
   transport, destination copy, selected result, and exhaustion. A Rust product
   compiler remains differential evidence only.
7. Lower-rooted refinement uses the distinct
   [`OMGRFN5`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V5.md)
   carrier embedding each selected exact unchanged OMGCOMP, exact unchanged
   OMGRSW1, CKIR4, ELF, and claimed
   result. Independent responsibilities establish: frame/component custody;
   source-to-OMGRSW1 reconstruction; OMGRSW1-to-CKIR4 tables; source-body-to-
   opcode-13 lowering with declaration-order canonicalization; source-only result
   without CKIR/ELF access; and CKIR4-only result plus exact ELF reconstruction.
   Responsibility-specific executables are permitted and required when one
   lower-rung procedure/local/tape ceiling cannot contain the conjunction.
8. One same-carrier composite invokes every lower-rooted responsibility over
   each immutable canonical carrier. It covers both the original runtime-record
   opener and a second complete-current-`SourceUnit`-API program built entirely
   from already-admitted CKIR4 forms. The second changes call count, block-
   parameter count, selected result type, binding count, and witness extent so
   fixture-census recognition cannot satisfy the relation. Phase-local
   mutations and valid-but-mismatched source/witness, witness/CKIR, CKIR/ELF,
   and result cross-pairs isolate each join. The source-only result executable
   is physically pruned of CKIR, ELF, and artifact-derived evaluator access;
   artifact-only checkers do not read source bodies beyond their stated
   envelope/table premises. The composite reports each component's build/run
   time and procedure, local, and tape usage rather than hiding a resource
   failure in fixture products.

Only that evidence closes this bounded runtime named-record-construction
source-to-artifact relation. It does not close slices, payload sums, boundary
intrinsics, the separate OMGRSW2 field-receiver relation, structural returns,
or the complete checkpoint-000001 compiler path, and it grants no compilation
authority without the separately accepted lock/closure and exact OMGCOMP
commitment join.

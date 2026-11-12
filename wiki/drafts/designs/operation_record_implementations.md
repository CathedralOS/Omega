# Operation-record implementations

Ratified 2026-11-09 as documentation. An implementation is an ordinary checked
record of callable selections and evidence; a trait declares its schema and a
`satisfies` block constructs a named value of it. The rule lives in
[conformances](../../spec/language/conformances.md#implementations-are-operation-records)
and [static callable selections](../../spec/language/machine_values.md#static-callable-selections).
It supersedes `inline-conformance-members` (2026-10-16). No compiler support or
fixture outcome is claimed here. Delete this record when
`OPERATION-RECORD-IMPLEMENTATIONS` on [TASKS.md](../../../TASKS.md) lands and the
spec is restated in these terms.

The 2026-11-11 decisions `operation-record-source-forms` and
`implementation-telescope-identity` complete the source rules below. They add
ordinary fields and constraints to trait schemas, not special law, lemma or
brand declarations. The examples specify the required source behavior, not
compiler execution results.

## Context

Runtime dispatch became ordinary records with machine-valued fields on
2026-11-06. Compile-time dispatch stayed a separate species: named implementation
maps selected through evidence binders. The same interface was therefore
declared twice, once as a trait and once as a hand-written operations record.
Reusing an existing machine in a conformance required a forwarding member, a new
declaration whose laws had to be re-established. Dictionary transformations had
to be generic conformance declarations, and derived implementations needed
reflection selection contexts because conformance evidence was not a value.

## Decisions

1. A trait declares named callable requirements, with identities preserved, and
  an ordinary record schema of one selected operation per requirement plus
  explicitly declared ordinary fields and constraints. An implementation is a value of that schema.
   Ordinary code constructs, passes, projects and returns such values, including
   from compile-time evaluation.
2. The new value form is the static callable selection: an exact declaration
   with its supplied static arguments, complete contract, remaining generic
   parameters and any binding to a named requirement. It is a canonical static
   atom identified by declaration and static application. It has no runtime
   representation.
3. Selection rows return. `Trait::member = Existing::machine;` in a `satisfies`
   block, or a field in a record literal, selects an existing machine and keeps
   its identity. An inline body still declares a new machine.
4. Every `machine` member contributes a selection bound to its exact named
  requirement, even when resultless or used as a theorem. An ordinary typed
  field stores a value of its type; `[erased]` removes runtime storage only.
  A schema `requires` clause constrains the assembled record. Callable binding,
  operator selection and authority routes remain explicit.
5. Identity is the schema application plus canonical relevant field values.
  Names and unused constructor arguments create no identity, including for
  empty and generic implementations. Actual schema indices, selected machine
  applications and carried lifetime obligations remain. Intentional distinctions
  use ordinary schema parameters or relevant fields, never a hidden brand.
6. Explicit evidence fields follow their mathematical types: strict proofs of
  the same complete proposition are irrelevant, relevant witnesses retain
  identity when erased, and selected machines retain declaration/application
  identity. General checked citation supplies evidence in ordinary records as
  well as traits; it does not store a callable or erase trust dependencies.
7. Construction establishes every constraint against the final selected
  operations. Replacing an operation requires repairing dependent evidence and
  constraints before reuse. No unfinished guarantee is assumed, and fields introduce no
  implicit receiver or captured state.
8. One schema serves static and runtime use. A static parameter exposes exact
   selections; the parameter `const Order: Ranked<T>` denotes one. A
   runtime value carries callable descriptors under checked envelopes. An open
   generic callable family is not runtime-materializable. The normalized field
   type records whether its contract is open, a record refinement supplies the
   finite runtime bounds, and omission is never reinterpreted by use site.
   Contracts stay per operation.
9. Defaults are constructor computation over already-selected prerequisites,
   never over the record under construction, and capture nothing. A default
   needing runtime-selected siblings takes the record or a context explicitly.
10. Inheritance contributes one field per originating member, not merged parent
  implementation values. Qualified field labels disambiguate distinct origins;
  short labels require uniqueness. Construction supplies one final initializer
  per field, and conflicting defaults require an explicit choice. Parent order
  never resolves them. Projected callable fields retain their existing binding.
11. Transparent refinements are domains over the record. Adapters are checked
    record-to-record transformations. Derivation is compile-time code returning
    records under ordinary typed reflection and access rules.
12. Operator tokens stay bound to named requirements. Static selection chooses
    the operation; runtime records use explicit field calls.
13. Authority stays with its source. The existing Provider argument of
  `Build::select_provider` accepts a fully evaluated static record, without a
  naming wrapper, and checks exact slots, realizations and admission. Packing a
    selection registers no provider and manufactures no satisfaction,
    establishment or cleanup authority.
14. No associated-type surface. Related types are parameters; hidden types use
    `some`. Selection stays explicit; nothing becomes an ambient default.

## Sketch

```omega
trait Codec<T> {
    machine encode(value: &T, out: &mut [u8]) -> u64;
    machine decode(input: &[u8], out: &mut T) -> bool;
}

WireCodec satisfies Codec<Message> {
    Codec::encode = lib_a::encode;
    Codec::decode = lib_b::decode;
}

machine save_static<T, const Format: Codec<T>>(value: &T, out: &mut [u8]) -> u64 {
    Format::encode(value, out)
}

machine save_dynamic<T>(format: &Codec<T>, value: &T, out: &mut [u8]) -> u64 {
    (format.encode)(value, out)
}
```

## Migration

- Add the static callable-selection atom and its canonical encoding.
- Lower `trait` to requirements plus schema and `satisfies` to record
  construction; admit selection rows and record literals of the schema.
- Restate evidence binders, row identity, binder contract dependencies and
  result relations over static record parameters and field identities.
- Carry open or closed field contracts in normalized types; check runtime
  materialization and record refinements.
- Move reflection-derived implementations to compile-time code returning records;
  retire selection contexts where the record form covers them.
- Complete ordinary fields, erased evidence and schema constraints in trait
  declarations and constructors; use general evidence application, not hidden
  resultless-member classification.
- Finish result identity where a constructor still stands for itself: an
  instantiated default, two generic constructors of one schema application, and
  an implementation passed as a static argument on a call. Selection-row and
  empty constructors already compare by the value they construct
  (`pass/traits/implementation_value_unused_const_one_value`,
  `run/traits/implementation_value_same_selections`); keep the schema-index,
  selected-machine and lifetime/custody mismatch controls.
- Audit empty-marker and same-selection customers; place intended distinctions
  in actual schema parameters or relevant fields. A wrapper's distinction does
  not survive projecting its identical operations.
- Admit computed provider values through the existing Build argument while
  retaining all selected supply and admission dependencies.

## Explicit evidence construction

These two containers use the same expected-evidence application. The trait's
machine member is a bound selection; the ordinary data record takes that exact
selection as a static parameter. Neither initializer treats the machine's
resultless return as proof data.

```omega
let positive_number(value: u32): core::Strict<0> = value > 0;

trait PositiveCodec {
    machine encode(value: u32) -> u32
        reaches none
        crashes none
        terminates;
    positive [erased]: (value: u32) -> positive_number(encode(value));
}

machine encode_one(value: u32) -> u32
    reaches none
    crashes none
    terminates
    ensures result == 1
{ 1 }

machine prove_positive(value: u32)
    reaches none
    crashes none
    terminates
    ensures positive_number(encode_one(value))
{}

const selected_encode = (encode_one satisfies PositiveCodec::encode);

const OneCodec: PositiveCodec = PositiveCodec {
    encode: selected_encode,
    positive: prove_positive,
};

data EncoderEvidence<const Encode: PositiveCodec::encode> {
    positive [erased]: (value: u32) -> positive_number(Encode(value));
}

const ordinary: EncoderEvidence<selected_encode> = EncoderEvidence {
    positive: prove_positive,
};
```

In both initializers, the expected field type introduces an arbitrary `u32`
input and checks the citation's guarantee at the exact installed encoder.
Changing only the encoder to one returning zero rejects; proof of a different
operation is not evidence for this field. Adding a premise not available for
every admitted input also rejects. An operational reset cannot substitute for
this mathematical evidence.

To store a particular proof-producing machine instead, declare an ordinary
`machine positive(...) ...;` requirement and supply its bound selection. Two
different declarations then give different field values. Relevant witness
evidence likewise follows its actual type; `core::Identity` is not silently
squashed into strict evidence. A simple positivity guarantee may instead be
written directly in `encode`'s `ensures`, with no additional evidence field.

## Acceptance

- A codec assembled from two existing machines and their proof needs no
  forwarding member, and the existing law applies to the exact selections.
- One trait declaration is used through a static parameter and as a runtime
  record with no second operations record.
- Two names for equal records are one implementation, including empty and
  generic constructors with different unused arguments. Actual schema indices
  and relevant fields distinguish values; projected equal operations do not
  retain a wrapper's identity.
- Two citations supplying strict evidence for the same complete field type
  yield one value, in an ordinary record and a trait schema. A relevant witness
  retains value identity, and two selected proof-machine declarations differ.
- Wrong-encoder evidence, unaccounted premises and operational-reset supply
  reject. Assumption and supply dependencies survive equal-value canonicalization.
- A record with an open generic member rejects runtime materialization and
  accepts after closing.
- A default cannot specialize against the record under construction.
- A diamond yields one field for the same origin; different origins with the
  same name require disambiguation. Qualified final initialization accepts an
  already-bound selection; conflicting defaults reject without it. Evidence
  concerning a discarded operation rejects.
- Construction, projection, forwarding and mutation follow ordinary dependent
  record checking. Updating an operation invalidates dependent evidence, and
  ordinary fields never introduce implicit receivers or runtime captures.
- A computed record offered through Build's existing Provider argument is
  checked for complete schema, constraints, native supply, target realization
  and admission, without a naming wrapper or authority from construction.

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
   an ordinary record schema of one selected operation per requirement plus the
   evidence its laws require. An implementation is a value of that schema.
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
4. Named requirements stay a separate constituent. A field is a structural
   selection or a selection bound to a named requirement, preserving
   requirement-routed invocation, token binding and domain access routes.
5. Identity is the schema application plus canonical relevant field values.
   Names create no identity and grant no authority. Separately declared machines
   are different selections even with identical bodies. A deliberately distinct
   policy carries an explicit brand.
6. Law fields contain evidence, and identity follows its logical sort: a strict
   proof is irrelevant, a stored theorem selection keeps its declaration
   identity, a relevant witness stays relevant when erased. Citing a theorem to
   supply evidence stores no reference to it. A consumer may index a type by an
   operations subrecord and carry laws as erased evidence.
7. Construction proves the laws for the selected operations. Replacing an
   operation re-establishes them.
8. One schema serves static and runtime use. A static parameter exposes exact
   selections; the binder spelling `Order satisfies Ranked<T>` denotes one. A
   runtime value carries callable descriptors under checked envelopes. An open
   generic callable family is not runtime-materializable. The normalized field
   type records whether its contract is open, a record refinement supplies the
   finite runtime bounds, and omission is never reinterpreted by use site.
   Contracts stay per operation.
9. Defaults are constructor computation over already-selected prerequisites,
   never over the record under construction, and capture nothing. A default
   needing runtime-selected siblings takes the record or a context explicitly.
10. Parent traits contribute one slot per originating requirement. Diamonds
    duplicate nothing; conflicting selections need an explicit choice.
11. Transparent refinements are domains over the record. Adapters are checked
    record-to-record transformations. Derivation is compile-time code returning
    records under ordinary typed reflection and access rules.
12. Operator tokens stay bound to named requirements. Static selection chooses
    the operation; runtime records use explicit field calls.
13. Authority stays with its source. Build may consume a computed record and
    still checks exact slots, realizations and admission separately. Packing a
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

machine save_static<T, Format satisfies Codec<T>>(value: &T, out: &mut [u8]) -> u64 {
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
- Audit the library for implementations that share every selection and rely on
  distinct names; add brands where the distinction is intended.

## Acceptance

Four cases below have no source spelling yet (brand, stored theorem selection,
explicit choice, computed record offered to Build); see
`operation-record-source-forms` in [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md).

- A codec assembled from two existing machines and their proof needs no
  forwarding member, and the existing law applies to the exact selections.
- One trait declaration is used through a static parameter and as a runtime
  record with no second operations record.
- Two names for the same selections are one implementation; a branded pair is
  two.
- A strict law proved by two different lemmas yields one implementation; a
  stored theorem selection yields two.
- A record with an open generic member rejects runtime materialization and
  accepts after closing.
- A default cannot specialize against the record under construction.
- A diamond yields one slot for the shared requirement; conflicting parents
  reject without an explicit choice.
- A computed record offered to Build is checked for slots, realization and
  admission and gains no authority from its construction.

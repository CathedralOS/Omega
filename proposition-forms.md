# proposition-forms

## still_open

True

## size

large

## depends_on

['Part A1 depends on, or should absorb, TASKS.md:314: move the per-obligation carrier table of equality::discharge into the mathematical signature, so a record has one kernel denotation', 'Part B4 changes ratified wire text (encoding.md proposition tags and marker 294). It is routine by precedent (marker 290 retired a tag), but encoding.md is spec, so the same PR edits that page', "B_pos (`Identity::<Outer>(result, o)` failing with 'no whole-value input correspondence', psi/semantics/src/substitution/clauses.rs:52) is a separate gap in tracing a returned value back to its input. Part A does not need it, but A_pos/A_neg cannot become a separating twin until it is fixed", 'E_pos (case field of another type, read/signature.rs:682-689) waits on the native inductive ruling (wiki/spec/proofs/inductive_profile.md#native-inductive-declarations), not on this item']

## evidence

The map's two claims are each partly stale. Field projection works for mathematical records; it is missing for ordinary records that live in storage.

THE TYPE. `Proposition` is at source-rust/subsystems/compiler/foundation/semantic-records/src/proposition.rs:1993-2051. It is a shared type: the lowered program, Terminal Psi and the Psi wire all use it.
- The five older equality forms:
  - `Equal(ScalarTerm, ScalarTerm)` (:1997)
  - `IntegerMathEqual` (:2000)
  - `ByteSequenceEqual` (:2015)
  - `StructuralIdentity` (:2019)
  - `FloatMeaningEqual` (:2034)
- The general form is `DeclarationIdentity(ClauseIdentity)` (:2050). `ClauseIdentity` is defined in foundation/semantic-records/src/clause_identity.rs:5-41:
  - carriers: Parameter, Scalar, Structural
  - operands: Scalar, Input(root + path), Formal, Application{declaration, arguments}
  - left/right are slots in its value table.

WIRE (wiki/spec/terminal-psi/encoding.md:812-863, current marker 293 at :45). The five older forms are portable tags 4, 14, 12, 18 and 19. Tag 23 (DeclarationIdentity) is deliberately not portable:
- The encoder refuses it in a module (declaration-encoding/src/terms/proposition.rs:266-273).
- Validation refuses it too: "unpublished declaration Identity in a portable proposition" (psi/artifact/wire/src/foundation_validation.rs:6085).
- Tag 23 is written only into the checker's ordering key.

DUPLICATES FOUND IN THE CODE.
1. `StructuralIdentity` has no producer. The only call sites of `StructuralIdentitySubject::new` are the decoder (declaration-encoding/src/terms/proposition.rs:382), substitution (psi/semantics/src/continuation/predicates.rs:284) and one test. It is the portable copy of `ClauseIdentity` with Input operands at a Structural carrier.
2. A scalar Identity has two spellings.
   - psi/artifact/wire/src/contract_forms.rs:12-32 sends `Equal` to `DeclarationIdentity` whenever one side is a `ScalarTerm::DeclarationApplication`.
   - stage05 body.rs:1096-1126 converts a ClauseValue table back into a `ScalarTerm::DeclarationApplication`.
3. `FloatMeaningClause` (values with Projection/Application slots, then left/right) has the same shape as a ClauseIdentity table. Source writes `core\Identity::<FloatMeaning>` and stage 05 (read/mathematics.rs:224) turns it into `FloatMeaningEqual`.

KERNEL TRANSLATION. `denote_leaf` in psi/semantics/proofs/bounded/bounded_denotation.rs:5560-5622 maps `Equal`, `IntegerMathEqual` and `ContentConservation` to a kernel `Id`. `ByteSequenceEqual`, `StructuralIdentity`, `FloatMeaningEqual` and `DeclarationIdentity` fall to `atom` at :5621, so the kernel sees them only as opaque `Type 0` assumptions.

STAGE 06.
- prove.rs:2068-2071 gives "goal form outside this prover" for ByteSequenceEqual/StructuralIdentity goals.
- prove.rs:1957 sends DeclarationIdentity to `equality::discharge` (lib.rs:993, joins.rs:1087).
- In equality.rs:2358-2410, `Declarations::identity` makes each Input with an empty path an opaque constant (`Entry::Input`), and each scalar field read an unrelated opaque constant (`Entry::Scalar`, :2351). An Input with a path is refused: "no whole-value mathematical input for this clause operand" (:2400).
- `cite` (:2441) matches only Inputs with empty paths.

PROJECTION. For mathematical records it is built: read/signature.rs:22-26 and 2039-2090 (records become Σ products, field reads become Fst/Snd chains). The stage 05 reader turns `a.f` into an Application of the field declaration, but only when `self.mathematical` is set (read/clause_terms.rs:1689-1765). For data that has storage, it falls through to :1766, "Identity operand has no term at its written carrier".

PROGRAMS RUN. Binary target/debug/omega.exe with `--check --target windows_x86_64`, against std extracted from 1f08b50ac2da: the current std fails with this older binary at std/build.omg:37 (`bind_native`). Sources are under build/diag-proposition/. Results:
- D_pos, mathematical `Pair`: `ensures core\Identity::<Peano>(swap(p).left, p.right)`. Exit 0, "compiled 155 source file(s)".
- D_neg, same with `p.left`: refused with "cannot prove ensures contract for `swap_left`: core\\Identity::<Peano>(swap(p).left, p.left). (no proof found from the facts available here: no available equality proves the stated Identity)". Projection over mathematical records works.
- C_pos, runtime `Outer{a: Inner, b: u32}`: `ensures core\Identity::<u32>(result.b, o.b)`. Exit 0.
- A_pos, `ensures core\Identity::<Inner>(result.a, o.a)` returning `o`: refused, "not supported yet: contract Identity operand has no term at its written carrier while reading the signature of `keep`". A_neg (returns a fresh Inner) is refused identically, so the two cannot be told apart.
- B_pos, `ensures core\Identity::<Outer>(result, o)`: refused, "not supported yet: no whole-value input correspondence in machine `keep`" (psi/semantics/src/substitution/clauses.rs:52). This is a separate gap in tracing a returned value back to its input.
- F_pos, `data Point { x: i32; y: i32; }`, `requires core\Identity::<i32>(a.x, b.x)`, `requires core\Identity::<i32>(a.y, b.y)`, `ensures core\Identity::<Point>(a, b)`, empty body: WRONGLY REFUSED with "cannot prove ensures contract for exit from same_point at statement 0: core\\Identity::<Point>(a, b). (no proof found from the facts available here: no available equality proves the stated Identity)". F_neg (only the x premise) gets the same text, so the positive and its negative twin are indistinguishable.
- E_pos, mathematical `data Bag { case Empty; case Full(item: Peano, rest: Bag); }`: refused, "not supported yet: machine data that contains itself with a field of another type" (read/signature.rs:682-689). This is a separate encoding gap.

SPEC.
- wiki/spec/proofs/contracts.md:64-66: "the checker must derive `core\Identity::<R>(a, b)` when it has identity evidence for every corresponding field of those exact values". :70-72 says the checker proves it "by reconstructing the records from their projections".
- TASKS.md:306 owns this customer (the plain `Point`; a refusal that names `y`).
- TASKS.md:314 owes moving the per-obligation table of `equality::discharge` into the mathematical signature.

DESIGN CONSTRAINT. wiki/drafts/designs/proof_kernel_core_audit.md:622 (L2): "The existing scalar, integer and float forms stay as the cases the arithmetic rules work on." `Proposition::Equal` has 778 uses across 118 files, including integer_affine.rs (94), bounded_denotation.rs (104) and integer_rules.rs (34). wiki/spec/proofs/kernel_metatheory.md:353-360 already treats scalar `Equal` as `Id` over its carrier and IntegerMath as `Id Int`.

## spec

wiki/spec/proofs/contracts.md#record-identity-from-field-identities (":64-66 the checker must derive `core\Identity::<R>(a, b)` when it has identity evidence for every corresponding field", proved "by reconstructing the records from their projections"). wiki/spec/proofs/contracts.md#data-identity-in-facts (Identity "explicitly forms the foundation's proof-relevant `Id T left right`"). wiki/spec/terminal-psi/encoding.md#propositions (tags and the non-portable tag 23, :812-863; marker policy, :45-65). wiki/spec/proofs/kernel_metatheory.md:352-360 (scalar `Equal` denotes `Id` over its carrier). wiki/spec/terminal-psi/mathematical_values.md#float-equality-and-source-identity (float meaning equality is kept distinct from raw identity, so FloatMeaning gets its own carrier).

## cause

There are two causes, one for each half of the item.

(1) Equality has one form per kind of operand, and the general form cannot leave the compiler. `DeclarationIdentity` was added beside the five older forms, not in their place, and it cannot be published, so the portable forms had to stay.
- The decoder still owns `StructuralIdentity` (proposition.rs:2019; tag 18), even though nothing writes it.
- The general form duplicates `Equal` on scalar carriers (contract_forms.rs:12-32 chooses between them; body.rs:1096 converts one into the other).
- `FloatMeaningEqual` duplicates the general form's value table.
- The kernel translation denotes the non-arithmetic forms as opaque atoms (bounded_denotation.rs:5621) instead of `Id`.

(2) Stage 06 cannot use a field projection of data that has storage. The per-obligation kernel table in `equality::Declarations` (equality.rs:2266-2410) declares every Structural carrier as an opaque `Type 0` assumption (`carrier`, :2292). It makes each scalar field read `a.x` its own opaque constant, unrelated to the constant for `a` (:2351), and refuses an Input with a path (:2400). So the premises `Id i32 a.x b.x` and `Id i32 a.y b.y` say nothing about `Id Point a b`.

Stage 05 makes this worse for non-scalar fields. Its reader serves `a.f` only for mathematical data (clause_terms.rs:1692, the `if self.mathematical` gate) and refuses `result.a` at :1766.

Mathematical records do not have this problem, because signature.rs:2041 `product` gives them a Σ body. That mechanism is not used for records that have storage.

## code_locations

['source-rust/subsystems/compiler/foundation/semantic-records/src/proposition.rs:1993-2051', 'source-rust/subsystems/compiler/foundation/semantic-records/src/clause_identity.rs:5-69', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/clause_terms.rs:1291,1608-1800', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/signature.rs:22-26,682-689,1939-1948,2041-2090,3686-3716', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/body.rs:1096-1126', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/equality.rs:2266-2617', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs:993', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/prove.rs:1957,2058-2072', 'source-rust/subsystems/compiler/psi/artifact/declaration-encoding/src/terms/proposition.rs:112-401', 'source-rust/subsystems/compiler/psi/artifact/wire/src/contract_forms.rs:12-43', 'source-rust/subsystems/compiler/psi/artifact/wire/src/foundation_validation.rs:6085-6110', 'source-rust/subsystems/compiler/psi/artifact/wire/src/semantic_module.rs:4163', 'source-rust/subsystems/compiler/psi/semantics/proofs/bounded/bounded_denotation.rs:5560-5622', 'source-rust/subsystems/compiler/psi/semantics/proofs/bounded/predicate_denotation.rs:117-131,506', 'source-rust/subsystems/compiler/psi/semantics/src/structural/structural_effect/byte_equal.rs:33', 'source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs:333-390', 'wiki/spec/terminal-psi/encoding.md:45,812-863', 'wiki/spec/proofs/contracts.md:62-78', 'TASKS.md:306,314', 'wiki/drafts/designs/proof_kernel_core_audit.md:47,585-589,622']

## repair_plan

Land two parts. Part A (projection) is what TASKS.md:306 needs and blocks nothing else, so do it first. Part B is the form cleanup the item names.

PART A: field projection as a kernel term, for every record value.
A1. One kernel denotation of a record carrier.
- Widen `product` in stage 05 read/signature.rs:2041-2090 to accept any record that has no cases and no payload-bearing sum. Each field's carrier is:
  - a scalar: an assumed `Type 0` constant, keyed by `ScalarType`, the same key `Entry::Carrier(Scalar)` uses today;
  - a record: its own product;
  - a type parameter: a variable.
- Refuse a record that has a descriptor or view field: contracts.md:77 says descriptor identity proves nothing about the storage it refers to.
- Refuse a type parameter of opaque structure for decomposition: contracts.md:74 says an opaque type parameter supplies no field decomposition.
- Include every field, erased ones too. Then "a visible subset of fields is insufficient" (contracts.md:75) holds by construction.
- Record the result in the mathematical signature as `SignatureEntryKind::Record`, as today.
- Do this together with TASKS.md:314: `equality::discharge` should take its carriers from the signature instead of creating `Entry::Carrier` assumptions (equality.rs:2292). Without that move there are two denotations of one record, which rule 6 forbids.

A2. One representation of a field read in a clause: the field's declaration applied to its value, `ClauseOperand::Application{field declaration, [value]}`. This is what clause_terms.rs:1689-1765 already does for mathematical data.
- Remove the `self.mathematical` gate at :1692.
- Have the Name/subject branches (:1619-1664) and the scalar branch (:1608-1618) produce the same Application whenever the operand is a field read and an Identity is being formed, instead of `Input` with a path or a `ScalarTerm::IntegerField`.

A3. In stage 06, `Declarations::identity` (equality.rs:2358-2410) and `cite` (:2412-2485) denote a field Application through the signature's record entry as an Fst/Snd chain over the term of its argument. This matches signature.rs:1939-1948, which builds the same chain; share that function instead of copying it.
- An Input with a path normalizes to the same chain, or stays refused. Never treat it as an opaque constant.
- Delete the opaque `Entry::Scalar` fallback for field terms (:2351) so the same field cannot be denoted twice.
- The prover already folds a tuple of every field of one value back into that value by pair eta (equality.rs:670-671), so F_pos then closes from its two premises.

A4. Make the refusal name the missing field. When `prove` returns NoProof on a record Identity whose sides are a value and its field tuple, report the component equality left unproved. TASKS.md:306 requires a refusal "identifying `y`".

PART B: replace the five older forms (rule 6).
B1. Rename `Proposition::DeclarationIdentity` to `Proposition::Identity`. Widen its carriers and operands so that each older form becomes one instance of it:
- `ClauseCarrier::MathematicalInteger` with `ClauseOperand::Integer(IntegerMathTerm)` replaces `IntegerMathEqual`.
- `ClauseCarrier::FloatMeaning(IeeeFloatFormat)` with `ClauseOperand::Meaning{source, projection contract}` and a catalog `Application{contract, format, operands}` replaces `FloatMeaningEqual`. Move `FloatMeaningClause::validate`'s catalog checks into `ClauseIdentity::validate`.
- `ClauseCarrier::ByteContent` with `ClauseOperand::Content(StructuralCaseSubject)` replaces `ByteSequenceEqual`. Contents are a projection of a view; they are not the descriptor's value, as contracts.md:77 requires.
- Input/Structural replaces `StructuralIdentity`.

Delete those four variants and every match arm on them. The consumers are listed below.

B2. Scalar equality: keep the inline `Equal(ScalarTerm, ScalarTerm)`, but as the only spelling of an identity between two scalar terms, and delete the duplicate route.
- `ClauseIdentity::validate` refuses a table whose two endpoints are both `Scalar` operands at a `Scalar` carrier.
- contract_forms.rs:12-32 always yields `Equal`.
- lib.rs:993 sends `Equal` with a `DeclarationApplication` operand to `equality::discharge` (or to a widened `Declarations::scalar`).
- body.rs:1096 is deleted.

Why not fold `Equal` into the table: it has 778 uses as the input to the arithmetic rules, it is allocation-free, and audit L2 together with kernel_metatheory.md:353-360 already define it as `Id` at a scalar carrier. Putting it in the table would add a Vec allocation to the hottest proposition, which the data-oriented storage rule forbids, and the arithmetic engines would still need their own view of it.

B3. Kernel translation. `bounded_denotation::denote_leaf` (:5580-5621) denotes `Identity` as `Id carrier left right`, built with the same operand-to-term function stage 06 uses. That removes four opaque atoms: those forms become judged `Id`s rather than assumptions.
- The bounded `FloatMeaningNormalization` rule (psi/semantics/proofs/bounded/float_meaning.rs:61) reads the FloatMeaning carrier of `Identity`.
- In predicate_denotation.rs:117-131 and 506, the reflexive check works on the table.

B4. Wire. Make the portable subset of `Identity` publishable:
- carriers Scalar, Structural, MathematicalInteger, FloatMeaning, ByteContent;
- operands Input, Integer, Meaning, catalog Application, Content.
An Application of a source declaration or a Formal stays unpublishable (refused with the current message).

Give that subset one row tag, reusing 23 and its layout from encoding.md:845-859, extended with the new carrier and operand tags. Retire tags 12, 18 and 19; keep 4 and 14 for `Equal` and `IntegerMathEqual`. Bump the semantic module marker from 293 to 294 in psi/artifact/wire/src/semantic_module.rs:4163, and add a "Marker 294" paragraph near encoding.md:45. Readers reject earlier markers, as marker 290's retirement did. foundation_validation.rs:6085-6110 moves the StructuralIdentity endpoint and root checks and the tag-19 catalog-commitment checks onto the new row.

CONSUMERS (all must change in the same PR):
- Stage 05: read/clause_terms.rs:1291, 1608-1800; read/contracts.rs:1173 and 2349-2350; read/mathematics.rs:209-224; read/float_meaning.rs:92; read/casts.rs:207; lower/places_contents.rs:451; read/signature.rs:1512, 2261, 3693-3716; canonical.rs:367-398; read.rs:3368; body.rs:1096.
- Stage 06: equality.rs:2266-2617; lib.rs:993; joins.rs:218-219 and 1087; prove.rs:151, 1957 and 2058-2072; prove/float_meaning.rs:21-72; reuse.rs:416, 1493 and 2556; verdict.rs:1251; known.rs.
- Stage 07: source_selection_publication.rs.
- Psi wire: declaration-encoding/src/terms/proposition.rs:112-401; terms/order.rs:62-71; wire/src/contract_forms.rs:12-43 and 154-187; foundation_validation.rs:6085-6110; semantic_module.rs:4163.
- Psi semantics: substitution.rs:109-351; substitution/clauses.rs:479-492; continuation/predicates.rs:254-284; closed-predicates/budget.rs:74-85, guard.rs and ieee.rs; structural_effect/byte_equal.rs:33 (the producer of ByteSequenceEqual); inhabitation.rs; numeric_primitives.rs.
- Bounded checker and kernel translation: proofs/bounded/bounded_denotation.rs:5560-5622; predicate_denotation.rs:70-131 and 506; float_meaning.rs:61; kernel.rs and proof.rs, for IntegerMathEqual only if B2 keeps it.
- Native compiler: optimization-unit/src/identity.rs:333-390 and 990; optimization-unit-semantics/src/unit_validation.rs:10791-10870.
- Interpreter: embedding/boundary_effects.rs.

Also update TASKS.md: delete the :306 bullet when Part A lands, and the :314 bullet if A1 absorbs it. Update proof_kernel_core_audit.md :47 and :586, the lowered-program README and encoding.md in the same PR.

## twin_programs

Positive (F_pos, build/diag-proposition/F_pos/main.omg; today WRONGLY refused):
```
data Point { x: i32; y: i32; }
machine same_point(a: Point, b: Point)
requires
    core\Identity::<i32>(a.x, b.x)
requires
    core\Identity::<i32>(a.y, b.y)
ensures
    core\Identity::<Point>(a, b)
{
}
```
It must compile, and the kernel must judge it (`MathematicalCoreDecision::Judged`) resting on the two entry premises. Today it is refused with "cannot prove ensures contract for exit from same_point at statement 0: core\\Identity::<Point>(a, b). (no proof found from the facts available here: no available equality proves the stated Identity)".

Negative twin (F_neg): identical except that the `requires core\Identity::<i32>(a.y, b.y)` line is removed. Today it gets the same refusal text as the positive, so the twins do not separate. After the repair it must be refused with UnprovedCause::NoDerivation and the text "cannot prove ensures contract for exit from same_point at statement 0: core\\Identity::<Point>(a, b). (no proof found from the facts available here: no available equality proves the stated Identity)", plus a component note naming the field left unproved: `core\Identity::<i32>(a.y, b.y)`, which TASKS.md:306 requires. The implementer chooses that note's exact wording; the test pins it.

Regression pair that already separates and must keep separating (D_pos / D_neg, mathematical `Pair` of `Peano`):
- `ensures core\Identity::<Peano>(swap(p).left, p.right)` is accepted.
- `ensures core\Identity::<Peano>(swap(p).left, p.left)` is refused with "cannot prove ensures contract for `swap_left`: core\\Identity::<Peano>(swap(p).left, p.left). (no proof found from the facts available here: no available equality proves the stated Identity)".

## tests

Add:
- Stage 06 fixture pair `tests/fixtures/record_identity/pass/point_from_fields/main.omg` and `tests/fixtures/record_identity/fail/point_missing_y/main.omg` (the F twins).
- A test `src/tests/record_identity.rs`, using the harness from src/tests/record_law.rs (`frontier_with_core`, then `judge_compilation`). It asserts:
  - the positive's ensures obligation is `Discharged::Identity` with `MathematicalCoreDecision::Judged`, its `assumption_closure` covering the two premises, and its certificate re-verified by `proof_admission::verify_mathematical_certificate`;
  - the twin differs only by the y line (assert on the source text, as record_law.rs:132-136 does) and is refused with NoDerivation naming `y`.
- Codec round-trip tests in declaration-encoding/src/terms/proposition.rs for the portable `Identity` row (Structural/Input with a path, MathematicalInteger, FloatMeaning catalog, ByteContent). The old tags 12, 18 and 19 must now be rejected.
- A marker test showing that marker 293 is rejected.

Run:
- `mbx test -p` on the stage 06 crate with tests record_law, nat_semiring, generic_machine, semiring_identity, law_premise, clause_terms, mathematical_clauses and the float_meaning prover tests;
- the stage 05 crate's contract_clauses test;
- the psi declaration-encoding and wire crates;
- psi semantics proofs bounded (float_meaning, predicate_denotation);
- the native optimization-unit identity tests;
- Clippy on each touched crate.

Re-check D_pos/D_neg and C_pos with `omega --check`.

## risks

- Folding `Equal` and `IntegerMathEqual` into the table would contradict audit L2 and the arithmetic rule set (778 and about 180 uses) and would add a Vec allocation to the hottest proposition. The plan keeps them deliberately and removes only the duplicate route. If the owner wants those folded too, that is a needs-owner question.
- Making Identity portable must keep every source-declaration Application unpublishable; the owner decided on 2026-11-23 not to publish law certificates. A missing refusal would let an unchecked claim travel.
- Widening `product` to records that have storage must refuse descriptor and view fields and opaque type parameters (contracts.md:74-77). Otherwise identity of a value would wrongly prove identity of storage contents.
- Float fields are raw identity at the f32/f64 carrier and must never be bridged from IEEE `==` (contracts.md:99-100).
- Kernel cost goes up for every record goal. Measure conversion steps on record_law and int.omg (the budget is 65,536).
- Moving the tag-19 catalog-commitment checks onto the new row must keep the forged-catalog tests (psi/semantics/proofs/bounded/float_meaning/tests.rs:150 and 344) rejecting.
- The local binary predates HEAD and cannot read the current std (std/build.omg:37, `bind_native`). The runs above used std from 1f08b50ac2da, so re-run them on a fresh build.


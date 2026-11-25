# declarations-binders

## still_open

True

## size

large

## depends_on

['Step 2 (C, evidence in a field) depends on step 1 (B, erased fields in ordinary data).', 'Step 1 should first resolve or record the AccessCustody-schema check noted in wiki/drafts/designs/proof_kernel_core_audit.md:450.', 'Step 3b (D, open indexed template) uses the integer search and `LawProof::Integers` from audit Step 6, already on main.', "Step 3a must locate the stage 04 producer of a closed copy's static-argument identity; not determined in this diagnosis.", 'Step 4 (A) relies on the native inductive builder already on main (psi/semantics/proofs/mathematical/inductive.rs:911, num_params).', 'Step 5 (E) for representation_agreement additionally waits on audit open question 1 (core\\Level binder spelling, refused in stage 03); without levels it depends on nothing.']

## evidence

Every probe used the worker-6 binary (built from 1f08b50ac2da; no commit in 1f08b50..898d56a touches these readers): `target/debug/omega.exe --check [--verbose] --target windows_x86_64 build/diag-stage05-kernel/<probe>/main.omg < /dev/null > <probe>.log`. The probes and logs are in C:/Development/Omega/.agents/worktree/worker-6/build/diag-stage05-kernel/.

Controls:
- vector_ctl (the source_indexed_family/field program rooted at main.omg): exit 0.
- bound_8: exit 0.
- bound_9 is refused with "exact arithmetic in machine `bounded` state `entry` return value may overflow `u64`: the operands are not provably in range".
- capped_relevant (`data Capped [copy] requires value <= cap { value: u64; cap: u64; }`): exit 0. Its twin capped_relevant_fail (value: 11) is refused with "cannot prove requires clause 1 of a data type in machine `main` state `entry` local `capped`".
- computed_index_closed (`inner::<1 + 1>()` in main): exit 0. A closed operator in a non-generic caller already works.

A, a field of another type:
- list_nat_field (`data Stack { case Empty; case Push(item: Nat, rest: Stack); }`) and list_u64_field are refused with "not supported yet: machine data that contains itself with a field of another type while reading the mathematical declaration of `Stack`" (read/signature.rs:682-690).
- list_u64_field is also refused with "mathematical declaration over a type other than data that contains itself" for the scalar formal `item: u64` (signature.rs:2137-2143, `ty`).
- seq_field (a program citing core `length_append` over `Seq<u64>`) is refused earlier, in read/places.rs:419-430: "type applied, constrained or boundary data that contains itself while reading the signature of `cite_append_length`".
- The record path refuses in the same way: signature.rs:2059-2065, "record of mathematical data with a field of another type".
- The kernel is not what blocks this. plain_data.rs:12-16 keeps only per-case counts of recursive fields (`arities`) and builds every field as the data itself (plain_data.rs:76-87, :114-150). The native inductive builder already takes uniform parameters and checks the strict positivity of mixed fields (inductive.rs:911 `num_params`, :353-387).

B, an erased field:
- erased_scalar (`data Tagged [copy] { value: u64; ghost [erased]: u64; }`) is refused with "type data with an erased field" (read/places.rs:2039-2041 and :2045-2047, in `record_shape`).
- capped_erased (capped_relevant with `cap [erased]: u64`) is refused with "type data with an erased field while reading the signature of `room`".
- lower/places.rs:407 hard-codes `relevance: BindingRelevance::Relevant` on every StructuralFieldDeclaration. The Psi wire format already carries and validates erased field relevance (psi/artifact/wire/src/foundation_validation.rs:9411-9530).
- Control erased_runtime_use (`machine leak(cap [erased]: u64) -> u64 { cap }`) is refused with "not supported yet: expression runtime use of an erased binding". That is an Unsupported refusal, though the spec makes this a violation ("it cannot determine runtime data or control").

C, evidence in a field:
- erased_evidence (`data Measured<Length: u64, Total: u64> [copy] { total: u64; evidence [erased]: Vector<Length, Total>; }`) gets two refusals:
  - "type data with an erased field while reading the local binding of `main::entry::measured`" (places.rs:2046).
  - "type generic data while reading the mathematical declaration of `Measured`". The cause is read/proof_stratum.rs:92-97 (`mathematical_within`): it counts an erased field of mathematical type, so the whole record is classed as mathematical, with no layout.

D, a computed index:
- computed_index_open (`machine outer<Count: u64>() -> u64 requires Count <= 10 { inner::<Count + 1>() }`) is refused with "admitted source selection Operator remains unresolved as CheckedOperator in `outer$specialized$3$14097` at 3 + 1" (selection_questions.rs:1298-1304).
- computed_index_vector (`let next [erased]: Vector<Length + 1, Total + 7> = Vector::Cons {..}`) gets the same refusal in `open_total$specialized$0$0`, plus "type index of indexed data other than a name or an integer while reading the evidence of `open_total`" (read/families.rs:846-859, `family_arguments_within`, which reads only `FamilyTerm::{Index, Payload, Name, Literal}`).
- computed_index_static_type (`widen<Limit>(..) -> Index<Limit + 1>`, result stored in `Index<6>`) is refused with "typing local initializer type" (read/statements.rs:606-617): the closed copy keeps `5 + 1` in the type's identity, so it is not the canonical value 6.
- open_const_implementation (a sample, copied) is refused in typed trees with "callable expression argument has no represented canonical value" (representations/typed-trees/callables/callable_applications.rs:5676).
- Typed preparation's deferral "using a type written as an expression, which no pass evaluates yet", quoted in TASKS.md:311 and audit Step 6, did not appear in any probe, including with --verbose. For these spellings that part of the board text is stale. Index equality is still compared as written: families.rs:713, :1775, :1816, :1889.

E, a definition with binders:
- binder_def (`let same<A: core\Type<0>>(left: A, right: A): core\Strict<0> = core\Identity::<A>(left, right);` used in `ensures same::<Nat>(a, a)`) and binder_def_value (`let below<Limit: u64>(value: u64): core\Strict<0> = value < Limit;`) are refused with "contract open mathematical predicate declaration" (read/mathematics.rs:330-331) and "contract open mathematical premise application" (mathematics.rs:143-149).
- No `let` with binders exists in source/library. Of the 37 sample files with one, 18 are source_indexed_family (`retain`, served by families.rs `evidence_definition`) and 18 are representation_agreement. representation_agreement is refused first in stage 03 at its level binder: "`u` is spelled like a builtin type but is not one" (audit open question 1).

Library status:
- No erased field or `let` with binders appears in core/nat.omg, int.omg, rat.omg or cauchy.omg.
- Audit "Step 4 as built" and TASKS.md:309 record that 96 laws of those files are judged.
- The library customer of this item is therefore core/seq.omg (`Seq<T>` with `head: T`; `length_cons(x: u64, ..)`, `length_append`, `append_assoc`, `length_reverse`).
- samples/cli/proofs/structural_proofs is refused first for stale syntax ("type `Nat` is not Equatable", from `a == a`), not by this item.

## spec

A: wiki/spec/proofs/inductive_profile.md#native-inductive-declarations says "This applies equally to a plain type, a type with uniform parameters such as `Seq<T>`, a family indexed by values such as `Vector<Length, Total>`, and a mutual block." The same section says the recursor's branch gets "the induction hypotheses for its recursive children", so a non-recursive field gets no hypothesis. The "Declaration correspondence" section requires keeping the declaration's binders and parameters, so Seq is one family with a uniform parameter, not one copy per application.

B and C: wiki/spec/proofs/contracts.md#explicit-erased-bindings (`data Certified<T> { value: T; proof [erased]: Valid<T>; }`) says "The binding remains in checking and semantic identity but contributes no runtime field, address, read, or cleanup." The same page says "Structural field rows retain authored relevance and exact normalized type identity."

D: contracts.md#licensed-normalization says "Closed proof-static indices remain canonical values, obtained through ordinary admitted evaluation under their exact concrete operations." For the open case it says "Compatibility evidence establishes that values fit a receiving indexed type."

E: wiki/spec/proofs/mathematical_bindings.md#named-mathematical-definitions says "Generic arguments are resolved first under their own binder rules."

Also used: audit wiki/drafts/designs/proof_kernel_core_audit.md, "Step 2/3/4/6 as built" and plan row 4; TASKS.md:309 and :311. OWNER_QUESTIONS.md has no entry on any of these. A level binder (`core\Level`) is audit open question 1, so representation_agreement stays out of scope.

## cause

One cause per sub-item:

A. Stage 05 has no kernel type for anything other than the data itself and records of such data. Where each refusal sits:
- places.rs:419-430 refuses any applied or parameterized self-containing data before it reserves a record.
- signature.rs:682-690 (`Converter::data`) requires every case field to be `ValueType::record(record)`. The cause is that `PlainData` (psi/semantics/proofs/mathematical/plain_data.rs:12-16) encodes a case as a count of recursive fields only.
- signature.rs:2137-2146 (`ty`) and :2059-2065 (`product`) accept only records of self-containing data, so a scalar field or formal and a type parameter have no kernel type.

B. `record_shape` (places.rs:2026-2047) refuses every `BindingRelevance::Erased` member rather than giving it a semantic row with no storage:
- The lowered record has no field form for an erased field: lower/places.rs:407 always writes `Relevant`.
- There is no reader for an erased initializer in a record literal, nor for an erased projection in an erased position.
- proof_stratum.rs:92-97 treats an erased field of mathematical type as layout-carrying, which pushes the record into the signature.

C. The evidence reader (families.rs:1696-1775 for passed evidence, :1799-1896 for locals) accepts evidence only as an erased local or formal named directly. It cannot see a field projection whose declared type, under the record's application substitution, is indexed data.

D. Two stage-05 gaps share one root: a written static value expression is never read as an expression.
- The closed copy reads the folded binding. `applies_nothing` (read/calls.rs:142-148) calls `static_argument_selections` (read/calls/callback_bindings.rs:55-89), which visits only machine and member selections, never a value argument's `TypeReferenceNode::ConstExpression`. The `+` occurrence therefore never reaches `complete_operator` (read/source_operators.rs:410-551), and `selection_questions.rs:1298` reports it unanswered.
- The same unevaluated `5 + 1` stays in a closed type's identity (statements.rs:606-617).
- In the open template, `family_arguments_within` (families.rs:846-859) has no integer-expression index term. Index agreement is structural equality of `FamilyTerm`, not a proved integer equality (families.rs:713, 1775, 1816, 1889).

E. `mathematical_predicate_definition` (mathematics.rs:330) and `applied_definition` (mathematics.rs:143) refuse `definition.binders` outright. They do not bind binders as leading inputs from `call.machine_arguments`, which they already do for ordinary and erased-type parameters (mathematics.rs:190-259, `bind_type`).

## code_locations

['source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/places.rs:419-430 (applied/parameterized self-containing data refused)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/places.rs:2026-2047 (record_shape: data with an erased field)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/lower/places.rs:407 (field relevance hard-coded Relevant)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/proof_stratum.rs:92-97 (an erased field makes the record mathematical)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/signature.rs:682-690 (data that contains itself with a field of another type)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/signature.rs:2059-2065 (record of mathematical data with a field of another type)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/signature.rs:2137-2146 (ty: only self-containing data)', 'source-rust/subsystems/compiler/psi/semantics/proofs/mathematical/plain_data.rs:12-170 (PlainData keeps only recursive-field counts)', 'source-rust/subsystems/compiler/psi/semantics/proofs/mathematical/inductive.rs:911-928 (declare_inductive_family already takes num_params)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/families.rs:846-859 (index other than a name or an integer)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/families.rs:689-713,1696-1775,1799-1896 (evidence sources; index comparisons as written)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/calls.rs:142-148 and read/calls/callback_bindings.rs:55-89 (static arguments visited only for machine selections)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/source_operators.rs:398-551 (complete_builtin_operator / complete_operator)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/selection_questions.rs:1257-1305 (Operator remains unresolved as CheckedOperator)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/statements.rs:606-617 (local initializer type: Index<5+1> vs Index<6>)', 'source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/callables/callable_applications.rs:5672-5676 (callable expression argument has no represented canonical value)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/mathematics.rs:143-149,330-331 (open mathematical premise application / predicate declaration)', 'source/library/core/seq.omg:6-9,77-150 (customer: Seq<T>, length_cons, length_append)', 'C:/Development/Omega/.agents/worktree/worker-6/build/diag-stage05-kernel/ (probe programs and logs)']

## repair_plan

Five pull requests, in dependency order. Each one is one general mechanism with its own twin pair.

1. (B) Serve erased fields in ordinary data. Lane psi; prerequisite of 2.
- Remove the two Erased refusals in `record_shape` (places.rs:2039-2047). In `declared_members`, give an erased field a semantic field row (its identity and normalized type) but no storage segment: no `ExactPlaceId` child, no write and no claim.
- In lower/places.rs:407, write `relevance: Erased` for it. The wire validator already checks erased field rows.
- In proof_stratum.rs:92-97, skip erased fields: an erased field of mathematical data does not make its record mathematical. Layout skips erased fields.
- A record literal's erased initializer is read through the existing erased-term reader (`erased_term`, statements.rs:847-860, the reader for erased locals). It becomes the field's term and is never an operand.
- A clause (data `requires`, machine contracts) reads the erased field as that term through `clause_term`, as it reads an erased formal.
- A runtime read of an erased field is refused at the projection with the existing erased-binding refusal. Do not add a new path.
- Before landing, check the audit's note (proof_kernel_core_audit.md:450): an `AccessCustody` conformance is not checked against its schema, and "the check has to exist before erased fields are served". The only declaration is the empty `public trait AccessCustody<C, P, T>` at source/library/core/layout.omg:514. Either confirm that no conformance can now reach an erased field, or record the gap in OWNER_CODE_CONCERNS.md in the same PR.

2. (C) Evidence in a field of other data. Depends on 1.
- In families.rs, extend the evidence sources at :1696-1775 (call arguments) and :1838-1896 (local initializers) with one more source: a field projection `place.field` whose field type is indexed data. Compute its index by substituting the record application's arguments (`Measured<Length, Total>`) into the field's declared `Vector<Length, Total>`. Reuse `family_arguments_within` and `applied_at_call`; do not add a second index reader.
- A record literal whose erased field holds evidence checks that evidence at the field's substituted index, by the same path an erased local initializer takes.

3. (D) Computed indices and static arguments. Two pieces share one mechanism: a written static value expression is read as an ordinary scalar expression wherever a static application or an index is read.
- (3a) Closed copies. In `static_argument_selections` (callback_bindings.rs:55):
  - When a value argument's `type_reference` is `TypeReferenceNode::ConstExpression(e)`, read `e` with the ordinary scalar reader. Its operators then reach `complete_builtin_operator` and record completion, and each partial operator owes its OperationSafety (`Count + 1` in u64) at the call. The open copy proves it from the caller's requires; the closed copy proves it by constant folding.
  - Make the folded value the argument's canonical identity, so `Index<5 + 1>` and `Index<6>` are one type (statements.rs:606-617). The canonicalization belongs where stage 04 folds the static application (the constant recipe in `constant_recipe_for_const_application`). Before writing anything, find the producer of the closed copy's type identity in stage 04 specialization; it is not determined here.
  - Find the cause of "callable expression argument has no represented canonical value" (callable_applications.rs:5676) for open_const_implementation. It is the same missing canonical value for a `Base + 4` argument.
- (3b) The open indexed template.
  - Replace `FamilyTerm::{Index, Payload, Name, Literal}` with an integer term read by `clause_term(.., ClauseUse::MathematicalInteger)` over the machine's binders, in families.rs:846-859 and at the theorem citation index (families.rs:689-713, which is the next refusal once this one is fixed).
  - Replace each structural index comparison (families.rs:713, 1775, 1816, 1889) with an owed integer equality law, `LawProof::Integers`, under the machine's value parameters and requires. This is the same route a case literal's requirement takes (`evidence_laws`, signature.rs:1697). Stage 06 proves it with the existing integer search.
  - A refused equality reports the two indices.

4. (A) A field of another type in data that contains itself. Independent of 1-3; it is the library customer (core/seq.omg). Three slices of one mechanism:
- (4a) Generalize `PlainData` (plain_data.rs) to per-case field kinds, `Recursive` or `Other(TermHandle)`, plus a uniform parameter count:
  - Constructor types Π over every field in order (plain_data.rs:76-87).
  - `case_binders`, `field`, `hypothesis`, `branch_type` and `case_value` (plain_data.rs:94-170) give an induction hypothesis only for recursive fields.
  - Pass `num_params` to `declare_inductive_family`.
  - Update every consumer of the arity-only API: signature.rs (11 sites), families.rs (3), stage 06 laws.rs (5), equality.rs (3), bounded_denotation.rs (1).
- (4b) `Converter::ty` (signature.rs:2137) becomes the single type reader for fields and formals:
  - The data itself gives a recursive field.
  - Other mathematical data or a record gives its entry constant.
  - A fixed-width integer gives one on-demand definition, `Σ (n : Int). lower <= n ∧ n < upper`, over the integer vocabulary (`integer_vocabulary`, signature.rs:1352). The carrier range then comes from the second projection rather than an assumption.
  - A type parameter gives the family's parameter variable.
  - Remove the refusals at signature.rs:682-690, :2059-2065 and :2138-2143.
- (4c) In places.rs:419-430, accept a self-containing declaration with uniform type parameters:
  - Reserve one record per declaration symbol, keyed by declaration, not application, and record each application's argument types.
  - `ty(Seq<u64>)` is `Apply(Seq, carrier(u64))`.
  - Keep refusing a declaration-level `requires`, a cleanup or boundary opaque supply, and value parameters on a non-indexed self-containing declaration.

5. (E) A definition with binders. Low priority, since no library program needs it.
- In `applied_definition` (mathematics.rs:143-149), bind each binder before the ordinary parameters from `call.machine_arguments`: a value binder through `bound.push` with the static argument's scalar term, a type binder through `actual_inputs.bind_type`.
- In `mathematical_predicate_definition` (mathematics.rs:330), add the binders to `MathematicalInputs::new` ahead of the parameters, with the same consistency check on the retained definition.
- Definitions that hand on indexed evidence keep their families.rs route (`evidence_definition`).
- A `core\Level` binder stays refused until audit open question 1 is answered.

After each PR, delete the board text it settles from TASKS.md:309 and :311, and correct :311's stale "typed preparation defers the open machine" sentence, which is not reproduced at 1f08b50.

## twin_programs

Each positive must compile, and those with a main must give Psi exit 0. Each negative differs from its positive in one input. The probe sources are under C:/Development/Omega/.agents/worktree/worker-6/build/diag-stage05-kernel/.

B, erased field (capped_erased):
- Positive: `data Capped [copy] requires value <= cap { value: u64; cap [erased]: u64; }`, `machine room(capped: Capped) -> u64 requires capped.cap == 10 { 10 - capped.value }`, and main builds `Capped { value: 3, cap: 10 }` and checks `room(capped) == 7`. Expected Psi exit 0; it is refused today with "type data with an erased field".
- Negative: `value: 11`. It must be refused with "cannot prove requires clause 1 of a data type in machine `main` state `entry` local `capped`". The relevant-field control capped_relevant_fail gives this exact text today.
- Misuse twin (optional): `room` returns `capped.cap`. It must be refused at the projection with the existing "runtime use of an erased binding".

C, evidence in a field (erased_evidence):
- Positive: Vector and vector_bound as in source_indexed_family/field, plus `data Measured<Length: u64, Total: u64> [copy] { total: u64; evidence [erased]: Vector<Length, Total>; }` and `machine remaining<Length: u64, Total: u64>(measured: Measured<Length, Total>) -> u64 requires Length <= 8 && measured.total == Total { vector_bound::<Length, Total>(measured.evidence); 80u64 - measured.total }`. main builds `Measured { total: 0, evidence: nil }` at <0, 0> and checks 80. Expected Psi exit 0.
- Negative: `requires Length <= 9`. It must be refused with "exact arithmetic in machine `remaining` state `entry` return value may overflow `u64`: the operands are not provably in range". The bound_9 control gives this text for `bounded` today.

D, computed index (computed_index_vector):
- Positive: `machine open_total<Length: u64, Total: u64>(tail [erased]: Vector<Length, Total>) -> u64 requires Length <= 7 { vector_bound::<Length, Total>(tail); let next [erased]: Vector<Length + 1, Total + 7> = Vector::Cons { previous: Length, previous_total: Total, head: 7, tail: tail }; vector_bound::<Length + 1, Total + 7>(next); 80u64 - (Total + 7) }`, called as `open_total::<0, 0>(nil)`, with main checking 73. Expected Psi exit 0.
- Negative: `requires Length <= 8`. It must be refused with "exact arithmetic in machine `open_total` state `entry` return value may overflow `u64`: the operands are not provably in range".
- Closed-copy pair (3a): computed_index_open (`outer<Count> requires Count <= 10 { inner::<Count + 1>() }`, main checks `outer::<3>() == 4`) is expected to reach Psi exit 0. Its twin `requires Count <= 18446744073709551615` must be refused at `Count + 1` with the u64 overflow refusal ("exact arithmetic in machine `outer` ... may overflow `u64`").

A, a field of another type (list_nat_field):
- Positive: `data Stack { case Empty; case Push(item: Nat, rest: Stack); }`, `depth` by cases, and `machine depth_push(item: Nat, rest: Stack) ensures core\Identity::<Nat>(depth(Stack::Push { item: item, rest: rest }), Nat::Succ { prev: depth(rest) }) { }`. It must be judged by the kernel.
- Negative: the ensures right side is `Nat::Succ { prev: Nat::Succ { prev: depth(rest) } }`. It must be an UnprovedLaw whose statement begins "cannot prove ensures contract for `depth_push`: core\Identity::<Nat>(depth(" (the form record_law.rs:141-145 pins).
- Repeat the pair with `item: u64` (list_u64_field) for the scalar carrier.
- Library pair (seq_field): `cite_append_length(s: Seq<u64>, t: Seq<u64>) requires core\Identity::<Nat>(length(s), Nat::Zero) ensures core\Identity::<Nat>(length(append(s, t)), add(Nat::Zero, length(t))) { length_append(s, t); }` must be judged. Its twin without the `length_append(s, t);` citation must be refused with "cannot prove ensures contract for `cite_append_length`: core\Identity::<Nat>(length(append(s, t)), ...".

E, a definition with binders (binder_def_value):
- Positive: `let below<Limit: u64>(value: u64): core\Strict<0> = value < Limit;`, `machine check(value: u64) -> u64 requires below::<10>(value) ensures result < 10 { value }`, and main calls `check(3)`. Expected Psi exit 0.
- Negative: `ensures result < 9`. It must be refused with "cannot prove ensures contract for exit from `check`: ..." (describe.rs:121-126 form).

## tests

Rules for every step: test the change and the small area around it, lint each touched crate with `mbx clippy -p <crate> --all-targets -- -D warnings`, and run `mbx xtask check-rules`. Do not run sweeps.

- Step 1 (B):
  - Extend the record_shape unit test in places.rs:2350-2417 so an erased field yields Ok, with a semantic row and no storage segment.
  - Add a stage 05 test in tests/field_reads.rs for the erased projection refusal at runtime.
  - Put the capped pair under the stage 06 crate's tests/fixtures/erased_field/{pass,fail}, check both engines' captured stdout, stderr and exit explicitly (BUILD-TEST-COMPARISON is not landed), and `mbx test -p` the stage 05 and stage 06 crates.
- Step 2 (C): put the Measured pair under the stage 06 crate's tests/fixtures/indexed_family/{pass/evidence_in_a_field,fail/evidence_in_a_field_unbounded}, extend src/tests/indexed_family.rs, and run the existing source_indexed_family/field and source_indexed_type_parameter/field samples with `mbx xtask sample-gate --filter`.
- Step 3 (D):
  - 3a: the outer/inner pair as a stage 05 test beside tests/operator_selection.rs, plus `mbx xtask sample-gate --filter open_const_implementation` and `--filter computed_index`.
  - 3b: the open_total pair in the stage 06 crate's tests/fixtures/indexed_family, asserting an integer law for index equality.
  - Before landing, rerun the three `source_indexed_*` field samples, which must stay unchanged.
- Step 4 (A):
  - Unit tests in plain_data.rs: a case with a non-recursive field gets no hypothesis; a uniform parameter declares through `check_signature`; negative recursion is still refused.
  - Stage 06 src/tests/ gains `seq_law.rs` with the Stack pair (Nat and u64), and `every_law_of_seq_is_judged` compiling nat.omg plus seq.omg declarations as record_law.rs:149 does for int.omg. Limit it to the monomorphic laws (length_empty, length_cons, append_empty_right, append_assoc, length_append, length_reverse); map and filter are generic mathematical machines, which are not yet served.
  - Rerun nat_semiring.rs and record_law.rs to confirm the 96 existing laws are unchanged, and report the change in kernel conversion steps.
- Step 5 (E): the below pair as `transparent_definition/{pass,fail}` binder cases beside stage 06's address_bounds.rs, plus a mathematics.rs unit test that a `core\Level` binder is still refused.

## risks

- Step 4 is the costly one. Changing the PlainData API touches stage 06's law prover (laws.rs, equality.rs). The audit records that kernel conversion cost per law is near the 65,536 budget (nat_gap_triangle 59,947 before the repair). Non-recursive Σ-carrier fields add a projection per computation, so measure on nat.omg before taking seq.omg.
- Keying a self-containing record by declaration instead of application changes `record_sources` and `self_containing` lookups, which `clause_terms.rs:1406` and `rankings.rs:158` also read.
- Step 1 makes erased fields reachable for the first time. The audit names the missing AccessCustody-schema check as a precondition (proof_kernel_core_audit.md:450).
- The erased runtime-use refusal is currently Unsupported ("not supported yet: ... runtime use of an erased binding"), but the spec makes it a violation. The step 1 PR must not lean on it as a verdict, and should record it in OWNER_CODE_CONCERNS.md.
- Step 3a crosses the stage 04 to 05 boundary: the folded value's canonical identity must be produced by its stage owner (stage 04), not by stage 05 re-reading source text (the source_text_reads ratchet).
- Not determined in this diagnosis:
  - whether the open template `outer<Count>` is checked at all once the closed copy passes;
  - the stage 04 site that writes `Index<5 + 1>` into the closed copy's result type;
  - whether `length_reverse` in seq.omg needs `add_succ_law` and `add_zero_right`, which are private in nat.omg and would have to be read through the conformance closure.
- The binary is from 1f08b50ac2da plus one Build change, not HEAD 898d56a. None of the 15 commits in between touch these readers (checked with `git log --oneline 1f08b50ac2da..HEAD`).


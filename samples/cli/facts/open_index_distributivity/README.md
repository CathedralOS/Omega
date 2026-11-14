# Open index distributivity

A generic body takes a value indexed `A * (B + C)` as `A * B + A * C`
using the selected algebra's checked distributivity law. The ratified
[identity/compatibility split](../../../../wiki/spec/proofs/contracts.md#licensed-normalization)
keeps those open canonical keys distinct and carries a compatibility proof.

Owning task: **OPERATOR-MACHINE-SUPPLY** in [TASKS.md](../../../../TASKS.md).
The implementation, control classifications and execution records below describe
the earlier law-sensitive canonical normalizer at their named source versions.
They are historical evidence, not validation of the 2026-11-06 design.
In particular, mutual distributivity is no longer grounds by itself to refuse
a valid compatibility proof. Literal connections are also moving from user-law
identity rewriting into proof admission. See [What remains open](#what-remains-open).

Every place copies that sentence and the same operation. The trait `IndexSemiring` declares `add` (`+`), `multiply` (`*`) and the law `multiply(a, add(b, c)) == add(multiply(a, b), multiply(a, c))`. The conformance `Canonical` selects both operations as constant three and proves the law through a satisfier that requires it. The generic body is

```omega
machine rearrange<const A: u64, const B: u64, const C: u64, Algebra satisfies IndexSemiring>(raw: i64) -> i64 {
    let source: i64 in Indexed<A * (B + C)> = raw as i64 in Indexed<A * (B + C)>;
    let moved: i64 in Indexed<A * B + A * C> = source;
    moved as i64
}
```

and each place calls `rearrange<2, 3, 4, Canonical>` with `41` and reaches `done()` only through `moved == 41`. The reject twin deletes the one law slot (its declaration in the trait, its satisfier and its conformance member) and nothing else. Only the transport of the raw value or of the index operands changes between places. Callers carry the raw `i64`: a data field cannot hold a computed open index (its owner is not a machine), and a closed caller cannot name the instantiated `Indexed<2 * (3 + 4)>`, which stays an operation of the selected algebra over literal leaves.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | the value is a local binding |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | the value is a field of a `[copy]` record, read as `held.raw` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | `observe` receives the value and performs the call |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | the value is a call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | a `match` yields `41` or `58`; only the taken arm reaches `moved == 41` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | the three index operands are named constants (`FACTOR`, `LEFT`, `RIGHT`) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | a successor machine `check` receives the value and performs the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | `relay<A, B, C, Algebra>` forwards its own binders to `rearrange`, so the specialization chain keeps the indices open one instance deeper |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | Build selects `X86Observe` or `ArmObserve` per hosted target; each performs the call after checking its own tag |

The row has no root `main.omg`; `local` is its canonical witness. One more production of the same layer, with the same sentence, composes the record with the other laws, and six controls bound what may license the rewrite:

| Production or control | Program | What it shows |
| --- | --- | --- |
| commutative semiring | [`commutative_semiring/main.omg`](commutative_semiring/main.omg), twin [`invalid/main.omg`](commutative_semiring/invalid/main.omg) | the trait also declares commutativity and associativity of both operations; `(A + B) * (C + D)` and `D * B + C * A + A * D + B * C` are one polynomial. The twin deletes only `multiply_comm`: a sum on the left of a product needs it, and the diagnostic lists `multiply` as licensing associativity and distributivity only |
| conditional law | [`controls/conditional_law`](controls/conditional_law/main.omg) | the law slot carries `requires a < 100`; the conformance is lawful, but an index rewrite has no premise to discharge, so the law licenses nothing |
| repeated operand law | [`controls/repeated_operand_law`](controls/repeated_operand_law/main.omg) | `multiply(a, add(a, a)) == add(multiply(a, a), multiply(a, a))` is a special case, not distributivity |
| foreign addition | [`controls/foreign_addition`](controls/foreign_addition/main.omg) | the selected `+` is `IndexSum::add`, the product's trait names a requirement `add` too; the shared trait and provider are the evidence, not the name |
| mutual distributivity | [`controls/mutual_distributivity`](controls/mutual_distributivity/main.omg) | the trait declares both directions, as a lattice does; neither expansion is canonical, so neither applies |
| missing law member | [`controls/missing_law_member`](controls/missing_law_member/main.omg) | the trait declares the law and the conformance supplies no member; the conformance is incomplete |
| unproved law | [`controls/unproved_law`](controls/unproved_law/main.omg) | the selected multiplication returns its first operand, so the law is false and its satisfier is rejected |

## Defect and repair

At base `05a6f6af37e`, with an optimized exact-source producer (`omega --build-profile`: `opt_level: 1`), the generic body above was rejected:

```
error: index compatibility condition `index-equality:Indexed:rearrange::entry:statement-1:Indexed<A * B + C>==Indexed<A * B + A * C>` is not established: actual `Indexed<A * B + C>` and expected `Indexed<A * B + A * C>` are distinct normalized instances; closed evaluation, licensed normalization, or an exact local equality fact is required
```

(the same body spelled as a free machine `distribute_indexed(value: i64 in Indexed<A * (B + C)>) -> i64 in Indexed<A * B + A * C>` gives that message with `distribute_indexed::entry:exit-0`). The label misprints `A * (B + C)` as `A * B + C`: the shared expression display writes no parentheses. The trait's law was recognized by the proof side (`licenses.rs`, `distributivity_shape`) and ignored by the index side: `OpenIndexOperationSelection` recorded two booleans, commutativity and associativity, so `normalize_open_index_expression_operations` selected the operations and `open_index_identity.rs` flattened and sorted but never expanded. The consumer had no list to read.

The repair records the missing requirement where the layer emits it and reads it where the identity is built:

- `index_authority.rs`: `OpenIndexOperationSelection::distributes_over` lists the additive requirements the selected requirement distributes over. Specialization clones the selection and `apply_provider` replaces only the provider, so the list reaches every instance `finalize_domain_instance_identities` renames.
- `licenses.rs`: `declared_index_algebra_laws` returns the additive requirement of each left-distributivity law slot of the requirement, skips any law slot that states a `requires` premise, and `distributivity_shape` now requires distinct operands, as the commutativity and associativity shapes do.
- `open_index_identity.rs`: normalization builds terms and applies each law the selection records. A product expands over a sum operand when both selections name one provider and one trait, the product lists the sum's requirement, and the sum does not distribute back. Commutativity lets a left sum move right and associativity groups the other factors; the new sum flattens and sorts under the addition's own laws; an expansion above 64 summands keeps the unexpanded form. An expression no new law touches keeps its identity text.
- `index_compatibility`: the rejection appends the laws each selected operation records (`` `multiply` of `IndexSemiring` licenses no law ``), so a missing law reads as one, and renders an index expression with the parentheses its structure needs.

The specialized instance is checked on its own: the twin's second diagnostic names `rearrange$specialized$…` over `Indexed<2 * (3 + 4)>` and `Indexed<2 * 3 + 2 * 4>`, so the evidence the law needs is read from the copied selection, not from the template.

## Execution

Every positive binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/open_index_distributivity/<place>/main.omg < /dev/null
omega --check samples/cli/facts/open_index_distributivity/<place>/invalid/main.omg < /dev/null
python tools/sample_native_gate.py --cli <omega> --facts --filter cli/facts/open_index_distributivity
python tools/corpus_gate.py --both --filter open_index_distributivity
```

Measured on Windows x86-64 with an optimized producer built from `af2cbd5e683` (`opt_level: 1`), base `6e9573fc181` plus this change; the base observations below are from the producer at `05a6f6af37e`, where the defect was first recorded:

- The nine places and the commutative-semiring production report `native exit 0` and `interp exit: 0` through `omega run --both` (20 of 20 engine runs exit 0).
- The ten twins and the six controls exit at compile time with the diagnostics above. The place twins and the production twin fail at the template and again at its specialized instance; the controls `conditional_law`, `repeated_operand_law`, `foreign_addition` and `mutual_distributivity` fail the same way, and `missing_law_member` and `unproved_law` fail at the conformance.
- The unmodified base producer rejects all nine place positives and the production positive with the compatibility error above (10 of 10 `--check` runs exit 1), so none of them meets its oracle at base.

### The oracle discriminates

`main()` reaches `done()` only through `moved == 41` after the value has crossed the specialized `rearrange` body twice (the qualifying cast and the `as i64` back). A specialization that dropped the body or returned another value takes `bad()` (`crash Trap`). In `match_join` the arms yield `41` and `58`, so taking the wildcard arm would trap. In `target_body` each provider checks its own tag (`17` or `29`) before the call, so a wrong selected body traps. The indices are proof-level data and leave no runtime trace: the construct is a static identity, and this oracle covers the instance that only exists after specialization, not the identity itself. The twins carry the static counterfactual: the same body without the law is rejected.

## Reject twins

Each twin's `sample.json` carries `"role": "intended-invalid"` and `rejection_contains`. The nine place twins pin `Indexed<A * (B + C)>==Indexed<A * B + A * C>` and `` `multiply` of `IndexSemiring` licenses no law ``. The commutative-semiring twin pins the two forms and `` `multiply` of `IndexSemiring` licenses associativity, distributivity over `add` ``. `foreign_addition` pins `` `multiply` of `IndexProduct` licenses distributivity over `add` `` with `` `add` of `IndexSum` licenses no law ``. `mutual_distributivity` pins `distributivity over` in both directions. `missing_law_member` and `unproved_law` pin the conformance diagnostics, ``closed conformance `Canonical satisfies IndexSemiring` is incomplete`` and ``which the requirement does not imply``; those two reject at base as well, because they exercise the existing conformance checks the license depends on, not this repair.

## Pinned fixtures

The row is pinned by 26 fixtures: `tests/omega/pass/facts/open_index_distributivity_through_<place>_exit` and `tests/omega/fail/facts/open_index_distributivity_through_<place>_rejected` for the nine places, `open_index_distributivity_commutative_semiring_{exit,rejected}`, and `open_index_distributivity_{conditional_law,repeated_operand_law,foreign_addition,mutual_distributivity,missing_law_member,unproved_law}_rejected`. Each pass fixture is a byte-identical copy of its positive with the application renamed to the fixture, plus `expected_native_exit.txt` and `expected_interpreter_exit.txt` of `0`. Each fail fixture is a byte-identical copy of its twin or control with an `expected.txt` carrying its fragments.

## Requirements and Evidence

These points describe the historical implementation and observations above.

1. **Establishment:** the generic body establishes `Indexed<A * B + A * C>` from `Indexed<A * (B + C)>` through the law its trait declares and its conformance proves. The conformance member calls a satisfier whose premise is the law itself, which is provable only because the selected operations satisfy it (`unproved_law`), and a conformance that omits the member is incomplete (`missing_law_member`). No caller guard or equality fact establishes the identity.
2. **Identity and lifetime:** the evidence is the pair of selections' provider, trait and requirement. A same-named requirement of another trait licenses nothing (`foreign_addition`). The specialized instance re-checks the identity over literal leaves with the selection copied from the template.
3. **Composition:** the record composes with commutativity and associativity (`commutative_semiring`; an unpinned probe with commutativity and distributivity but no associativity also passed), flattens nested sums, and expands nested sums. Nine transports carry the call, including a forwarding generic (`other_machine`).
4. **Soundness boundaries:** a conditional law, a repeated-operand law, a mutual declaration, a foreign addition, an unproved law and a missing member each license nothing. A search limit (64 summands) keeps the unexpanded form instead of claiming inequality.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the index and its algebra are erased; the Terminal Psi interpreter executes the specialized instance and agrees with native execution in all ten positives.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** every positive binds `ProgramEntry` for the four hosted targets; `target_body` selects distinct providers per target through Build rows. The value passes as an `i64` register argument and result.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** the executed images hold one scalar across the specialized call; nothing index-specific is emitted.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** the emitted PE images execute and agree with Terminal Psi; the twins and controls reject before emission.

## What remains open

- **Stable keys and proved compatibility.** Implement the [ratified migration](../../../../wiki/drafts/designs/index_identity.md): fixed core-operation canonical rules, no distributive or user-law identity rewriting, and automatic checked algebraic transport. Preserve the original source/value/custody oracles across the nine places. The earlier `normalizer_version` 3 results above do not validate this policy.
- **Policy and evidence.** Implement root `builder.proof.index_expansion_terms`, default 64, with recorded sponsorship/usage. Exercise 64/65 terms, larger allowances, theorem-assisted equality, separate canonical keys and exact matching. Prove associative noncommutative chains and computed substitutions through compatibility rather than forcing a shared expanded key. Carry generic and published derivations without re-searching under smaller consumer allowances.
- **Control migration.** Audit the old mutual-distributivity rejection under the new rule: a valid forward law remains usable when the reverse is present. Reclassify only after actual verification; retain false/missing/foreign-law and unproved-premise negatives. The historical pins above are not an instruction to keep the retired no-expansion exception.
- **Specialization and closed keys.** Transport exact checked evidence while retaining distinct symbolic specialization keys; fully closed equal indices still evaluate to a common value. Update artifact/profile provenance coherently. This documentation changes no program, diagnostic pin or recorded outcome.
- **Computed index arguments.** A binder substituted by a computed expression contributes the expression's identity text as one leaf, so a sum arriving as a call argument is not expanded further. No fixture here passes one.
- **Hosted targets.** Native execution was observed on `windows_x86_64` locally. `linux_x86_64`, `macos_arm64` and `linux_arm64` are bound in every `build.omg` and realized by binding; hosted evidence is recorded in the landing pull request.

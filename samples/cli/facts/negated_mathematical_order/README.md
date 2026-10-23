# Named negation establishes an executing integer bound

Owner: **PROOF-CONTRACT-MIGRATION**. A named negated integer-order predicate
establishes `value <= 5` for exact `5u64 - value`. Every place uses
`within(limit, value) = !(value > limit)` and the route-free `Small` domain;
the consumer returns `2`. Every reject twin changes only the establishing
input from `3` to `6` (the named constant in `const_name`).

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The same authored sources and oracles are pinned under
`tests/omega/{pass,fail}/facts/negated_mathematical_order_through_*`.
All positives declare the four hosted entries. Actual execution evidence is
Linux x86-64; no foreign native host is claimed.

## Requirements and evidence

- Establishment: checked mathematical application preserves logical negation
  and the selected integer comparison. Initializers and writes must establish
  the exact declared domain; naming a predicate does not prove it.
- Identity and lifetime: captures bind the exact subject and limit. Transport
  does not invent a relation about another value or a later storage version.
- Composition: qualified values carry the bound through all nine places into
  subtraction. Both match arms execute. Result equality guards check only the
  runtime oracle; they never establish the bound consumed by subtraction.
- Soundness: the input-6 twins reject. The local twin fails Exact subtraction;
  the others fail the establishing domain write. Focused controls retain false
  mathematical calls, changed operands, IEEE NaN-sensitive order and selected
  custom comparisons. Existing kernel closure and malformed-equation controls
  remain separate coverage; no calculus rule or PCC replay is added.
- Psi 05–07: checking retains the complete predicate/requirement roster before
  temporary mathematical preparation expires. Lowering consumes that result,
  erases proof-only mathematics and preserves the scalar operation in Terminal.
- Omega 00–02: ordinary hosted entry selection, scalar/field layout and
  call/state argument plans transport the same qualified value.
- Omega 03–06: the common subtraction, loads, calls, joins and state transfers
  execute with the authored scalar carriers.
- Omega 07–09 and emission: native ELF and Terminal interpretation must agree
  on exit zero through `omega run --both` for every positive above.

## Baseline and remaining scope

At `67191f681c81cfcc9902218365fc2df902b95ab1`, a direct named `value <= limit`
predicate executes, but its equivalent named negation and double negation
refuse the consuming subtraction. The frozen row also refuses source admission.
The source, diagnostics and engine observations are retained in the existing
qualification evidence under `build/proof-boolean-sol`. The repaired row has
nine native/Terminal exit-zero positives and nine intended rejections. Direct
and double-negated predicate contract regressions also execute to exit zero.

The neighboring `proofs/mathematical_predicate_contract_import_exit` fixture
refuses `helpers::byte_mask() == helpers::byte_mask()` on both the preserved
baseline and candidate under canonical `--target macos_arm64`; that pre-existing
refusal is attributed in `existing-import-attribution.json`, not re-recorded.

Boolean equality around an order, general mathematical operands and non-integer
ordering remain outside this bounded repair. Opaque/custom/IEEE operations
receive no integer law from negation. Quotient lifting, issuer routing, indexed
families, progress subjects and the broader proof-migration acceptance remain open.

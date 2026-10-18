# float_range_delivery

The same-format inclusive f64 fact `0.0 <= value && value <= 0.25`
proves the wider requirement `0.0 <= value && value <= 0.5` at `admit(value)`.
No conversion is inserted. The original root bracket-syntax check probe is
preserved as historical coverage; the nine current witnesses use canonical
`where`, `requires`, `ensures`, and established flow facts.

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call_result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match_join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const_name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| other_state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| other_machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| target_body | `target_body/main.omg` | `target_body/invalid/main.omg` |

Every positive has an authored native exit oracle of zero and exercises the
inclusive upper endpoint 0.25. The match also exercises its other arm at 0.125.
Every twin removes only the narrow upper premise and must reject at the same
`admit` operation; target_body checks all four authored target bodies.
The const-name witness names the scalar endpoints: floats and records containing
floats are not canonical generic atoms.

The compiler transports proved float observations through ordinary value facts,
entry contracts, completed data construction, and covered match-result joins.
It retains NaN behavior, format identity, write invalidation, and exact subject
identity. Regression controls include out-of-range construction, omitted-zero
construction, f32 rounding, unknown NaN, mutable storage, copied scalar snapshots,
asymmetric joins, an unknown arm, write-only storage, and gated native entry.

Canonical authority: `wiki/spec/language/numeric_values.md` (bounds as
propositions, same-format delivery, IEEE comparison), `dependent_values.md`
(construction and exact contract substitution), `evaluation.md` (canonical atom
eligibility), `expressions.md` (once-only evaluation and mutation), and
`state_contracts.md` (all-predecessor joins and fact lifetime).

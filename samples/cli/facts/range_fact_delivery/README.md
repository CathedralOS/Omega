# range_fact_delivery

Declared-range delivery proven at every site: literal `admit(0.5)` with
`requires value >= 0.0 && value <= 0.5`, narrower declared param
`forward(value)` with `requires value >= 0.0 && value <= 0.25` delivered into
`admit`, nested call `outer(admit(bounded))` inside an expression, and guard
fact `index < 4` discharging `count(index)` whose contract requires `value <= 4`.

The root program and nine application projects use canonical propositions
and run natively with `// Expected exit: 0`.

## Integer delivery row

For u64 index, index < 4 proves index <= 4 at count(index).

The root program and nine application projects use canonical propositions
for integer and float delivery cases.
Each calls the same `count` requiring `index <= 4`; each positive exercises 0
and 3 and exits 0 only when both values return unchanged.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/controls/missing_bound/main.omg` |
| field | `field/main.omg` | `field/controls/missing_bound/main.omg` |
| parameter | `parameter/main.omg` | `parameter/controls/missing_bound/main.omg` |
| call_result | `call_result/main.omg` | `call_result/controls/missing_bound/main.omg` |
| match_join | `match_join/main.omg` | `match_join/controls/missing_bound/main.omg` |
| const_name | `const_name/main.omg` | `const_name/controls/missing_bound/main.omg` |
| other_state | `other_state/main.omg` | `other_state/controls/missing_bound/main.omg` |
| other_machine | `other_machine/main.omg` | `other_machine/controls/missing_bound/main.omg` |
| target_body | `target_body/main.omg` | `target_body/controls/missing_bound/main.omg` |

Each twin weakens the transported premise to `< 6`, keeping count's `<= 4`
requirement unchanged. The match twin weakens only its right arm, testing that
every predecessor must establish the joined bound. Const-name names the source
ceiling, not a new scalar type or proof authority. All four target bodies are
source checked; native execution covers the matching host.

Run `omega run --both samples/cli/facts/range_fact_delivery/<place>/main.omg`
with finite stdin. Check a twin with `omega --check <twin>/main.omg`.
Focused corpus fixtures use the selector `integer_range_delivery_through_`.

The Windows x86-64 and Linux x86-64 rows check and run with the root program
and all nine places producing native exit 0; all nine twins reject the `count`
precondition at the intended operation. The focused corpus additionally checks
all eighteen fixtures and observes native/Terminal exit agreement at 0 for every positive.
No compiler repair was needed.


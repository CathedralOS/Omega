# Size and alignment geometry

Fact: querying size_of and align_of on a two-field u64 structural record (Pair { lo: u64, hi: u64 }) establishes its exact layout geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.

This row supports `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS.md](../../../../TASKS.md): ordinary layout observations retain their scalar facts through calls, value transport and private arrivals before target closure. The governing contracts are [layout queries](../../../../wiki/spec/layouts/plans.md#size-and-alignment-queries), [invocation and arrival contracts](../../../../wiki/spec/language/state_contracts.md), [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md) and [exact arithmetic](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons).

Every positive forms `size - (align * 2u64)` before checking the transported size against `16u64`, alignment against `8u64`, and difference against zero. Root, forwarding-machine and target-body programs run the same verification immediately after forming the difference. These runtime checks supply no preceding arithmetic premise. Both `size=0, align=0` and `size=32, align=16` would satisfy a difference-only oracle; the explicit geometry checks trap for both corruptions.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The [root program](main.omg) exercises the same geometry. Two compositions retain the same fact, bound and operation: [ordinary call results followed by a private arrival](compositions/private_arrival_calls/main.omg), and [terminal helper returns followed by a private arrival](compositions/terminal_returns/main.omg). Their [first](compositions/private_arrival_calls/invalid/main.omg) and [second](compositions/terminal_returns/invalid/main.omg) twins break only the record's second-field requirement.

Each twin removes `Pair.hi`, giving size eight and alignment eight. The unchanged subtraction underflows with bounds `[-8, -8]`. The checks after subtraction do not make that partial operation form. Positives and twins are mirrored under `tests/omega/{pass,fail}/facts/size_align_layout_geometry_*`.

Every positive binds `ProgramEntry` for Windows x86-64, Linux x86-64, Linux ARM64 and macOS ARM64 and declares exit zero. Fixture target oracles execute those four worlds against the same Terminal artifact with compiled-application provisions. Native agreement is measured on the actual execution host; binding another target is not an observation of its native execution.

```sh
python3 tools/sample_native_gate.py --cli <optimized-omega> --facts --filter cli/facts/size_align_layout_geometry --jobs 2
```

Evidence is retained in [the qualification record](../../../../wiki/drafts/measurements/layout_query_arrivals_2026_10_29/evidence.json). The fixed `9d0ba50bdc86` baseline wrongly refused the nested-machine arithmetic, the ordinary-call/private-arrival composition and a complete runtime arithmetic fixture. Checking now retains the existing admitted observation recipes on completed scalar values, composes both operands within each arrival, joins every complete path, and shares bounded return inference through ordinary and terminal descendants. No caller equality guard replaces that transport.

The row accounts for these obligations:

- Construction establishes the exact validated layout of two `u64` fields. Queries expose geometry without storage or access authority.
- Copies, fields, call returns, joins, constants and explicit machine arguments preserve the captured value's facts. Every incoming path participates; unknown predecessors and changed storage cannot donate a bound.
- Each exact operation owes its own formation condition. A safe final result cannot excuse an overflowing child. Dedicated controls retain this distinction, unknown arrivals and unknown call-result joins.
- The same portable layout-observation route supplies target geometry to checked arithmetic and Terminal/native realization. Exact geometry, subtraction and runtime control flow meet the authored oracle.

This closes the demonstrated geometry fact transport. The parent task still owes its complete `FixedLayout`/`LayoutStorage`, alias, C ABI, codec/conversion and producer-migration acceptance; this row alone does not close it.

# Checked IEEE guard for I32::from_f64

Checked IEEE guard `value == value && value >= 0.0 && value < 1000000000.0` proves `I32::from_f64(value)` is finite and in range.

This row establishes the IEEE conversion qualification required by
[CANONICALIZE-SCALAR-RANGE-CONTRACTS](../../../../TASKS.md). The guard excludes
NaN and infinities and places the truncated value within i32. The conversion
of 123.75 returns 123. An above-guard input 2000000000.0, a negative input -1.0
and NaN each take the false branch and return 0. The same fact, bound and
conversion travel through every place.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| nested machine | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| selected target body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

The [root program](main.omg) is also qualified. Every positive binds all four
hosted targets and declares Expected exit 0. Native observations here cover
macos_arm64; the other hosted entries are authored, with no execution claim
for those hosts in this qualification.

Every twin omits only the upper bound. It must refuse the unqualified conversion:
`cannot prove unqualified I32::from_f64 operand` (with diagnostic backticks).
Authored rejection fragments live in each sample.json; the corresponding
canonical corpus pin also requires the complete diagnostic prefix. No unrelated
refusal qualifies. The canonical pins are
`tests/omega/{pass,fail}/facts/ieee_guard_i32_from_f64_<place>{,_invalid}`.

| Requirement | Evidence |
| --- | --- |
| Establishment | The checked conjunction supplies non-NaN, lower and upper bounds; conversion consumes them directly. Missing upper-bound twins reject. |
| Identity and lifetime | Local copies, exact fields, parameter actuals and result contracts carry the same value. No reference authority or mutable-loan rule is introduced. |
| Composition | Calls, branch joins and nested machines retain the checked guard through the nine places. No caller assertion substitutes for the conversion obligation. |
| Soundness | IEEE comparisons retain NaN behavior. Integer/Bool equality rules do not establish floating reflexivity. The three false-guard inputs exercise separate boundaries. |
| Psi 05–07 | All ten programs execute through Terminal Psi with exit 0, including fractional conversion and false guards. |
| Omega 00–09 | The same authored programs realize and execute natively with exit 0 on macos_arm64, agreeing with Terminal Psi. The runtime footer observes conversion and branch results. |

A source-valid counterfactual replaces the local conversion with zero while
retaining the footer. Both engines trap (native signal 5, Terminal Trap), so
omitting the operation cannot satisfy the runtime oracle.

Fresh qualification uses optimized CLI and corpus runner built from
`7ef67fcfd74ed0d6cb23af7652a42727ed1fc219`. The compiler and library inputs
remain identical at the source rename/cleanup candidate. Ten authored programs
run with native/Terminal exit 0/0; all nine twins reject their intended obligation.
The focused canonical corpus checks nine positives and nine negatives and runs
all nine positive pins with native/Terminal agreement. No compiler repair is
claimed by this qualification. The separate current private two-predecessor
join probe accepts and its missing-upper-bound predecessor twin rejects; the
old README's stated source-checking frontier is therefore obsolete. That probe
is source-check evidence only.

Evidence and actual consumer revisions are retained under
`build/scalar-range-retirement-20261029/`: producer receipts,
`ieee-source-snapshot.json`, `validation-b0/`, `counterfactual-b0/`, the focused
corpus reports and `duplicate-retirement.json`. The retired repro_through pins
were duplicate positives, or identical negative programs with weaker metadata;
the canonical pins retain their obligations. The original sources are unchanged
apart from moving this folder from repro to its descriptive name.

[Historical Intel scratch-copy evidence](https://github.com/CathedralOS/Omega/blob/7ef67fcfd74ed0d6cb23af7652a42727ed1fc219/samples/cli/facts/repro/README.md#run-record)
retains its original producers and added fifth target binding. It is separate
from this unchanged-source ARM64 qualification. The parent task remains open
for other proposition, customer migration and parser-retirement acceptance.

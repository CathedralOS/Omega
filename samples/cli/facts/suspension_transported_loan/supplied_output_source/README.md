# Keep installed references usable across calls that may pause

A checked helper can install a reference in a caller-owned output record.
The caller must keep reading the original data after a call that may pause,
including when it has copied that reference or replaced the output slot.
This production extends the existing [transported-loan fact](../README.md)
under [STATIC-MACHINE-MIGRATION](../../../../../TASKS.md).

For example, the output initially points at a sentinel. The selected checked
provider then installs a reference to one backing's Cell. Reading Message must
produce 73 for the first backing and 11 for the second. A failed installation,
wrong source or wrong field offset makes main trap. The constant place selects
the installed reference instead of sentinel 0; the match join chooses two
distinct installed references; selected target implementations check distinct tags.

Fact: A loan transported into a record before a possible suspension keeps its original backing, so the record reads 73 after the call.

| Place | Positive | Policy twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected target implementation | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every positive binds four hosted targets and declares exit 0. Canaries from
the backing are read before the pause; the backing stays needed afterward
through saved references. Each twin changes only Backing's suspension policy.
A final twin must reject that policy once its positive works; an earlier
unrelated refusal does not establish this obligation.

The generic record is fully applied as Output<&Cell>; provider signatures
place the explicit lifetime on that reference argument. Its completed call
supplies the output reference's access and backing promise. The complete input
lifetimes remain separate obligations: a shared output cannot erase an original
mutable input restriction or select one convenient alternative.

The [focused controls](controls/) distinguish writable descriptor storage from
mutable access to its shared pointee, exact callee substitutions, and immutable
or shared containing storage. They also cover output replacement, earlier
argument capture, all possible suppliers, compatible views, and complete mutable
input restoration after every saved copy ends. The pointer-replacement pair
requires the old scalar snapshot to retain its bound while the current pointer
loses the old pointee's bound; the positive also checks the replacement's value.

Applicable obligations:

- Establishment uses explicit checked stores or an exact normal provider
  completion. A signature's lifetime spelling does not locate the output.
- Identity and lifetime preserve the actual output generation, captured inputs,
  complete input restrictions and every surviving reference copy.
- Composition retains every possible supplier through all nine places;
  narrowing a view does not shorten the original call-input obligation.
- Source checking, ordinary resource planning, Terminal execution and native
  realization must preserve access and restoration end to end.

## Fixed baseline and current status

The fixed base is f2c75870ebaf890328b787fa6d8ed50e868edbb8. Its optimized
nightly 2026-09-04 CLI has SHA-256
a6024d089634e19198511cc41867c1f1ee2d09f7bf7a98d8b91c10dadfd682af.
The build took 523.390 seconds. A direct-store probe checks and runs with native
and Terminal exits 0. The canonical ordinary-helper and selected-provider probes
wrongly refuse at the pause for an unavailable loan resource. The provider also
reports an incomplete stored-leaf frontier on its output record.

On that fixed baseline, all nine canonical positives refuse source checking;
their runtime oracles are unqualified. Eight initial policy twins name both
forbidden backings alongside the unrelated resource failure. The parameter twin
reaches only the unrelated call-resource fence. The strengthened match join has
its own corrected baseline. Reference-slot controls refuse earlier at
mutable-source admission; those refusals are not arithmetic or final carry-policy
evidence.

All nine places now check and run with native and Terminal exits 0 at
103a32b6b6bf27ed0bdf896e35a452b9cef025b0. All nine policy twins reach their
intended refusal for forbidden backing. The caller can fill two output records
and then choose between their distinct installed references; both choices execute.
The original 54 frozen files and their 73/11 outcomes are unchanged. Byte-identical
inputs are pinned as `supplied_output_loan_through_<place>` under
[pass facts](../../../../../tests/omega/pass/facts/) and
[fail facts](../../../../../tests/omega/fail/facts/). Corpus registration and
final integrated validation remain pending.

The same producer completes 30 programs in both engines across 82 recorded
commands (52 source checks plus those 30 runs). A saved view survives replacing
its output variable, a compatible view survives the possible pause, and an
original mutable input becomes writable after every saved copy ends. The
early-read and early-write controls still reject access before that closure.
Numbers read before pointer replacement keep their old values; arithmetic on
the replacement cannot use the old bound. A focused Rust regression also proves
that a call uses its captured write origin and keeps an unknown capture unknown
(retained result at 6efc, with that signature-state write consumer unchanged).
A shared containing record can now lend a shared view of its mutable-reference
field: the program checks and both engines observe the installed 73. Its
mutable-upgrade twin still reaches the intended access refusal.

An ordinary checked helper now keeps the input reference it saved, including
its original backing and call. An earlier argument also keeps its first value
when a later argument replaces the pointer variable; both reads run in both
engines. The [ordinary helper control](controls/ordinary_helper_saved_reference/main.omg)
and its [forbidden-backing twin](controls/ordinary_helper_saved_reference_forbidden/main.omg)
retain byte-identical copies of the frozen programs. The positive executes on
this producer; the twin's intended policy refusal is retained at a3f125, with
that policy consumer unchanged. Fixture-path qualification is separate.

The full repair remains unfinished. Borrowing an incoming pointer slot still
conflicts with a saved reference, and the alternative-handback control loses
three later qualification obligations. A conditional update of a record received
from its caller now gets past its original entry-permission error but still
lacks a lifetime resource at the pause. These two original programs and the
conditional program, scoped fixture checks, checked-to-lowered retention plans,
exact final review and main publication remain required.

The [read-only containing-record control](controls/shared_stored_input_ceiling/main.omg)
and its [mutable-upgrade reject](controls/shared_stored_input_no_mutable_upgrade/main.omg)
retain the separate native mutable-to-shared reference transport case. Their
source and Build files are byte-exact copies of the frozen candidate39 controls;
the positive ran natively and in Terminal Psi, and the twin rejected the access
upgrade. Focused fixture registration is recorded separately from that execution.

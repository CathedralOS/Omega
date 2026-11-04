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

Eight places now run with native and Terminal exits 0 at the tested
5f6b07fb3a63d3c8f24ccc71805be64d92d1003b checkpoint. Their policy twins name
both forbidden backing values. The match join still refuses while capturing the
second installation's input references; its twin has not reached the intended
policy check. The frozen witnesses and their 73/11 outcomes are unchanged.

The later tested repair at 89edff58b9cf0426947eeff5ccebedd46fc71112 runs
18 focused programs in both engines. A saved view survives replacing its output
variable, a compatible view survives the possible pause, and an original mutable
input becomes writable after every saved copy ends. The early-write control
still rejects access before that closure. Numbers read before pointer replacement
keep their old values; arithmetic on the replacement cannot use the old bound.

This remains unfinished. Ordinary helper-installed generations still lack an
input-permission association in two controls; a shared containing record reaches
a cleanup correspondence refusal. The incoming descriptor relay and qualified
alternative-handback controls also remain red. Those failures do not qualify
their intended negative controls. Full nine-place acceptance and final pins
remain required.

Sources, hashes and raw observations are retained in build/supplied-output-loan/.
The 54-file row was frozen before compiler edits. The two match-join mains were
strengthened before implementation; their initial bytes and baseline are retained
separately. Earlier bare lifetime-parameterized Output annotations and an untyped
local attempt are authoring history, not accepted compiler-defect evidence.

This checkpoint is unfinished. Native/Terminal acceptance, final diagnostic pins,
scoped tests, exact review and normal main publication remain required. The
scheduler returns inline, so actual parking, resumption and cancellation are not
claimed. Parent work outside this production, including ownerless backing and
Terminal Unresolved labels, remains open.

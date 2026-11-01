# Exact applied zero establishment

Owning task: **EXPLICIT-ENTRY-STORAGE** in [TASKS.md](../../../../TASKS.md).
Requirement: an authored ordinary entry local must establish its complete
instantiated default domain before borrowing or observing it. Retiring automatic
entry contexts must not weaken invalid-initialization controls.

**Fact:** A zeroed `Gate<0>` establishes its exact `Number == 0` application before
its code field is read. Each place copies this operation. Its twin changes only
the selected static actual to `2`, making that same default domain false.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [unestablished application](local/invalid/main.omg) |
| field | [program](field/main.omg) | [unestablished application](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [unestablished application](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [unestablished application](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [unestablished application](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [unestablished application](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [unestablished application](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [unestablished application](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [unestablished application](target_body/invalid/main.omg) |

The row is pinned under `tests/omega/{pass,fail}/facts/` as
`applied_zero_establishment_through_<place>_{exit,unestablished}`.

The declaration-to-static-actual join establishes membership for `Gate<0>`;
zero representation alone cannot establish `Gate<2>`. Each use retains its exact
application, including through containment, ordinary calls, joins, state moves,
and target-scoped bodies. Recomposition proves all declaration predicates,
including predicates mentioning no stored fields. No caller guard supplies the
missing default domain. The runtime check only observes the resulting code byte.
No proof grants backing, loan authority or termination permission.

All positives bind the same ordinary static entry on all four hosted targets.
The current local macOS ARM64 producer executed all nine with native exit 0 and
Terminal interpreter exit 0; every twin rejected at the default-domain invariant
window. The exact producer and source observations are recorded in the active
qualification record. Other hosts' native execution is unverified.

Checked-to-Terminal lowering preserves the selected application and code-byte
read. Provider settlement and layout/calling plans transport that byte through
ordinary record storage and calls. Instruction selection, activation storage,
relocations and final Mach-O emission are exercised by the actual native runs.
There are no external providers, placed inputs or new entry adapters in this row.

The frozen integrated baseline at `83dd1a639a63` wrongly accepted the local,
field, parameter, const-name, other-machine and target-body twins. Its existing
ownership checks already rejected the call-result, match-join and state-transfer twins; those
retained controls are not claimed as newly repaired behavior. The state-transfer
probe's initial syntax was corrected before its baseline and candidate replay.

## Zero image and static application

A zero image is judged against its own application. `let value: Gate<0>;` keeps
its zero image only when the default domain holds of zero fields and the exact
static actuals the application supplies, the same tuple an explicit constructor
reads. Before this reading the template's binder `Number` stayed open, the
judgment was unknown, and the local lost its zero image: native execution still
read a zeroed slot while the Terminal interpreter found no field cell.

Three controls keep the mechanism exact. [Field bound](controls/field_bound/main.omg)
combines a zero field with a static actual (`used >= Minimum`, `Floor<0>`); its
[twin](controls/field_bound/invalid/main.omg) changes only that actual to 5 and
rejects. The [open application](controls/open_application/main.omg) names
`Gate<Open>` inside a generic machine, where nothing supplies the actual, and
still rejects. They are pinned as `applied_zero_establishment_field_bound_{exit,unestablished}`
and `applied_zero_establishment_open_application_unestablished`.

This row establishes ordinary initialization validity. The companion
[installed-entry storage row](../installed_entry_storage/README.md) establishes
static-backed exclusive state and exact installed handoff with actual original
backing observations; those obligations are not inferred from this row.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.

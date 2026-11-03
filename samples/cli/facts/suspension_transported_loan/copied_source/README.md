# Keep reading through a saved reference

Programs can copy a reference, make a compatible view of the same data, and
keep using it after a call that may pause. The compiler used to forget the
original borrowing permission at the view and reject these programs.

Each witness puts references to two arrays in a record, selects one, copies
it, and forms a `Cell` view. A later call may pause. Reading the saved view must
still read the selected original array: 73 on one path and 11 on the other.
The source reference's slot may change without redirecting its earlier copy.

A copy retains the borrow that permits access; it creates no new borrow event.
View formation checks the actual captured descriptor, its presented access,
and every possible supplier. It keeps the nearest real borrowing parent and
the saved descriptors used to reach it. Replacing a slot retires its old
reference generation, while the original permission stays held until all its
surviving copies finish. A narrower view also keeps the complete input held by
an originating call.

Parent: [STATIC-MACHINE-MIGRATION](../../../../../TASKS.md). This production
closes the copied, projected and selected source-reference part of checked
per-value loan carry. It extends the existing
[transported-loan fact](../README.md) and [direct recast production](../recast/README.md);
it is not another fact row.

Fact: A loan transported into a record before a possible suspension keeps its original backing, so the record reads 73 after the call.

| Place | Positive | Reject twin |
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

Each positive binds all four hosted `ProgramEntry` targets and declares exit 0.
The twins change only `Backing`'s suspension policy. Their diagnostic pins
require both backing names and the forbidden suspension policy. Both source
choices execute, canaries catch damaged neighboring fields, and the selected
implementation must preserve its own tag. A wrong choice or read makes `main`
trap. The row is pinned as
`tests/omega/{pass,fail}/facts/suspension_saved_reference_view_through_<place>_{exit,rejected}`.

## Focused controls

All controls are pinned under
`tests/omega/{pass,fail}/borrows/suspension_saved_reference_view_*`:

- [Inherited backing](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_inherited_backing_exit/main.omg)
  and its [forbidden twin](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_inherited_backing_rejected/main.omg)
  never directly read the backing after the pause. Only the saved view keeps
  it live, so the rejection distinguishes inherited retention from direct use.
- [Source replacement](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_source_replaced_exit/main.omg)
  and an [ordinary projected borrow](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_projected_borrow_exit/main.omg)
  preserve the earlier pointer without relying on a recast-only rule.
- [Earlier argument capture](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_earlier_operand_capture_exit/main.omg)
  reads the original array after a later argument replaces its source slot.
  Its [policy twin](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_earlier_operand_original_forbidden_rejected/main.omg)
  forbids only the original backing. No post-pause direct backing read can
  supply that lifetime or policy obligation.
- [All suppliers](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_all_suppliers_exit/main.omg)
  run both selection paths, with canaries captured before the pause. The
  [forbidden-alternative twin](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_forbidden_alternative_rejected/main.omg)
  forbids only the second backing; retaining one convenient source is insufficient.
- [Shared copies from mutable storage](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_shared_cohort_call_restore_exit/main.omg)
  allow a mutating helper only after every descendant finishes. An
  [early write](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_shared_cohort_early_write_rejected/main.omg)
  still rejects. [Replacing the original shared slot](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_shared_cohort_source_replaced_exit/main.omg)
  must not release permission held by its copies or prevent eventual restoration.
- [A narrowed returned view](../../../../../tests/omega/pass/borrows/suspension_saved_reference_view_retained_call_input_source_replaced_exit/main.omg)
  survives replacement of its returned-reference slot. The view reads only
  the first field, but an [early write to the original input's second field](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_retained_call_input_early_write_rejected/main.omg)
  rejects. The same write succeeds after the view is consumed.
- [Shared-to-mutable escalation](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_shared_cannot_write_rejected/main.omg)
  and [write-only-to-readable escalation](../../../../../tests/omega/fail/borrows/suspension_saved_reference_view_write_only_cannot_read_rejected/main.omg)
  reject the actual access obligation.

## Requirements and evidence

- Establishment: checked array borrows establish the original permissions;
  compatible recasts preserve their storage and validity. Copies supply no
  new authority. See [recasts](../../../../../wiki/spec/layouts/recasts.md).
- Identity and lifetime: captures identify the descriptor generation read,
  including each actual argument before later effects. Lexical slot retirement
  remains distinct from all-holder resource closure. See
  [loan ordering](../../../../../wiki/spec/terminal-psi/loans.md) and
  [carried loans](../../../../../wiki/spec/language/lifetimes.md).
- Composition: all nine places transport the same loan into `Message`; calls
  and joins keep every supplier. The stronger returned-view control retains
  its whole original call-input debt after geometry narrows.
- Soundness: the backing's carry policy, presented read/write authority, and
  complete shared/exclusive descendant closure remain enforced. No geometry
  or enclosing-storage footprint manufactures a supplier.
- Checked-to-Terminal Psi: the existing reference, call, resource-restoration
  and suspension plans lower without another source recognizer. Every primary
  positive executes in Terminal Psi and natively with the same exit 0.
- Provider settlement, layout and calling plans: all four hosted entries are
  authored; ordinary Build-selected implementations carry explicit arguments.
  The empty scheduler implementation keeps its public possible-suspension
  requirement. Compatible array/record views keep their pointer representation.
- Instructions, homes and frames: host executions preserve both possible
  backing arrays and neighboring canaries across the call; the restoration
  controls observe writes in the original caller storage.
- Encoding and native emission: actual `macos_arm64` Mach-O execution and
  Terminal Psi agree. The branch/tag falsifiers reject wrong selection,
  pointer replacement, offset, or returned-data behavior.

The frozen baseline is `9a8b1fbf5c453b153c4ae7ae2c02296efc294c7c`, optimized
at level 1 with nightly `2026-09-04`. Its CLI SHA-256 is
`d6b7d6afdc729630c310cd2b8e4eb3f9574fcc4ab858ffc8d662ae26dab8ab60`.
All nine positives wrongly refused for an unavailable retained lifetime.
Those observations are source checks, not executions.

The repaired CLI at `c6160b0dea32f6a233bc381fcdf232d84ebd9cc6`, SHA-256
`871160cd74836a11e127d6404c0d9efcc52df0dc7a75d1b50345cbd2fc86e84b`, ran
all nine primary positives with `omega run --both`: native 0 and Terminal 0.
All nine twins named the forbidden original backings. The controls above have
separate source/runtime records. The original Main/Build bytes and exit oracles
remain frozen; only nine stale diagnostic text pins were corrected to the
observed backing-specific refusal. Source-comment, fixture and documentation
edits after that producer do not change its executable behavior. The later
constructor/path repair reused from #1626 affects Rust tests only. Nine invalid
sample metadata labels were corrected from the unsupported `negative` spelling
to `intended-invalid`; their original frozen hashes remain in the run records.

Final scoped corpus records use source stamp
[`24e82dfa56fd9dcb9eefb9c3cdea25c8d700f328`](https://github.com/CathedralOS/Omega/commit/24e82dfa56fd9dcb9eefb9c3cdea25c8d700f328)
and the retained optimized CLI/runner from `a39c93abfa652af674bd3139640b8f15339616eb`.
All 33 fixtures pass their source oracle (17 accepted, 16 intended refusals);
all 17 positives execute with native 0 and Terminal 0. Six existing direct/recast
controls also agree in both engines. All 18 sample checks pass, including the
nine policy twins. The affected checking and lowering crates pass 28 and 15
unit tests respectively. Checked-trees passes 27 tests and fails its existing
String-identity guard on ten fields: the guard and every flagged file are
byte-identical to the fixed base. This is source attribution, not a fresh
baseline test execution.

Matching-host execution on Windows and both Linux hosts remains unqualified.
The scheduler completes inline, so these runs establish the possible-suspension
contract, not actual parking, resumption, or cancellation. Projected call
referents, supplied-output/lifetime-dependency descriptors, backings without
an enclosing owner, Terminal `Unresolved` labels on accepted transfers, and
the rest of the parent migration remain open on `TASKS.md`.

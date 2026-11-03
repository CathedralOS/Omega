# Keep a helper's saved view across a call that may pause

A helper can return a reference to part of its input, and the caller can keep
reading that original data after a call that may pause. These programs used to
be rejected because the compiler lost the returned view's borrowing permission.

For example, `project(source)` returns a reference to an array field. A saved
result must keep reading 73 from the first backing even after `source` points
to a second backing containing 11. A later argument that replaces a source
slot also cannot redirect an earlier argument's saved reference. When the
helper borrows an entire record, its returned field view keeps that complete
input borrowed until the view finishes; another field cannot be written early.

The compiler records the checked normal-return relation and the exact evaluated
reference that lends the new view. Copies and selections follow every saved
source to its real borrowing parent. Calls and ordinary view formation use the
same source lookup. A projected child remains a new borrow; pointer forwarding,
shared access, full input lifetime debt and slot replacement retain their own
meaning. Installed outputs and returned references use the same evaluated input
handles, including after dynamic selector and receiver separation.

Parent: [STATIC-MACHINE-MIGRATION](../../../../../TASKS.md). This production
establishes checked carry for helper-returned projections of owned input backing,
including selected/copied actuals and compatible caller views. It extends the
existing [transported-loan fact](../README.md), rather than adding another row.

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

Every positive binds all four hosted targets and declares exit 0. Both choices
execute: the saved view reads 73 or 11, neighboring canaries must survive, and
the selected target implementation must retain its own tag. Wrong selection,
address, offset or returned data makes `main` trap. Each twin changes only the
original `Backing` suspension policy; its diagnostic pins require both backing
names and the forbidden policy. Pins are
`tests/omega/{pass,fail}/facts/suspension_projected_call_view_through_<place>_{exit,rejected}`.

## Focused controls

The controls are pinned under
`tests/omega/{pass,fail}/borrows/suspension_projected_call_view_*`:

- `returned` / `returned_forbidden` use only the saved view after the pause.
  `returned_direct_read` reads the returned reference without a recast, and
  `inline_source` calls `project(&backing)` without a preformed source local.
- `returned_alternatives` exercises both projected normal returns.
  `all_suppliers_only_view` / `forbidden_alternative` select between two
  backings, capture canaries before the pause, and forbid only the second in
  the twin. One convenient supplier cannot satisfy the check.
- `source_replaced` observes saved 73 and replacement 11. `earlier_argument`
  makes a later argument replace a record's reference slot while the earlier
  argument keeps its old value. `earlier_argument_original_forbidden` forbids
  only the original backing and must name it after replacement.
- `whole_input_restore` / `whole_input_early_write` distinguish a write after
  consuming the narrowed result from a write while the whole call input is
  still held. `shared_cohort_restore` / `shared_cohort_early_write` also keep
  a sibling copy live after the first view finishes.
- `shared_result_cannot_write` rejects writable access through a shared
  returned presentation. Existing shared and write-only access refusals remain
  covered by focused checks.
- `reborrow_selected` passes `&selected` explicitly; `entry_source` forwards
  a genuine incoming reference. Neither a named local nor lifetime metadata
  about references inside an input is required to supply entry permission.

## Requirements and evidence

- Establishment: checked explicit borrows and the helper's complete normal
  return establish the projected child. Signatures, geometry and lifetime
  roots cannot invent a borrowing source.
- Identity and lifetime: the caller keeps the exact descriptor generation and
  nearest real parent. Each actual freezes at its own completion; a descriptor
  reached through a pointer is read at invocation entry. Slot replacement does
  not change an earlier saved value. See [lifetimes](../../../../../wiki/spec/language/lifetimes.md).
- Composition: every selected source and checked normal-return alternative
  remains required. Complete call-input debt survives a narrower result and
  later compatible view. The nine places retain the same fact and operation.
- Soundness: original backing policy, source access, backing-owner bounds and
  complete shared/exclusive closure remain enforced. See
  [carried resources](../../../../../wiki/spec/resources/carry.md) and
  [loan parents and ordering](../../../../../wiki/spec/terminal-psi/loans.md).
- Checked-to-Terminal Psi: returned views, copies, calls and restoration lower
  through the existing resource graph. Every positive executes in Terminal Psi
  and natively with exit 0.
- Provider settlement, layout and calling plans: four hosted entries are
  authored, Build chooses the implementation, and compatible views preserve
  address and access. The inline scheduler's body does not erase its public
  possible-suspension requirement. See [recasts](../../../../../wiki/spec/layouts/recasts.md).
- Instructions, homes and frames: host runs preserve the old saved reference
  and the updated source variable independently, including across calls.
- Encoding and native emission: actual `macos_arm64` Mach-O execution agrees
  with Terminal Psi; branch/tag/canary and replacement oracles expose wrong
  backing selection and field offsets.

The fixed baseline is `e4617438fb66df39eeb1fa6605b8fcacba19df62`, optimized
at level 1 with nightly `2026-09-04`; CLI SHA-256
`6033cf737ddf2edbef3178ebd9321b59f3efcb64ba0db5cf65069a28e780eca1`.
The direct projection probe checked and executed in both engines. All nine
helper-returned positives wrongly refused for a missing retained lifetime.
The constant witness's initially malformed array projection was corrected and
rechecked on that retained baseline before compiler edits. Original attempts,
corrected frozen bytes and raw observations remain separately recorded.

Candidate `6f254fd148f830b2ef933cdcfd9de349e849d327`, CLI SHA-256
`cd372da148fcd08d35c36cc1df98a786b21816bb063db340f1584f65b8ac96ec`,
checks the nine positives and runs each with `omega run --both`: native 0 and
Terminal 0. All nine twins name the original backings' forbidden policy.
Focused controls have their own raw observations: the original-only policy
refusal names `first`, and the alternative-only refusal names `second`, with
no independent post-pause backing read. Five existing generic/attenuation
positives remain accepted; three existing selected shared/exclusive/byte
stored-output cases also execute in both engines.

Matching-host execution on Windows and both Linux hosts remains unqualified.
The scheduler completes inline: these runs establish the possible-suspension
contract, not actual parking, resumption or cancellation. Supplied-output and
lifetime-dependency descriptors, backing without an enclosing owner, Terminal
`Unresolved` labels on accepted transfers, and the remaining parent migration
stay open on `TASKS.md`.

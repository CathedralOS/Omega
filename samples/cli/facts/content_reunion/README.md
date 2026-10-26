# Local content reunion

One owned root may be partitioned and reconstructed from its complete live
descendant claims; equal geometry alone supplies no ownership.

The operation is that reunion. The bound is base 10 and length 48. Every place
copies that sentence, partitions the issued root at requested counts 0, 16, and
48, and reconstructs one region whose base is 10 and whose length is 48. Exit 0
requires the reconstructed region to be in `Owned`. A freshly issued root of
the same base and length does not supply that ownership.

Every place now calls a separately checked `merge(left, right)` whose two
owned parameters are its complete input frontier. The merge has no parent root
in scope. Its authored separated-content equation and ordinary bounds establish
one owned output. The caller then reconstructs a raw region and must derive its
qualification from the original root's complete live descendants. After each merge, the same fact is carried through a separately checked finisher:
it partitions the reunited root at its full length, explicitly disposes the empty
tail with the owner-designated cleanup, and rebuilds the remaining region. The
complete input roster is accounted for by returned origins and exact disposal,
without treating numeric emptiness as ownership authority. The other
machine also composes a two-parameter wrapper, selects both call alternatives,
and reverses the formal order in one helper.

This discharges the same-owner numeric split/recombine requirement of
**EXTENT-LIBRARY-TRANSFORMS / CONSERVATION-CONTRACT**. Physical backing,
provider issuance, cross-owner boundary custody and Cathedral remain open.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | several reunion shapes in one program |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the descendant claims are locals |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the parts live in a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `observe` receives the reunited region |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `deliver` returns the reunited region |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms reconstruct the root |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `BASE` and `LENGTH` name the bound |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine reconstructs the root |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `forward` reconstructs and `rejoin` forwards |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies reconstructs |

## Execution

Every positive place and the root bind `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64`, and `linux_arm64`, declare `Expected exit: 0`,
and pass `omega run --both`. Earlier qualification reported Windows native and interpreter exit 0 for the
root and all nine places. Fresh engineering runs at `ad1772d8f612` also observed
native and Terminal exit 0 on macOS ARM64, with all nine ownership twins
rejecting. The retained Windows report was not revalidated by those runs.

```sh
omega run --both samples/cli/facts/content_reunion/main.omg
omega run --both samples/cli/facts/content_reunion/local/main.omg
omega run --both samples/cli/facts/content_reunion/field/main.omg
omega run --both samples/cli/facts/content_reunion/parameter/main.omg
omega run --both samples/cli/facts/content_reunion/call_result/main.omg
omega run --both samples/cli/facts/content_reunion/match_join/main.omg
omega run --both samples/cli/facts/content_reunion/const_name/main.omg
omega run --both samples/cli/facts/content_reunion/other_state/main.omg
omega run --both samples/cli/facts/content_reunion/other_machine/main.omg
omega run --both samples/cli/facts/content_reunion/target_body/main.omg
```

The oracle discriminates. Each invalid twin keeps the reunion expression and
changes only the partitioned root from the input to `issue()`, then drops the
input. The fresh root still has base 10 and length 48. Checking rejects
`result in Region::Owned`. The equal geometry still proves the mathematical
content equation; it supplies no lineage from the consumed input to the fresh
root. Every twin therefore pins the routed ownership failure. The match-join
and successor-state twins report it for each reconstructing arm.

## Pinned fixtures

Pass fixtures `tests/omega/pass/facts/content_reunion_through_<place>_exit`
copy the place program and record native exit 0. Fail fixtures
`tests/omega/fail/facts/content_reunion_through_<place>_rejected` copy the
twin. Each invalid sample carries `sample.json` with the same fragments.

## Requirements and evidence

- **Owning contract:** [content custody](../../../../wiki/spec/resources/content_custody.md)
- **Parent outcome:** **EXTENT-LIBRARY-TRANSFORMS** in [TASKS.md](../../../../TASKS.md).
  This row establishes separately checked same-owner numeric reunion and its
  checked-wrapper composition. Admitted initial supply, exact cross-owner
  backing correspondence, physical storage and Cathedral acceptance remain open.

1. **Establishment:** `issue` is the only establishment of the base-10
   length-48 root. `partition` authors the split of that root. The reunion
   body consumes both live children and rebuilds one region. `result in Owned`
   and the content equation with `old(&whole)` are that body's obligations.
   The caller does not re-establish them with a later equality guard.
2. **Identity and lifetime:** the reconstructed region keeps the input root's
   lineage. A second `issue()` of equal geometry is a different root. Dropping
   the input and partitioning the fresh root leaves the numeric bound intact
   and still refuses `Owned`.
3. **Composition:** the same reunion is carried through a local, a field, a
   parameter, a call result, both match arms, the named bound, a successor
   state, a forwarding machine, and four target bodies. Each twin differs
   only by partitioning `issue()` and dropping the input root.
4. **Soundness boundaries:** all nine twins reject during checking. Equal
   intervals do not grant the input's ownership. Overlap, gaps, mutation, and
   mixed-origin controls under `tests/omega/fail/core/content_reunion_*`
   remain separate from this row.
5. **Checked-to-Terminal Psi lowering:** the positive places lower the
   reunion, the domain obligation, and the exit choice into Terminal Psi.
   `omega run --both` agrees on interpreter exit 0 and native exit 0.
6. **Provider settlement and layout:** each positive binds all four hosted
   `ProgramEntry` values. `Region` is two `u64` fields and `Parts` is two
   regions. Calls and the reunited return use ordinary aggregate calling plans.
7. **Instruction selection and frames:** the Windows x86_64 native image
   executes the partition arithmetic, the reconstruction, and the exit
   selection. The other three targets are checked through their entry bindings
   and, for `target_body`, through their checked bodies. This host did not
   execute those images.
8. **Encoding and emission:** the Windows native image runs and agrees with
   Terminal Psi. Branch layout for the empty, interior, and full requests is
   in that image.

### Current repair and remaining acceptance

The fresh optimized compiler at baseline `f6b76a817aa` rejected the separately
checked merge's returned `Owned` claim. The first repair exposed a second
refusal when a two-input wrapper reconstructed a raw result after a completed
merge. Joint frontier replay now retains all original inputs and each current
claim's origins. A third refusal, `structural result has no selected disposition`,
exposed missing content-parent operands in executable value planning. Checked
values now retain the exact transaction and its complete owned operands before
Unit disposition planning, and Terminal lowering consumes that transaction.

The earlier Windows observations above cover the earlier local reconstruction
production. They are historical evidence, not runs of this new merge production.
Fresh qualification of the new production is recorded in this change's PR and
its retained producer, raw engine streams, products and reject diagnostics.

A wrong-root twin must still reconstruct an unqualified raw result after the
checked merge: forwarding that merge's already-qualified result would be a legal
use of independently established authority and would not test the original root's
live frontier. The twins change only `partition(whole, requested)` to
`partition(issue(), requested)` plus disposal of `whole`; they retain the same
merge and raw reconstruction as their positive place.

Physical backing, selected provider issuance and Cathedral's retirement/readback
remain unfinished. A joint ancestry roster is not an original-to-current backing
address or per-input projection correspondence. Cross-owner boundary bodies still
require that exact correspondence and refuse unsupported multi-origin substitutions
without consuming inputs or publishing a qualification.

The disposal finisher exposed a canonical-artifact mismatch: native and Terminal
execution returned 0, while `inspect-terminal` rejected the retained `program.oterm`
because the disposed tail had no returned claim origin. Terminal ancestry now
accounts for exact nominal disposal separately from returned origins. Its bounded
replay follows the complete claim frontier on every normal path and accepts only
the uniquely designated leaf cleanup at the current typed storage. Home retirement,
retaining hooks, unaccounted descendants and opaque custody exits do not establish
disposal. Qualification therefore checks canonical reload of every retained
Terminal artifact in addition to both execution engines.

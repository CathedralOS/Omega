# Local content reunion

One owned root may be partitioned and reconstructed from its complete live
descendant claims; equal geometry alone supplies no ownership.

The operation is that reunion. The bound is base 10 and length 48. Every place
copies that sentence, partitions the issued root at requested counts 0, 16, and
48, and reconstructs one region whose base is 10 and whose length is 48. Exit 0
requires the reconstructed region to be in `Owned`. A freshly issued root of
the same base and length does not supply that ownership.

This is numeric custody, not physical storage, an allocator, or the separate
checked-wrapper witness under CONSERVATION-CONTRACT.

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
and pass `omega run --both`. This Windows host observed native exit 0 and
interpreter exit 0 for the root and all nine places. Those bindings do not
by themselves execute the other three hosts.

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
`result in Region::Owned`. Straight-line, field, parameter, call-result,
const, forwarding, and target-body twins also reject
`Owned.content(old(&whole)) == Owned.content(&result)`. The match-join and
successor-state twins report the ownership failure for each reconstructing arm.

## Pinned fixtures

Pass fixtures `tests/omega/pass/facts/content_reunion_through_<place>_exit`
copy the place program and record native exit 0. Fail fixtures
`tests/omega/fail/facts/content_reunion_through_<place>_rejected` copy the
twin. Each invalid sample carries `sample.json` with the same fragments.

## Requirements and evidence

- **Owning contract:** [content custody](wiki/spec/resources/content_custody.md)
- **Parent outcome:** the content-reunion fact row. This does not discharge
  CONSERVATION-CONTRACT's checked-wrapper witness.

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

### What remains open

Physical backing, provider issuance, and Cathedral's resident memory are not
established here. A callee that reunites two region parameters without the
parent root in scope still has no interval-set body judgment; these places
author the reunion in the body that partitioned the root, or forward a reunion
that body already proved.

# Double-indexed read-modify-write

A const write of 41 at `grid[1][2]` becomes 42 after `grid[i][j] = grid[i][j] + 1`
with `i == 1` and `j == 2`, and `rows[2].data[1]` becomes 42 after the same
increment while `rows[2].tag` stays 7.

The operation is that pair of read-modify-writes through borrowed caller
storage. Both arrays are locals loaned into the state that writes them. The
grid cell is selected by two runtime indices. The row cell is a field between
two runtime indices. Exit 0 requires both observations. This witness does not
depend on `source/library/std`.

Every place copies that fact sentence and operation. Only the transport of the
borrowed storage changes: a local, a record field, an explicit parameter, a
call result, both match arms, the named bound, a successor machine, a forwarding
machine, and per-target bodies. The twin of each place stores 40 instead of 41
at `grid[1][2]`, so the cell observation is false; indices, increment, and the
row store stay.

The corpus fixtures `runtime_double_indexed_rmw_exit`,
`runtime_indexed_struct_field_rmw_exit`, and `runtime_hoisted_index_write_exit`
keep their authored exits. On this host they stop at program-entry admission
because `Build::depend` on std asks for an accepted `WindowsX86_64Application`
binding. That wall is not this operation.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) | the writing states borrow the entry machine's locals |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both arrays stay locals of the entry machine |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both arrays live in one record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | one machine receives both arrays as parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | both arrays arrive as call results |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms write and one successor observes |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named bounds supply both indices and both stores |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine writes and observes |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `Writer::run` writes through borrowed caller storage |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one `rmw` body per hosted target |

## Execution

Every positive place and the root bind `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares
`Expected exit: 0`. Each twin keeps the same bindings and declares the trap.

```sh
omega run --both samples/cli/facts/double_indexed_rmw/main.omg
omega run --both samples/cli/facts/double_indexed_rmw/<place>/main.omg
```

Measured on this macOS arm64 host with the wave's shared generation omega
(595562a958b, opt_level 1, sha256
`5c805564eb160e5714885a302f42b97dc34c85e59d877a96dbe685ec98c20fbd`):
`omega run --both` reports native exit 0 and interpreter exit 0 for the root
and all nine places (10/10), and a native signal 5 with interpreter
`crash: Trap` and host presentation agreement for the root and all nine twins
(10/10). An earlier Intel macOS x86_64 host measured scratch copies that added
`macos_x86_64::ProgramEntry`, because none of the four authored targets
executes there; a Windows x86-64 host previously reported the same agreement
for the root and its twin at `ff5d57b7c845`.

## Pinned fixtures

Pass fixtures `tests/omega/pass/facts/double_indexed_rmw_through_<place>_exit`
copy the place program and record the native exit 0 oracle. Trap fixtures
`tests/omega/run/facts/double_indexed_rmw_through_<place>_traps` copy the twin
and record the `trap` oracle, following
`tests/omega/run/facts/signed_division_quotient_truncation_not_floor_traps`.

The pinned fixtures carry this host's recorded rows, measured with the wave's
opt-level 1 corpus runner at `595562a958b` (sha256
`fa7c9b282deaefd7f38eafd3b4ea07d960ad1eb7e679d23253fef05ed8ff30dd`): the check
leg records all eighteen as `checked check-target:macos_arm64`,
`corpus_native_macos_arm64.txt` records the nine `built ... exit:0` positives
and the nine `built ... exit:signal signal:5` traps, and
`corpus_both_macos_arm64.txt` records the same runs with
`interpreter-exit:0 interpreter:agrees` and `interpreter:trapped`.

## Requirements and evidence

- **Owning contract:** [structural access](../../../../wiki/spec/terminal-psi/structural_access.md)
- **Parent outcome:** PLACED-ACCESS-NATIVE-OPS. This witness discharges
  source-to-native indexed access through borrowed caller storage for this
  double-index read-modify-write. It does not close the rest of that item.

1. **Establishment:** the state that receives the borrowed array writes 41,
   then increments that same cell through the runtime indices. The following
   read is the observation. A later caller guard is not what establishes it.
   Every place authors the same write-increment-observe body; no place
   re-establishes the value in the caller.
2. **Identity and lifetime:** `i` and `j` select the cell that the const write
   named. The row tag is a sibling of the incremented field and stays 7. The
   twins differ only in the stored grid value, so a wrong cell or a stale fold
   still fails the observation.
3. **Composition:** the borrowed grid and the borrowed row array are the
   caller's storage through a local, a field, a parameter, a call result, both
   match arms, the named bound, a successor machine, a forwarding machine, and
   four target bodies. The observation after each transport is the consuming
   operation, and both engines agree on it.
4. **Soundness boundaries:** the false store must not complete as exit 0. All
   nine twins take their `crash Trap` route natively and in Terminal Psi
   instead of reporting exit 0.
5. **Checked-to-Terminal Psi lowering:** the RMW, both runtime indices, the
   field-between-indices projection, and the exit choice lower into Terminal
   Psi; `omega run --both` reports interpreter exit 0 for every positive place
   and `crash: Trap` for every twin.
6. **Provider settlement and layout:** each positive binds all four hosted
   `ProgramEntry` values. `Row` is a tagged-field record over a two-element
   `i32` array, and both arrays are locals whose addresses travel only as
   loans to the writing machine or state.
7. **Instruction selection and frames:** the `macos_arm64` images execute the
   nested-index arithmetic, the field store between two runtime indices, and
   the exit selection, including the twins' `crash Trap` route. The other
   three authored targets are checked through their entry bindings and, for
   `target_body`, through their checked bodies; this host did not execute those
   images.
8. **Encoding and emission:** the executed images agree with Terminal Psi,
   including the trap presentation on the twins.

### What remains open

Native execution on `windows_x86_64`, `linux_x86_64`, and `linux_arm64` (this
host pins `macos_arm64`; `omega run --target` is compile-publication only, so
it supplies no runtime claim). The rest of PLACED-ACCESS-NATIVE-OPS — placed
MMIO access and the wider address-composition frontier — is not established
here.
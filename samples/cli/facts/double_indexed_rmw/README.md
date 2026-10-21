# Double-indexed read-modify-write

A const write of 41 at `grid[1][2]` becomes 42 after `grid[i][j] = grid[i][j] + 1`
with `i == 1` and `j == 2`, and `rows[2].data[1]` becomes 42 after the same
increment while `rows[2].tag` stays 7.

The operation is that pair of read-modify-writes through borrowed caller
storage. Both arrays are locals loaned into the state that writes them. The
grid cell is selected by two runtime indices. The row cell is a field between
two runtime indices. Exit 0 requires both observations. This witness does not
depend on `source/library/std`.

The corpus fixtures `runtime_double_indexed_rmw_exit`,
`runtime_indexed_struct_field_rmw_exit`, and `runtime_hoisted_index_write_exit`
keep their authored exits. On this host they stop at program-entry admission
because `Build::depend` on std asks for an accepted `WindowsX86_64Application`
binding. That wall is not this operation.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) |

The twin changes only the grid store from 41 to 40. The indices, the increment,
and the row store stay. The grid observation is then false. Checking accepts
that source. `invalid/expected_native_exit.txt` requires the trap:
`STATUS_ILLEGAL_INSTRUCTION` on Windows x86-64.

```sh
omega run --both samples/cli/facts/double_indexed_rmw/main.omg
omega run --both samples/cli/facts/double_indexed_rmw/invalid/main.omg
```

On Windows x86-64, opt-level 1, after rebasing onto
`ff5d57b7c845c9e903bddba537b7a8688b70d079`: the positive witness reports
native exit 0 and interpreter exit 0. The twin reports native exit
`-1073741795` (`0xC000001D`) and interpreter crash `Trap`, and the host
presentation agrees.

## Requirements and evidence

- **Owning contract:** [structural access](../../../../wiki/spec/terminal-psi/structural_access.md)
- **Parent outcome:** PLACED-ACCESS-NATIVE-OPS. This witness discharges
  source-to-native indexed access through borrowed caller storage for this
  double-index read-modify-write. It does not close the rest of that item.

1. **Establishment:** the state that receives the borrowed array writes 41,
   then increments that same cell through the runtime indices. The following
   read is the observation. A later caller guard is not what establishes it.
2. **Identity and lifetime:** `i` and `j` select the cell that the const write
   named. The row tag is a sibling of the incremented field and stays 7.
3. **Composition:** the borrowed grid and the borrowed row array are the
   caller locals. The twin differs only in the stored grid value.
4. **Soundness boundaries:** the false store must not complete as exit 0.
5. **Checked-to-Terminal Psi and native execution:** `omega run --both` on
   the positive witness. Record the host result in the claim note.

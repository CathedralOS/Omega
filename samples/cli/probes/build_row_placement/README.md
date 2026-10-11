# build_row_placement

Row placement is order-insensitive:

- `use kit::main` after a machine declaration still imports — use
  rows are not header-only.
- `Build::depend_as` after other build statements (here after
  `exclude_crash`) still projects the edge — dependency rows may
  appear anywhere in the build entry, as long as they are direct
  unconditional statements there.

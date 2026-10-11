# build_sibling_divergent_build

Five product siblings pooled on one `Build::application` request:

- `a`, `c`, `e` converge: each build edge-pools on `../shared` and its
  build calls `shared::main`'s `seed()` returning 3.
- `bad1` (position 2): build-eval divergence — its build retires a
  required-output obligation with `BuildOutput::fail`.
- `bad2` (position 4): checked-compilation divergence — its build source
  calls `ghost_machine()` which does not exist.

The question pinned: which package does each diagnostic name — the
divergent sibling's own occurrence, the requesting package, or the pool —
and do both failures surface or does the first rejection mask the second?

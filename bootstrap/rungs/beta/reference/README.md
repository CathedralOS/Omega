# Beta executable reference meaning

This directory owns the untrusted, executable reference meaning for the Beta
rung. `beta_parser.py` recognizes the Beta source surface and `beta_interp.py`
executes its tuple AST. The fuzz and exhaustive-I/O gates compare that meaning
with programs produced by the canonical self-hosting Beta compiler.

The owner contains no compiler backend and imports no refinement machinery.
Its comparisons are diagnostics, not artifact authority. Acceptance of a
compiler artifact still requires the lower-rooted refinement edge described by
the bootstrap lattice.

Run `ownership-test.sh`, `beta-correctness-fuzz.sh`, and
`beta-io-exhaust.sh` from any working directory.

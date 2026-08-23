# Beta refinement reconstruction

This directory owns untrusted reconstruction of Beta source meaning for
lower-rooted compiler refinement. `beta_symbolic.py` derives closed terms and
`symbolic_loop_check.py` pins its loop summaries to the executable reference
over concrete input grids.

The shared parser and concrete interpreter remain under
`bootstrap/rungs/beta/reference/`. Reconstruction may consume that meaning
surface, but it neither compiles Beta nor grants an artifact authority.

Run `ownership-test.sh` and `symbolic-loops.sh` from any working directory.

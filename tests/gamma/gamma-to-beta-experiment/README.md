# Gamma-to-Beta differential gate

`run.sh` validates the retained concatenative Gamma-to-Beta compiler against exact
receipts, representative Delta-generated Gamma, and the Alpha tape limit.

`delta_recursive.gamma` and `delta_surface.gamma` are unchanged concatenative
receipts from the retired Delta compiler-slice gate. This experiment is their
only remaining executable consumer and owns their existing exact identity
checks. They are not receipts of the selected functional Gamma route.

`direct_compiler.gamma` is the former Gamma-to-Alpha compiler. It is retained
here only as a test-owned differential oracle: the concatenative compiler lowers it
through Beta, then both routes must produce byte-identical Alpha tapes. It is
not part of the selected source spine or a bootstrap premise.

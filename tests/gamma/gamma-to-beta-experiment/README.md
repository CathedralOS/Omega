# Gamma-to-Beta differential gate

`run.sh` validates the selected Gamma-to-Beta compiler against exact retained
receipts, representative Delta-generated Gamma, and the Alpha tape limit.

`direct_compiler.gamma` is the former Gamma-to-Alpha compiler. It is retained
here only as a test-owned differential oracle: the selected compiler lowers it
through Beta, then both routes must produce byte-identical Alpha tapes. It is
not part of the selected source spine or a bootstrap premise.

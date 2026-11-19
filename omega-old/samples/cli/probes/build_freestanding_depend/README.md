# build_freestanding_depend

The application registers `dep/` as a local package with `Source::Path`.
The sample gate captures the same dependency subtree that the source graph
loads for the `edge_dep` package, so both views of the nested source agree.

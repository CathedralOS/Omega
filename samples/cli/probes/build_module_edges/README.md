# build_module_edges

The dependency contains both `util.omg` and `util/mod.omg`. The entry imports
`{kit}/util.omg` and calls its `flat` machine.

The `invalid_use_dep_build` control selects a machine from `{kit}/build.omg`.
The dependency’s Build file is private to its planning sequence.

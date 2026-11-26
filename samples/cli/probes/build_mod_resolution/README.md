# build_mod_resolution

The entry imports the exact file `{kit}/util/mod.omg` and calls its `u` machine.

The `invalid_dir_no_mod` control imports `{kit}/util.omg`. The dependency
contains a `util/` directory and has no file at the requested path.

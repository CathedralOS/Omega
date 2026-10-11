# build_nonmain_module

`use kit::other` imports the `other.omg` file of package `kit`, not
just its `main.omg`: `other_v()` resolves through the non-default
module while `main.omg` stays unimported. Module selection is by
file name — each `use` row names one `*.omg` leaf.

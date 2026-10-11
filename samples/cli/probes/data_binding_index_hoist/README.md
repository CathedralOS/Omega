# data_binding_index_hoist

A computed index rooted at the machine's DATA BINDING hoists into a
temp: `main.cells[main.i - 1]` is a Binary index whose leaf is a field
of the record this machine operates on, and `i`'s declared `[1..=4]`
range proves `i - 1` lands inside `cells`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

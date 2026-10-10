# whole_element_store

Whole-record store through a runtime index on an element view: `nodes[pick] = fresh` with `pick` a computed scalar — the write lands on the owner view observed back through `bag.nodes[2]`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

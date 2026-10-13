# nested_leaf_rank

The projected slice coordinate resolves through a nested member
chain: `outer.inner.items` names the leaf slice under the exact
`inner` record step, the requires fact and guard spell that same
chain, and every arrival rebuilds both declarations down to the
ranked leaf.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

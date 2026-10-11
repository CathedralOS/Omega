# subslice_folded_facts

Let-bound computed endpoints subslice a view: `view[start..]` tail and
`view[start..end]` middle fold `start + width` into a bound fact; the
later `end - 3` pick must bind to a local first — a bare computed index
is fenced.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# transitive_frame_facts

First sample composing may-write summaries through a
sibling call chain: `Main::middle` calls `Main::touch`,
neither writes `stable` — the caller's local facts survive
the whole transitive frame.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

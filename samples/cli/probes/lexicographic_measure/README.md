# lexicographic_measure

Two machines share the same declared lexicographic view:
`measure Progress::Steps lexicographic { outer, inner }` ranks the
pair in dictionary order — the first edge forwards the tuple
unchanged, the second preserves `outer` and strictly decreases
`inner`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

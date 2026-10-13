# machine_const_indices

The same `keep` signature spelled in three modules reads a different
`SIZE` per module — `Buffer<SIZE + 1>` resolves against the declaring
module's constant, so the three `keep` machines have distinct types
even though each imports `main` cyclically for `Buffer`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

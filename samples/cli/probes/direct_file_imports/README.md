# direct_file_imports

Each source imports the exact files supplying its names: `main` sees
`A` and `B`, while `m/b.omg` saw `m::a` for its own field — imports are
per-file, not transitive visibility.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

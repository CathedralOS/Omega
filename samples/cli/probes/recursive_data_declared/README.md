# recursive_data_declared

Direct (`Nat`) and mutual (`Tree`/`Forest`) recursive data are LEGAL to declare; the proof-only classification fences runtime consumption, not declaration — declaring them and never constructing them compiles clean.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

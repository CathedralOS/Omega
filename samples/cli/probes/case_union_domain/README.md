# case_union_domain

Authored case-union domain covers its members on dispatch: `domain Command::Interactive(command) requires command in Command::Move | Command::Say` — the `Command::Interactive` transition arm matches either tag and counts both toward coverage (no wildcard arm needed). LEARNING/GAP-adjacent: the same spelling inside a `match` subject is rejected `match does not cover its subject` — union-domain coverage is currently transition-only.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

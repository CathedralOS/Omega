# union_guard_paren

A parenthesized union of implicit case domains as the transition guard
subject: `(main.cmd in Command::Quit | Command::Move)` is a tag-only
test even for the payload-bearing `Move` arm — the constructed payload
does not leak into the compare.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

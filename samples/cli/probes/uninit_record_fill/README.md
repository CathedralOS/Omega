# uninit_record_fill

First sample with declaration-only locals: `let point:
Point;` fills member by member (`mut` gates whole-local
rebinding, not field establishment), and `let mut made:
Point;` reads its zeroed shape — `made.x` projects the
declared `Trapping` field out of the established zero.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

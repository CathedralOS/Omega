# domain_recast_alias

First sample retaining a domain through a mutable recast:
`&mut value as &mut u32` on a `u32 in Small` field stays
`&mut u32 in Small` — both views denote the same
established value set; the view suspends the owner until
its last use, then `main.value` reads again.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

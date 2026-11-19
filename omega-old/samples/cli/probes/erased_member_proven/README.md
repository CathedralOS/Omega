# erased_member_proven

An `[erased]` payload member is supplied from its unique nullary constructor
at construction — `Certified::Proven { value: 5 }` needs no `proof` argument
— takes no runtime position, and the arm destructures `proof as _`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

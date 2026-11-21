# trait_default_inherit

Trait requirements carry default bodies: `machine
reset(subject: &mutable T) { set(&mutable subject, 30); }` —
a named implementation satisfying a child trait
receives the inherited machine through header-parent
composition (`trait RequiredCounter<T>:
Resettable<T>`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# trait_refinement_axes

`trait LocalLogger<T> = Logger<T> { machine *
suspends false; blocks false; terminates; machine
Logger::flush terminates; }` — transparent refinement:
a structural bound narrowed by operational axes
(`machine *` wildcard + member-qualified rows), not a
`satisfies` target; evidence binders are its legal
consumers.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

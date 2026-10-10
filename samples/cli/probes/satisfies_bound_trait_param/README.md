# satisfies_bound_trait_param

First sample on `satisfies`-bounded trait parameters:
`trait Service<Policy satisfies Decider>` requires the
type argument to carry a named Decider implementation,
wired by the `X satisfies T { T::member = Impl::member; }`
roster.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Learnings

- A machine that `satisfies` a trait retires its
  value-qualified call route: `Registrar::pick(6)` is
  rejected ("value-qualified machine calls are retired;
  use a declaration namespace"); `Decider::pick(6)` — the
  trait's declaration namespace — is the callable
  spelling.

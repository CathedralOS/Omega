# satisfies_bound_trait_param

First sample on `satisfies`-bounded trait parameters:
`trait Service<Policy satisfies Decider>` requires the
type argument to carry a named Decider implementation,
wired by the `X satisfies T { T::member = Impl::member; }`
roster.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Learnings

- An implementation names a map of machines, not a type
  ([data and code are independent](../../../../wiki/spec/language/conformances.md#data-and-code-are-independent)),
  so it is named apart from the data whose machine it binds:
  `RegistrarDecider satisfies Decider { Decider::pick = Registrar::pick; }`.
- "Every whole-trait use names its evidence ... There is no unique-visible,
  most-specific, ambient default, or attached-machine search"
  ([declaration and selection](../../../../wiki/spec/language/conformances.md#declaration-and-selection)).
  `Decider::pick(6)` names none, so it is not a call of the one visible
  implementation. "If only one implementation is intended, source can call
  that machine directly"
  ([generics](../../../../wiki/spec/language/generics.md#static-machine-binder-categories)):
  `Registrar::pick(6)`. A generic machine selects it through an evidence
  binder: `decide<RegistrarDecider>(7)` with `Policy::pick(value)` inside
  (`tests/omega/pass/traits/satisfies_bound_trait_parameter_exit`).

# erased_ref_arg

Erased static arguments keep their authored lexical scope: `&Element` as a
type argument inside a machine-call data bound proposition (`where value <= ceiling<&
Element>()`), an unused `const Tag: u8` forwarded through `relay`, and a
`Bounded<u8>` field whose bound folds away.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# affine_state_clone

Element-by-element clone through a cycle of `-> Cell` states: `each` walks
`position`, `store` writes one indexed member under
`requires position < 2`, and the false arm returns the finished copy via
`Worker::copy(copied)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

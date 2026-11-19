# bool_representation_views

First sample probing bool's {0,1} representation set:
`&flag as &u8` forgets it, `&byte as &bool` is established
by the byte's `where byte <= 1` fact, and `&mut byte as
&mut bool` is legal under set preservation — writes
through the alias land on the byte.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

# machine_param_data

Data family parameterized by a compile-time machine symbol: `data Stream<machine S> where machine S(index: Index) -> Index` — recursive `Stream<S>` occurrence forwards the static param; `boundary machine sample`/`alternate` are used as `Stream<sample>`/`Stream<alternate>` instantiations; contract checked at the concrete use.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

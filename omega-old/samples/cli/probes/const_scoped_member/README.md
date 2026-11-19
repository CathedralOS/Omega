# const_scoped_member

Data-scoped consts as static arguments: `const Sizes::SIZE: Index`/`Sizes::WIDTH` under `data Sizes {}` scope feed `Indexed<const Selected: Index>` — `keep(Indexed<Sizes::SIZE>)`/`keep2(Indexed<Sizes::WIDTH>)` keep canonical identity; `let Sizes: u64` local shadows the scope name inside a body without disturbing the type.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

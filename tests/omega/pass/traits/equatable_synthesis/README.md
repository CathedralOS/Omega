# equatable_synthesis

Empty `satisfies Equatable<T> {}` gets a synthesized structural `equals` row; `Equatable::equals = marks_share_x` fills a row by machine reference; evidence binder `Equality satisfies Equatable<T>` selects the same rows.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

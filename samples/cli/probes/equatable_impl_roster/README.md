# equatable_impl_roster

Named-implementation roster mechanics: two `Equatable<Coin>` impls coexist (`==` is ambiguous on Coin but `CoinByValue::equals`/`CoinStructural::equals` select exact rows); `Equatable::equals = marks_share_x` fills a row with an ordinary machine; `equal<T, Equality satisfies Equatable<T>>` generic evidence binder; `PlayerEquality` synthesized row recurses into `Pos` via PosEquality.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

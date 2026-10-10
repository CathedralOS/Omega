# domain_expr_canonical

Generic domain family with comptime-arg canonicalization: `domain<T,const N:u64> T::Indexed<N>` — `u64 in Indexed<SIZE>` ≡ `Indexed<SIZE+0>` ≡ `Indexed<1>` and `Indexed<SIZE+1>` ≡ `Indexed<2>` in param positions. GAP-adjacent learning: fields of the family inside Main can be READ and passed but a literal store into them cannot prove membership — the generic domain has no literal establishment rule.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

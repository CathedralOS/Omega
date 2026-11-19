# const_param_identities

Const-param canonical identities: `Buffer<SIZE>` ≡ `Buffer<7>` (computed const), `Buffer<MATCH_SIZE>` ≡ `Buffer<7>` (match expr in const init), `Flag<const Enabled: bool>` bool param (`Flag<ENABLED>` ≡ `Flag<true>`), `Indexed<const Selected: [u64;2]>` ARRAY const param.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

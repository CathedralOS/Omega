# nested_pair_endpoint

Both the rank and its ceiling are read through nested projections: the ranked field lives in `inner`, the limit in the sibling record `bounds` — `in 0..=countdown.bounds.limit`.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

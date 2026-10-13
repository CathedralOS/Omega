# projected_slice_length

A `Slice::Length` subject over projected storage: `bag.items` resolves to the one exact produced length coordinate rooted at the `bag` record formal, and `current.items[1..]` descends from `current.items`'s own length.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

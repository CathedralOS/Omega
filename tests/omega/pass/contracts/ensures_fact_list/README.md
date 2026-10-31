# ensures_fact_list

One `ensures` may list several facts — each is its own published guarantee: `bump` publishes `count > 0, count <= slots.len` and `release`'s requires list consumes them together (the exact subtraction cannot go below zero).

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

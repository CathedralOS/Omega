# member_to_state_param

An arm-bound record member moves into a state parameter: the `Shape::Line`
arm destructures `{ inner, tag }`, hands `inner` to `use_inner`, and a nested
leaf loans onward as `&inner.leaf` into `read_leaf(main, leaf: &Leaf)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

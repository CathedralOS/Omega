# cross_module_sum_ctors

Closed generic sum constructors across sibling modules: `first::Choice<u64>::Full{value}` vs same-spelled `second::Choice<u64>` stay distinct; `first::Packet<u64>` mixes common field `stamp:u8` + payload `value:T` in one construction `Full{value:11, stamp:2}`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# denotational_self_requirement

A trait requirement cites its sibling member denotationally: `machine law(input) requires value(input) == value(input) ensures value(input) == value(input)` — `value` is an ordinary trait member with `-> u8 in Saturating terminates`.


Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

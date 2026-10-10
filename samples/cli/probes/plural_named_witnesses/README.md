# plural_named_witnesses

Plural named proof lanes: three `proposition ... evidence Evidence` premises + `ensures first_out:`/`second_out:`/`third_out:` lanes; impl assigns each lane (`first_out = local_first`); caller destructure `let (; first_out: first, third_out: third) = Order::produce(value; incoming_first, ...)` — `(; ...)` keeps only named output lanes and `;` separates premise args.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

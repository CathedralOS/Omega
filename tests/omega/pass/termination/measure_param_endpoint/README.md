# measure_param_endpoint

A projected-measure rank range may name a parameter endpoint:
`in 0..=ceiling` uses the machine's own `ceiling` formal, repinned by
`requires 5 <= ceiling && ceiling <= 10`; a subordinate state carries
its own requires clause, and the record's data-where facts
(`0 <= remaining <= 5`) discharge the descent measure.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# Borrowed transparent alias custody

This imported alias has its own lifetime and Type telescope: `Borrowed<'region, T> = &'region T`. The generic `observe<'left, 'right, T>` keeps both input lifetimes distinct, interchanges the alias with an explicit reference under its lexical binder, and returns the left input. The caller infers the call result into an ordinary reference; it does not invent an undeclared alias lifetime. Reading 42 while the other input remains 17 makes wrong-input capture observable.

The two intended-invalid children change only the returned input or one supplied lifetime to an undeclared tag. Source checking, native/Terminal execution and the authored refusal fragments are pending actual observations. This is a focused lifetime/Type rung, not full binder, nine-place, Product or other-host acceptance.

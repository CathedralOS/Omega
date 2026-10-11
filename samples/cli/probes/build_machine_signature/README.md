# build_machine_signature

The build activation is one machine named `build` whose first parameter
is pinned to the spelling `builder: &mut Build`. The parameter NAME is
part of the contract — `context: &mut Build` rejects identically to a
missing parameter. A declared return type is accepted (this sample
returns `i32`; the value is discarded).

Controls pin the enforced shape:

- `invalid_param_name` / `invalid_param_missing` /
  `invalid_param_immutable` / `invalid_extra_param`: each rejects
  "package build machine's first parameter must be
  `builder: &mut Build`" — the extra-parameter case shows arity is
  enforced through the same first-parameter diagnostic.
- `invalid_no_kind`: a `build` machine calling no `Build::` kind
  rejects "build must declare exactly one kind".
- `invalid_no_build_machine`: a file declaring only `machine setup`
  produces the same "exactly one kind" diagnostic — the activation is
  the machine named `build`, not any machine that calls `Build::`.

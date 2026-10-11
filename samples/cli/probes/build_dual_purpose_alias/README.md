# build_dual_purpose_alias

One dependency directory reached by the same alias `kit` on both purposes:

- `Build::build_depend_as(builder, "kit", ...)` gives the build scope the
  dep's product machines (`use kit::main` inside `build.omg`).
- `Build::depend_as(builder, "kit", ...)` gives the product scope the same
  machines (`use kit::main` inside `main.omg`).

The dep stages `generated.omg` through the required-output custody flow;
its `value()` calls the generated `generated_value()` and must return 7.

The question pinned here: does one alias carrying both purposes reject as
a collision, or does the roster keep the two rows distinct per scope?

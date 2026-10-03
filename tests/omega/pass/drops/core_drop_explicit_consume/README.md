# Explicit disposal with erased evidence

Run `omega run --both tests/omega/pass/drops/core_drop_explicit_consume/main.omg`.
Both engines exit 0. The ordinary consuming `drop` call disposes of the Guard
and Carrier owners; Carrier retains Evidence in its semantic field declaration,
without constructing runtime Evidence storage or invoking Evidence::drop.

Prepare the project with `omega update --project tests/omega/pass/drops/core_drop_explicit_consume --target macos_arm64`, substituting the host target as needed.

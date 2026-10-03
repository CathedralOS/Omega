# Projected reference record with residual custody

`omega run --both tests/omega/pass/borrows/projected_reference_record_residual_exit/main.omg`
must exit 29 natively and in the checked interpreter. Moving `input.inner`
preserves its stored reference and leaves `input.spare` for ordinary exit
cleanup; the containing root cannot be retired as a second whole-value move.

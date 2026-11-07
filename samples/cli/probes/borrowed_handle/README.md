# Borrowed handle

An issued affine handle exposes a lifetime-bearing borrowed view of its
original storage. Observing and retiring that view permits the owner to be
consumed once; equal numeric fields cannot manufacture an issued view.

Run `omega run samples/cli/systems/borrowed_handle/main.omg`; expected exit 0.
Native and Terminal execution both exit 0 on macOS ARM64. This exercises the
portable filesystem adapter's handle-borrow prerequisite, not an OS call or
resource grant. Qualifications retain their checked evidence without changing
the borrowed field's storage or creating another owner.

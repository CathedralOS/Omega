# runtime_write_only_boundary_satisfier_exit

A `&write i32` call routed through `Sink::write`, a boundary-trait requirement.
`NativeSink::write` satisfies the requirement with a checked Omega body, so the
`satisfies` binding supplies the non-observation receipt: the caller's loan is
admitted, the store through the write-only view executes, and readback of the
stored `7` exits 0.

Expected exit: 0

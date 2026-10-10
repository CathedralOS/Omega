# boundary_len_discharge

First sample discharging a boundary byte-extent requirement
against caller storage: `requires count <= buffer.len` on
`ByteHost::read` proves through `&mut` slices, and
`pump(host: &mut Binding<ByteHost>, buffer: &mut [u8])`
re-proves the length across a second boundary.

Compile-checked only: `use omega::language::core::binding`
is depend-free; run a check target, not native.

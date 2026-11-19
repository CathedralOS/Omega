# boundary_len_discharge

First sample discharging a boundary byte-extent requirement
against caller storage: `requires count <= buffer.len` on
`ByteHost::read` proves through `&mut` slices, and
`pump(buffer: &mut [u8])`
re-proves the length across a second boundary.

Compile-checked only: calls name `ByteHost::read` directly. No provider
selection runs at `--check`; native execution needs a selected provider.

# boundary_len_discharge

First sample discharging a boundary byte-extent requirement
against caller storage: `requires count <= buffer.length` on
`ByteHost::read` proves through `&mutable` slices, and
`pump(buffer: &mutable [u8])`
re-proves the length across a second boundary.

Compile-checked only: calls name `ByteHost::read` directly. No provider
selection runs at `--check`; native execution needs a selected provider.

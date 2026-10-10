# Wire Domain Member

Compile-scope probe: wire `#N` tags combined with domain-qualified
borrowed-slice members. No standard-library dependency.

- `domain [u8]::Utf8 requires valid_utf8` — slice-carrier domain.
- `LabelMessage { #0 count: u32; #1 label: &[u8] in Utf8 }` — the
  generated `encode` emits the same length-varint + raw-bytes frame a
  plain `&[u8]` uses; the qualification rides the referee slice.
- Members fill memberwise on a declared local — each store
  establishes its descriptor.
- Frame for `{ count: 7, label: "hi" }`: era 0x00, tag0 0x00, 0x07,
  tag1 0x01, len 0x02, 'h' 'i' — 7 bytes.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/wire_domain_member/main.omg
mbx run -p omega -- update --project samples/cli/probes/wire_domain_member
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

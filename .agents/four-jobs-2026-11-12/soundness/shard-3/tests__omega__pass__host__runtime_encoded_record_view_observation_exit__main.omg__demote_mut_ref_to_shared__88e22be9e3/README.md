# Encoded scalar view at a foreign boundary

On macOS ARM64 and Linux x86-64/ARM64, run:

```sh
mbx run -p omega -- run tests/omega/pass/host/runtime_encoded_record_view_observation_exit/main.omg
```

The program writes `A` through the original signed-byte backing observed as
`u8`, then restores the byte image of -1. Its exit is zero. Ordinary helper
calls and a returned reference retain the source codec; the host snapshot
checks the original signed restriction in its own value domain. Thin native
export loads the original referent address, never the internal descriptor.
The shared `u8` loan has an invariant one-byte footprint. This does not admit
wide shared scalar loans whose backing could be narrower than their formal.

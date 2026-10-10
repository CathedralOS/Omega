# Wire Scalar Roundtrip

Compile-scope probe: `#N name: T` wire field tags and the generated
codec pair. No standard-library dependency.

- `data Frame { #0 code: u32; #1 level: i32; }` — numbered fields are
  the wire order; `FrameValue` is the un-tagged mirror data.
- `Frame::encode(&value, &mut buffer, &mut written)` and
  `Frame::decode(&mut value, &buffer, &mut read, &mut verdict)` are
  synthesized from the tags — no imports beyond the depend-free set.
- `data WireVerdict { case Invalid; case Sound; }` — `Invalid` is the
  ZII zero case so an untouched verdict reads as failure.
- Roundtrip proves the decoded fields read back; `written`/`read`
  count codec bytes.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/wire_scalar_roundtrip/main.omg
mbx run -p omega -- update --project samples/cli/probes/wire_scalar_roundtrip
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

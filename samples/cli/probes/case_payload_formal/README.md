# Case Payload Formal

Depend-free compile-scope probe: enum-of-enum payloads — the first sample
where a case payload carries another case type and is built from a
formal.

- `Res::Error` wraps a `Kind` — a case whose payload is itself a
  two-case enum, built directly from the parameter `kind` (the
  parameter-rooted consumption path, not a local).
- `Res::code` dispatches the nested shape: `Res::Error { kind }` binds
  the inner enum for the caller to inspect again.
- `kind_of` unwraps one more level — `Kind::NotFound` vs `Kind::Other` —
  proving the inner case survives construction, return, and re-dispatch.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_payload_formal/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_payload_formal
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

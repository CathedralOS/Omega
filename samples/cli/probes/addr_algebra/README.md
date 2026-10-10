# Address Algebra

Compile-scope probe: `addr` — a pointer-width address type distinct
from `u64` counts — and the legal half of its algebra. No
standard-library dependency.

- `let a: addr = 4096` — integer literals into addr fields/locals.
- `addr - count -> addr` — offset backward; `addr - addr -> u64` —
  distance between addresses; `addr < addr` — unsigned ordering.
- `a as i32 in Trapping` — addr to i32 conversion through the Trapping
  domain.
- The illegal half (`addr + addr`, `addr * x`, shifts) rejects at
  compile — fail-corpus territory, not exercised here.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/addr_algebra/main.omg
mbx run -p omega -- update --project samples/cli/probes/addr_algebra
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

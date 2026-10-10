# borrowed case read

Compile-scope probe. No standard-library dependency.

- `transition duo` on `&Duo` and `&mut Duo` receivers: borrowed
  subjects still dispatch cases and read payloads (`a`, `b + c`).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/borrowed_case_read/main.omg
mbx run -p omega -- update --project samples/cli/probes/borrowed_case_read
```

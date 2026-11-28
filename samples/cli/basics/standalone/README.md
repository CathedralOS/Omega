# Standalone benchmark subject

This sample measures the fixed cost of compiling an empty hosted application.
Its entry takes no arguments, returns normally, and writes nothing. The hosted
loader finishes the process with status zero. The build binds the entry
directly and has no package dependencies.

Run the sample gate on the target host to check the published executable and
the Psi interpreter against the same expected status.

```sh
mbx xtask sample-gate --filter samples/cli/basics/standalone
```

For compile-time measurements, the benchmark can still use `--no-run` to
measure only compilation. Earlier measurements that used that flag recorded
the runtime leg as skipped.

## Measuring compilation

Accept the root package once per target, then compile with `--timings`, which
prints one line per compiler phase and the published artifact:

```text
mbx exec target/debug/omega packages update --project samples/cli/basics/standalone --target linux_x86_64
# Edit the `pending` decisions in the review it names to `accept`, then:
mbx exec target/debug/omega packages update --resume --project samples/cli/basics/standalone
mbx exec target/debug/omega --timings --target linux_x86_64 samples/cli/basics/standalone/main.omg < /dev/null
```

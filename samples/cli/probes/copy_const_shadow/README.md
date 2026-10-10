# Copy Const Shadow

Depend-free compile-scope probe: `[copy]` data, top-level constants, and
name shadowing — the first sample declaring all three.

- `data Key [copy]` is copyable data: reads keep working after earlier
  reads and after the name is shadowed.
- `const KEY: Key` is a top-level constant of user data — read directly,
  read as a machine parameter spelling, and shadowed by `let KEY`.
- `local_key` shadows the constant in one let (the parameter wins the
  spelling afterwards); `later_key` reads the constant BEFORE shadowing
  and returns the saved read.

The machine reads the constant twice, shadows it twice, and accepts when
every read agrees on 1 (constant) or 9 (parameter). Compile-checked only
(no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/copy_const_shadow/main.omg
mbx run -p omega -- update --project samples/cli/probes/copy_const_shadow
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

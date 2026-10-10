# Equation-Binder Box

Compile-scope probe: structural type-equation binders in `where`
clauses. No standard-library dependency.

- `data Mode<const Inner: u32>` — const-generic data.
- `Box<Payload [copy], const Count: u32> where Payload == Mode<Count>`
  — the equation links parameters; the Payload slot must be exactly
  Mode<Count>.
- `capacity<Payload, const Count>() where Payload == Mode<Count>` —
  the equation exposes Count at the call; `capacity<Mode<4>>()` and
  `recovered<4>()` both partial-apply.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/equation_binder_box/main.omg
mbx run -p omega -- update --project samples/cli/probes/equation_binder_box
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

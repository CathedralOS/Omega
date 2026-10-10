# Domain Conjunction Pairs

Compile-scope probe: domain conjunctions, transparent aliases, and `in`
membership as a first-class value — including a tuple `transition`
subject with a literal-tuple pattern. No standard-library dependency.

- `domain Socket::Usable = Connected & Authenticated` is a transparent
  alias: `forward` declares the two atomic requirements and that
  satisfies `consume`'s `socket in Socket::Usable`.
- `Socket { connected: true, authenticated: true }` — record literal
  construction is the gate, granting both field domains at once.
- `(a in Usable, b in Usable, b in Connected)` is a tuple subject;
  `(true, false, true)` is the matching literal pattern.
- `data Socket [copy]` keeps `forward(main.socket)` a copy, not a move
  out of borrowed entry storage.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domain_conjunction_pairs/main.omg
mbx run -p omega -- update --project samples/cli/probes/domain_conjunction_pairs
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

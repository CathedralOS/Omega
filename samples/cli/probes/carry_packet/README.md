# Carry Packet

Depend-free compile-scope probe: explicit Carry property declarations —
the first sample marking aggregates with the closed `[carry(...)]`
property.

- `Packet` declares all four mandatory axes (`suspension`, `cpu`,
  `thread`, `address`) beside common fields and payload cases;
  `Envelope` nests a `Packet` through the same declared property —
  nested and case-payload structural validation both run.
- Carry is a closed compiler property: transparent aggregates derive it
  structurally; the explicit declaration pins the authored spelling.

The machine builds a Burst packet inside an envelope, matches the active
case's payload, and accepts on 3 && 2. Compile-checked only (no std
dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/carry_packet/main.omg
mbx run -p omega -- update --project samples/cli/probes/carry_packet
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

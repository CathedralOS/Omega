# alias fact transfer

Compile-scope fact. No standard-library dependency and no ProcessExit
provider. An entry `requires` on `main.player` transfers through a bare
`Player` alias, and that alias carries `Alive` into `inspect`'s `requires`.

The fact is checked at nine places. Each place is its own project. The
oracle is `omega check` plus a native compile: every place binds
`ProgramEntry` for the host targets and must compile, though these
programs declare no exit code.

- `local/` — `let mut copy: Player` carries the entry fact into `inspect`.
- `field/` — the alias stored in a field still carries `Alive`.
- `parameter/` — a parameter alias carries `Alive` into the callee body.
- `call_result/` — a call result alias carries `Alive` onto a bare local.
- `match_join/` — both arms carry `Alive`, so the joined alias does.
- `const_name/` — a const name of an `Alive` player carries the fact.
- `other_state/` — another state's parameter receives the alias and its fact.
- `other_machine/` — one machine returns the alias and another requires it.
- `target_body/` — every target body uses the transferred fact.

Each place has `controls/invalid_alias`. That control must reject because
the alias does not carry `Alive`.

```sh
mbx run -p omega -- --check samples/cli/facts/alias_fact_transfer/local/main.omg
```

Repeat `--check` for `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`.

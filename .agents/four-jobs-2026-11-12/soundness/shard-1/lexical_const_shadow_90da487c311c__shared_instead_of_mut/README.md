# lexical_const_shadow

A `const` aggregate value may be shadowed lexically at every binder depth: const `KEY`, param `KEY: Key`, and `let KEY` each resolve to their own stage.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

## Gaps found while authoring (filed)

- Corpus fixture `constants/lexical_aggregate_values` fails `--check`
  on main today: its sibling `consumer.omg` module cannot see main's
  `Key`/`KEY` — `use consumer` imports consumer's names into main,
  not the reverse (fixture carries a `regressed-from` pin).

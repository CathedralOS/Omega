# mixed_record_cases

A mixed record carries common fields beside its cases: `token.start`
reads like a record field on every case, while a payload field is
observed through its case step — both on elements copied out of a
borrowed array into a local.

## Learnings

- Predicate arms (`Token::Mark { kind: Kind::A }`) do not count for
  exhaustiveness — an uncounted-arm transition needs a `_`.
- `stream.tokens[at]` needs `requires at < N`; index arithmetic is not
  discharged by element width.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

# Operator Desugars

Depend-free compile-scope probe: the ordinary operator surface — the first
sample exercising compound assignment, bitwise, shift, and unary
operators in one program.

- `v += 5; v -= 3; v *= 2; v /= 4; v %= 4` each desugars to
  `v = v OP rhs`; chaining in one state forces every later read to
  observe the prior write.
- `&`, `|`, `^` produce integers (not booleans) and carry no arithmetic
  domain obligation; `2u64 << 8` shifts on the annotated type; `-v`
  negates.

The machine walks v through the compound chain to 2, checks the bitwise
results (8, 14, 6), the negation (-10), and the shift (512).
Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/operator_desugars/main.omg
mbx run -p omega -- update --project samples/cli/probes/operator_desugars
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

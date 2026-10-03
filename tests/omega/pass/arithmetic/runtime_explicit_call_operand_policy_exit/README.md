# Explicit call-operand carriers

Call results retain their integer carrier and arithmetic policy. This program
explicitly widens a `u16` result and qualifies an Exact `u32` result before
combining them with Wrapping operands. Its shift count keeps an independent
type and policy. Correct native and interpreted execution both exit `70`.

From the repository root:

```sh
python3 tools/corpus_gate.py --both --filter runtime_explicit_call_operand_policy_exit
cargo run -p omega -- run tests/omega/pass/arithmetic/runtime_explicit_call_operand_policy_exit/main.omg
```

The public CLI command requires ordinary package-review acceptance for this
project. A fresh unapproved checkout stops at that review, not at the arithmetic
check. The corpus's fixture setup does not record user acceptance for the CLI.
The companion fail fixture `implicit_call_operand_conversion_rejected` checks
that unconverted call operands reject before artifact production.

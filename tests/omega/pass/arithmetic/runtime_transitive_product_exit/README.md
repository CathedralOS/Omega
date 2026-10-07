# Conditional ordered bounds survive a discarded intermediary

Run on a matching host:

```sh
cargo run -p omega -- run tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg
```

A saved Boolean observes `-10 <= floor < value < ceiling <= 10`. Only the
Boolean and `value` cross the first state edge; selecting the Boolean later
must retain both derived endpoints without replaying its initializer.
Native execution exits 0 after checking the signed products -18 and 18 and
the refused value 100. This exercises the shared relation projection through
conditional facts, independently of shift-count checking.

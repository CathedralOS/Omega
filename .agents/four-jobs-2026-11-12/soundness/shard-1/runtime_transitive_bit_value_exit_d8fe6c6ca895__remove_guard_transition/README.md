# Ordered bounds survive a discarded intermediary

Run on a matching host:

```sh
cargo run -p omega -- run tests/omega/pass/arithmetic/runtime_transitive_bit_value_exit/main.omg
```

The selected guard proves `count < ceiling <= 64`. The next state receives
only `count`, so checking must retain the derived `count < 64` rather than
dropping it with `ceiling`. Native execution exits 0 after checking bit 3,
the unsigned high bit, and the refused count 64. Count validity and Exact
result representability remain separate obligations.

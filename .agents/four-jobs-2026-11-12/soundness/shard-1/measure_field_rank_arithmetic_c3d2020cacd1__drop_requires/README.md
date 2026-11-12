# Arithmetic field-rank endpoint

The exclusive ceiling combines two invocation inputs. Their declared bounds
prove the sum exceeds the field's maximum rank, and both inputs remain fixed
on every loop edge. Endpoint arithmetic uses selected builtin Exact semantics.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_rank_arithmetic/main.omg
```

The [replacement control](../../../fail/termination/measure_field_rank_arithmetic_replaced/main.omg)
rejects when only the padding input changes, even though the sum remains large
enough. This exercises source checking, not native custom-measure certificates.

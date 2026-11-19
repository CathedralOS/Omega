# Stage vector match

Selects one three-species concentration triple from a reaction order. Each
match arm is a call returning a fresh `[f64; 3]`. The selected array moves
through a local, a state parameter, a field store, and a call argument.

```sh
omega run samples/cli/independent/stage_vector_match/main.omg < /dev/null
```

Exit 0 prints `stage vector 9` (the order-1 triple `2 + 3 + 4`).

# Principal component row

A principal-component observation row stores four features. A runtime choice
selects one pair, centers its leading feature by a supplied mean, and returns
both the mutable pair and the centered value.

The pair length is observed through a local, a record field, a parameter, and
the call that returns the pair. Pair 0 is the first two features; pair 1 is
the last two. Each call centers the pair's leading feature by the supplied mean.

```text
// Expected exit: 0
// Expected output contains: row score
```

# Bootstrap runner

`verify-lattice.sh` is optional POSIX-shell orchestration for the direct
compiler sequence. It is not a compiler stage, evidence producer, source
resolver, or acceptance authority. Every invoked gate is independently
executable, and the runner prints the exact command before execution and again
on failure.

```sh
sh tools/bootstrap/verify-lattice.sh
```

The runner contains only bounded construction and admission gates. Expensive
differential, mutation, fuzz, proof-corpus, and developer-tool campaigns remain
independently executable diagnostics under their owning rung; there is no
repository-wide stress orchestration layer.

`paths.sh` only maps semantic-owner roles to repository locations. Shell and
Python helpers may coordinate or test commands, but no bootstrap claim depends
on this runner, its working directory, or transformations performed by it.

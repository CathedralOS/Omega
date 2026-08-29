# Lattice runner

`verify-lattice.sh` is optional POSIX-shell orchestration for the direct
compiler sequence. It is not a compiler stage, evidence producer, source
resolver, or acceptance authority. Every invoked gate is independently
executable, and the runner prints the exact command before execution and again
on failure.

```sh
sh tools/lattice/verify-lattice.sh
```

The runner contains only the presently closed floor: selected Alpha seed
behavior plus exact assembler construction and construction of the below-Beta
checker. The current `bc.beta` fixed point, generic artifact-framing gate,
Gamma program suites, path-policy test, and stress tools remain directly
runnable diagnostics; repeating them as top-level rows would falsely present
an open compiler edge as closed.

Native Alpha source/container reproduction remains available through
`source/alpha/verify.sh` without `--edge`. It is a supply-chain diagnostic and
irreducible seed-admission aid, not a semantic premise repeated by the direct
compiler-edge runner.

As later artifacts become available, this runner should invoke their actual
producer/admission commands. It must not substitute a verifier's fixture suite
or a source snapshot for the missing artifact.

Reusable Gamma interpreter/type-checker suites remain locally testable, but are
not compiler edges and therefore are not default lattice steps. The future
Beta-written Gamma compiler owns the actual canonical Gamma-to-tape invocation.

`paths.sh` only maps semantic-owner roles to repository locations. Shell and
Python helpers may coordinate or test commands, but no lattice claim depends
on this runner, its working directory, or transformations performed by it.

The retired root `compiler/` cache, `.lattice-cache` receipt profiles, and their
ignore rules are gone. A role path remains only when it names a current owner or
an explicitly tracked migration component.

# Lattice runner

`verify-lattice.sh` is optional POSIX-shell orchestration for the direct
compiler sequence. It is not a compiler stage, evidence producer, source
resolver, or acceptance authority. Every invoked gate is independently
executable, and the runner prints the exact command before execution and again
on failure.

```sh
sh tools/lattice/verify-lattice.sh
```

The runner contains only the presently closed producer spine: selected Alpha
seed behavior plus exact assembler construction, construction of the
below-Beta checker, non-mutating Alpha-rooted reconstruction of `beta_compiler_bytecode.tape`,
and the one canonical `bc` admission. The standalone assembler self-host,
artifact-framing gate, Gamma program suites, path-policy test, source-closure
test, and Delta publication fixtures remain directly runnable diagnostics or
subchecks; repeating them as top-level rows would not add a compiler edge.

Native Alpha source/container reproduction remains available through
`source/alpha/verify.sh` without `--edge`. It is a supply-chain diagnostic and
irreducible seed-admission aid, not a semantic premise repeated by the direct
compiler-edge runner.

As later artifacts become available, this runner should invoke their actual
producer/admission commands. It must not substitute a verifier's fixture suite
or a source snapshot for the missing artifact.

Reusable Gamma program suites such as canonical-byte decoding remain locally
testable, but are not compiler edges and therefore are not default lattice
steps. The Delta publication owns the actual canonical Gamma invocation.

`paths.sh` only maps semantic-owner roles to repository locations. Shell and
Python helpers may coordinate or test commands, but no lattice claim depends
on this runner, its working directory, or transformations performed by it.

The retired root `compiler/` cache, `.lattice-cache` receipt profiles, and their
ignore rules are gone. The remaining roles and Delta manifests each have a live
gate consumer; they are subject identities or invocation paths, not historical
aliases or hidden bridge stages.

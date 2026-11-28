# Rebuilding an edited receiver with an explicitly published dependency

The experiment asks whether an application can change its own source and consume the same checked independent provider through the native compiler. The shipped CLI provides the reference build. The test host supplies the optional artifact through the compiler's existing `ComponentSuppliers` API.

## Run

Use benchmark base `e6477bc08c36684b78cca182ba91bd012b4c5a4d` for the release compiler. The experiment changes an integration test and tools. It leaves the compiler and the filesystem fixture at that revision.

```sh
mbx build --locked --release -p omega
mbx test --locked --release -p omega --test prechecked_dependency -- --nocapture
mbx xtask measure-public-values target/release/omega /absolute/path/to/new-results
```

Run on Linux x86-64 and supply a new output directory. The command times the entire compiler process and records argv, exit status, binary and input hashes, ELF machine and format, and result paths. It launches each executable in a private empty directory and checks exit status, streams and cleanup. Each of the three cold trials uses the original `public_values/build.omg`, an empty resolver cache and a new output directory. This defines compiler/dependency coldness independently of the OS page cache.

The edited trials use an independent source copy with the same relative dependency declaration. A preparation build uses a separate resolver cache. Each edited trial changes the copy destination used throughout the root program and builds into a new output directory with that same resolver cache. File counts record what the resolver actually retained. The separate profiling run enables `--timings` and uses another empty cache.

## What crosses the boundary

The test's provider comes from `tests/fixtures/synchronization/run/independent_scalar_results/supplier`. The root uses an ordinary package registration, explicit validation and an independent provider selection. The first invocation performs a separately timed extra publication. The test host writes its Psi bytes to a named file and retains the producing compiler's exact artifact pin. A new resolver traversal and Build execution prepare the edited receiver. The host compares the supplier occurrence before opening the explicit file and passing it through the receiving host's Psi authorization and ordinary native provider checks.

The prototype is limited to one local Product dependency with an empty child graph, explicit validation, a fixed compiler process, and an unprivileged invocation. The test retains typed provider-demand and Build evidence in the host. A production CLI consumer still needs an explicit carrier for that evidence. The release filesystem fixture imports a fused source library. Independent provider reuse and the cold filesystem target are separate results.

The input and acceptance rules are in [compiler storage](../../../wiki/spec/build/compiler_storage.md), [package acceptance](../../../wiki/spec/packages/acceptance.md), [resolver security](../../../wiki/spec/packages/resolver_security.md) and [receiving Psi admission](../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption).

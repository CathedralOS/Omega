# Assigning service reaches in declaration order

Completing one machine body in a 4,096-machine package took 5.39 ms, down from 11.61 ms. With just one machine, both resolvers took 19.83 µs. During reach assignment the original resolver searched the pending reaches for each machine, then searched the results to publish each row. The changed resolver follows the positions already computed when trait defaults are selected. Each surviving machine gets its own row in a single traversal.

## Resolver operations

The scaling input defines `main` and enough independent, constant-returning machines to reach the stated count. Every body sample requests only `main` from an already-open package. The smaller checked-in examples exercise cross-file overloads, a generic array extent, and explicit boundary-service reaches. The [stage guide](../../../omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/README.md) describes the operation boundaries.

Times are median microseconds per operation. The paired change is the median percentage difference within each pair. The last column is its 95% bootstrap interval. Lower times are faster.

| Operation | Input | Batch | Before µs | After µs | Paired change | 95% interval |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| body | tiny | 1,949 | 19.83 | 19.83 | -0.61% | -5.42% to +4.82% |
| body | machines-64 | 696 | 80.99 | 79.30 | -3.17% | -5.21% to -1.69% |
| body | machines-256 | 137 | 322.22 | 280.26 | -13.00% | -14.72% to -10.03% |
| body | machines-1024 | 36 | 1,487.48 | 1,262.41 | -14.68% | -18.85% to -13.41% |
| body | machines-4096 | 4 | 11,608.96 | 5,387.51 | -54.03% | -55.15% to -50.01% |
| body | imports | 164 | 292.73 | 289.76 | -2.17% | -5.11% to +2.83% |
| body | generics | 301 | 203.05 | 201.68 | -2.18% | -3.91% to +0.47% |
| body | reach | 239 | 192.33 | 197.62 | +2.33% | -3.66% to +5.01% |
| full | machines-1024 | 8 | 6,809.52 | 6,320.90 | -7.54% | -8.75% to -4.59% |
| deferred | machines-1024 | 13 | 4,384.16 | 4,188.85 | -4.97% | -8.11% to -2.93% |
| full | imports | 99 | 638.75 | 629.17 | -2.92% | -4.72% to -0.04% |
| extension | reach | 370 | 124.41 | 125.23 | -1.58% | -4.64% to +5.41% |

All 12 workloads produced matching fingerprints, including machine identities, service definitions, row membership, authored reach owners and targets, and pending bodies. Completing `main` showed a size-dependent gain. Resolving the entire 1,024-machine package improved by 7.5%, and indexing its declarations for deferred completion improved by 5.0%. The intervals for the tiny, generic and explicit-reach body cases span zero.

## Whole native build

The [two-file overload fixture](../../../tests/fixtures/pass/calls/ordinary_family_binding/main.omg) imports two overloads, calls both and checks their results. Each CLI invocation built a Linux x86-64 executable in a fresh output directory. One warmup pair preceded eight measured pairs with alternating order. The timer for the whole build covers process startup through exit. The resolver timer is the compiler’s own `Psi 02` entry. Each produced executable ran afterward and exited 0. All 18 executables had the same SHA-256 digest, `1dcf24c330f4c7fa9fbad22d5f79208a471ae04579e2dfb3da7982b945279a05`.

| Measurement | Before s | After s | Paired change | 95% interval |
| --- | ---: | ---: | ---: | ---: |
| Resolver in the native build | 1.147 | 1.137 | -2.56% | -7.67% to +4.01% |
| Complete native build | 29.017 | 28.839 | -0.61% | -1.98% to +0.06% |

On the baseline, the reported resolver time accounted for about 4% of wall time on this package. The whole-build interval spans zero, so these runs do not establish an end-to-end speedup.

## Reproduce

Use benchmark-only revision `1dec6027373` for the original resolver and `5463b0e51fb` for the changed resolver. The benchmarks used the pinned Rust nightly on Linux x86-64, on logical CPU 2 of a shared Intel Xeon Platinum 8573C. Rust builds had finished before the reported measurements. Run the following in each checkout and retain both versions of the executables.

```sh
mbx bench -p syntax-trees-to-symbol-resolved-trees --bench resolver_timing --no-run
mbx build -p omega
```

For each resolver row, pass the operation, input and batch size to each benchmark binary. Run `omega-bench compare` with a `reported_work` plan, two warmup pairs and sixteen alternating measured pairs. Use the checksum printed by the baseline and the batch size as the expected work units. The harness times only resolution. It prepares and clones requests before timing, then fingerprints and drops the results afterward.

For the native build, use the optimized development-profile CLI from each revision. From the fixture directory, run the following with each binary, using the same toolchain library and private package cache, and a fresh build directory for every invocation. Record process wall time and the emitted resolver time. The comparison covers one host and one native workload.

```sh
omega --timings --target linux_x86_64 --build-dir "$output" \
  --build-input build.omg --build-input main.omg --build-input kit.omg main.omg
"$output/omega-program"
```

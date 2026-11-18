# Timing the Rust name resolver before and after reorganizing its source

The same stage-local benchmark ran on the original resolver at `b49ad4267f2` and the reorganized resolver at `5281cc7c6ef`. The original compiler revision was `8b6cf05b8ba`. The benchmark-only commit makes both revisions directly buildable. All twelve workloads matched their original output fingerprints across all pairs.

## Results

Times are median microseconds per resolver operation. The paired change and its 95% bootstrap interval compare the two binaries within each pair. A positive value means the reorganized version took longer. Each workload has two warmup pairs and sixteen measured pairs in alternating order. The batch size was calibrated on the baseline to target about 60 milliseconds of resolver work.

| Operation | Input | Operations per sample | Before µs | After µs | Paired change | 95% interval | Checksum |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| full | tiny | 559 | 109.0 | 112.8 | +2.8% | -0.1% to +5.3% | `1637349209395619623` |
| full | imports | 102 | 625.1 | 625.3 | +0.8% | -0.6% to +3.9% | `16764712453741141031` |
| full | generics | 101 | 500.0 | 506.8 | +1.4% | -0.8% to +3.7% | `16166752878096538172` |
| deferred | tiny | 626 | 91.2 | 92.0 | +0.1% | -2.6% to +3.5% | `613420627920911524` |
| deferred | imports | 217 | 280.9 | 288.1 | +1.9% | -0.3% to +5.5% | `13636093942684839317` |
| deferred | generics | 272 | 229.7 | 231.3 | +0.9% | -1.6% to +2.9% | `15720211938782243697` |
| body | tiny | 2943 | 19.0 | 19.2 | -0.1% | -1.8% to +1.1% | `1637349209395619623` |
| body | imports | 234 | 273.2 | 270.5 | +0.3% | -3.7% to +4.7% | `8731963396977771950` |
| body | generics | 363 | 215.4 | 199.5 | -1.4% | -19.5% to +5.2% | `2683308265655785411` |
| extension | tiny | 814 | 82.4 | 86.1 | +2.3% | -0.2% to +4.2% | `2353157195290221553` |
| extension | imports | 542 | 118.5 | 121.9 | +4.0% | +0.9% to +5.1% | `14466639072263171459` |
| extension | generics | 366 | 179.5 | 182.4 | +1.9% | -1.4% to +2.6% | `7401312519720794906` |

Eleven intervals include zero. Extending the imports package measured 4.0% slower in the initial comparison. A second comparison of the same binaries used three warmup pairs, 32 measured pairs, and 1,200 operations per sample. The paired change was -0.1% (95% interval -1.4% to +0.5%). The initial slowdown did not reproduce.

The tiny input has one machine. The imports input calls two overloads from another file and declares nested machines. The generics input uses a constant parameter as an array extent. Each body sample requests `main` from an already-open package. Each extension sample appends one machine to an existing package. Preparing the request, parsing, inspecting the output and dropping the returned trees happen outside the timer. The output check covers machine IDs and source spans, selected calls, authored declaration selections, and pending body IDs. For extensions it also checks the retained machine IDs after the public join. Resolver and source-compiler tests exercise refused programs and their diagnostics.

## Reproduce the comparison

Build each revision with the pinned Rust toolchain and keep its benchmark executable. Running the same binary with `body imports 100` prints the elapsed resolver time, number of operations and output checksum. The input sources are compiled into the benchmark.

```sh
mbx bench -p syntax-trees-to-symbol-resolved-trees --bench resolver_timing --no-run
mbx build --release -p omega-bench
```

For each row, run `omega-bench compare PLAN OUTPUT` with the two saved binaries, arguments `OPERATION INPUT OPERATIONS`, two warmups and sixteen pairs. Use the `reported_work` measurement with the checksum and number of operations recorded for that row. Declare the harness and three fixture files below as inputs. The runner validates their digests, ordering and checksums. The raw observations and execution order are archived in the task workspace at `/workspace/scratch/psi-resolver-benchmarks/pairs.csv` (SHA-256 `78cb1bbfe12ab40162e6939b71b58962c803bba650e9bea275e8fa25b7ae09a1`). The `paired/` directory beside it retains the plans, runner journals, reports and binaries. The [stage guide](../../../omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/README.md) explains each operation.

Both binaries ran on logical CPU 2 of a shared Intel Xeon Platinum 8573C Linux x86-64 host with a four-CPU quota and one resolver worker. The test and build processes for this comparison finished before timing. Each benchmark process warms up its resolver once before timing its batch. Rust was `1.100.0-nightly (a69a63265 2026-09-03)`. Whole-build latency and larger packages need separate measurements.

| Measured artifact or input | SHA-256 |
| --- | --- |
| Before binary | `dea8f783d5c9e53a26ff14888a70cff24ebd1f9697b86f7d1851572c27c05775` |
| After binary | `bff745f730d0f9e71aca945446a4ee3bcc0a23deeb3a2fb967438d909e6518f9` |
| omega-bench | `00a201aab343d90a5fd3f91ee1636a627b8cd55cbd45d0333aebb533749f888f` |
| resolver_timing.rs | `d6d0a768b6f92e6949f52bc6211c5677371dd2b40fc5e8d6eabdf3f017a653cb` |
| pass/calls/ordinary_family_binding/kit.omg | `2358e86d408e64036bd83740dedee9173c95cb5c2cd6ca729786fa3f6538bd58` |
| pass/calls/ordinary_family_binding/main.omg | `529767d2c5cadd63bdf2e00cd0b4a91e79eb4acacb34ebe2aebaf3b4478ba9a1` |
| run/generics/nominal_const_extent/main.omg | `fb45d603775647b8d688c7a908b6c0c5833c9e93d4acb7d522b805b88d57c3bd` |

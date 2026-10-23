# Inline common integer spelling

Measured baseline `d7259534785f3a07d1d07bb13ad104d7870f54f6`. Integer literals retain canonical radix/sign identity and landing metadata. Magnitudes fitting u64 now live inline; larger authored values retain canonical text with no language width limit. Numeric consumers read the stored magnitude instead of reparsing text. Decimal-only consumers retain their former admission window. Export and diagnostic spelling is reconstructed explicitly through bounded temporary storage; this cost is included in consumer assessment.

## Ownership and size

IntegerLiteral and Option<IntegerLiteral> shrink from 24 to 16 bytes. Syntax expression rows remain 112 bytes and resolved rows remain 128 bytes: their other alternatives still determine their size. Typed expression rows shrink from 704 to 672 bytes; syntax/resolved/typed static arguments shrink from 152/160/112 to 144/152/104 bytes. These are measured type sizes, not a claim about total typed-forest memory. The 1,792-source census contains 21,104 integer rows, of which 21,094 fit a u64 magnitude. Arbitrarily large inputs are separately covered by primitive controls.

Complete normalized stored rows, snapshot text, diagnostics and retained source custody match for every listed input, including refused inputs. Sixteen malformed partial forests also match. The primitive production implementation matches all 412 baseline boundary observations, including radix identity, landing, sign, conversion windows, large magnitudes and late invalid digits.

| Corpus (sources) | Allocation requests | Cumulative requested bytes | Live requested bytes after parse |
| --- | ---: | ---: | ---: |
| syntax (463) | 172,180 → 151,795 | 59,288,595 → 59,042,631 | 28,770,528 → 28,607,464 |
| lookahead (1792) | 611,947 → 545,364 | 198,037,109 → 197,257,310 | 97,459,298 → 96,946,554 |
| acknowledgement (31) | 7,169 → 6,277 | 1,778,956 → 1,768,979 | 879,318 → 872,214 |

Complete assembly of 289 projects is exact in both unique and shared source ownership modes. Each mode removes 480,765 allocation requests and 5,463,414 cumulative requested bytes; no individual project increases either measure. Modes are separate, not additive. These allocator observations include reallocations and are not RSS, peak memory, or CPU timing. The raw full-forest observations and per-project records are retained.

## CPU and consumer evidence

ARM64 inspection confirms the common from_parts branch stores fields without allocation; overflow branches to the retained textual owner after validating all digits. from_value has 13 instructions and no calls; value_i64 has 18 and no calls. Baseline from_value has 188 disassembly lines and baseline value_i64 has 130. Function length is explanatory evidence, not a timing result. The frozen binaries, disassembly hashes and exact timing-window call/drop sites are in assembly-audit.md.

Parser and fused timings use four alternating pairs with a three-second no-new-minimum window and a 15-second cap. Preparation is outside the window; real parsing/publication and destruction are inside. Initial compiler timings measure complete separate CLI invocations on fixed producers, after warm-up, in four alternating blocks of five invocations per side. The bounded compiler confirmation uses twenty alternating pairs of one invocation per side to reduce drift. Per-invocation child user/system CPU medians are retained in results.json. Independent-unit confirmation repeats the same frozen probe without source changes. Diagnostic sampling of three candidate compiles observed canonical integer spelling in only one stack sample; this does not establish zero cost, but did not support the proposed multi-percent formatter attribution. Full stacks and interpretation remain in the archive. The shared Apple M4 host has other work and memory pressure; raw samples and process resources remain recorded. Whole-process resources are not interval fault counts.

### timing

| Workload / producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse-pages / baseline | 5.837041 | 11.150208 | 12.049081 | 45.545583 | 1289 |
| parse-pages / candidate | 5.592875 | 10.911000 | 11.950878 | 55.420417 | 1793 |
| append / baseline | 8.577292 | 14.155083 | 15.402454 | 70.261667 | 1502 |
| append / candidate | 8.506042 | 14.097729 | 15.794341 | 291.786583 | 1412 |
| units / baseline | 9.135583 | 17.452250 | 20.136744 | 168.072500 | 1193 |
| units / candidate | 9.681500 | 19.225250 | 22.452181 | 181.997958 | 812 |

### compiler-timing

| Workload / producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| check-length / baseline | 1828.253458 | 1983.035042 | 1999.736806 | 2167.939875 | 20 |
| check-length / candidate | 1824.604209 | 1975.946354 | 1969.180188 | 2088.079417 | 20 |
| compile-length / baseline | 1890.658292 | 2019.778792 | 2067.314871 | 2461.480750 | 20 |
| compile-length / candidate | 1936.591292 | 2067.683501 | 2072.066419 | 2223.512250 | 20 |
| compile-integer-constant / baseline | 1482.025042 | 2007.740938 | 1946.468713 | 2288.016250 | 20 |
| compile-integer-constant / candidate | 1562.291250 | 2065.355750 | 2105.393742 | 2757.982750 | 20 |

### units-confirmation

| Workload / producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| units / baseline | 8.515458 | 13.983041 | 15.297378 | 126.302083 | 1313 |
| units / candidate | 8.277584 | 13.027875 | 14.262205 | 66.873500 | 1406 |

### compiler-confirmation

| Workload / producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| compile-length / baseline | 1224.473500 | 1749.350750 | 1814.379585 | 2514.358792 | 20 |
| compile-length / candidate | 1360.796750 | 1749.192749 | 1818.597675 | 2336.655375 | 20 |
| compile-integer-constant / baseline | 1439.798042 | 2087.984646 | 2022.000983 | 2367.795000 | 20 |
| compile-integer-constant / candidate | 1336.437042 | 2101.110604 | 2057.093806 | 2488.056292 | 20 |

Retain for the demonstrated ownership reduction and narrower representation, with no whole-compiler CPU speedup claim. Initial parse-page and append timings are slightly lower overall, while the initial independent-unit run is higher; unchanged-binary confirmation reverses that independent-unit result (median 13.983→13.028 ms, minima 8.515→8.278 ms), so it is not a reproducible regression. Initial full-compile medians are about2–3% higher; per-invocation alternating confirmation gives length compile median1749.351→1749.193 ms and integer-constant compile2087.985→2101.111 ms. Corresponding user-CPU medians differ by+0.59% and+0.09%, with large variation in individual pairs. This evidence does not establish a stable CPU benefit or a material repeatable consumer regression. All observations, including slow runs and tails, remain retained. Three diagnostic samples did not support attributing the initial full-compile difference to integer formatting. The allocation removal and exact behavior comparisons are the acceptance basis; CPU effects remain uncertain on this shared host.

## Correctness and integration

The full CLI all-targets check passes. Numerics and the syntax/resolved/typed representation suites pass 142 tests. The parser suite has 271 passes and seven failures already present on the fresh baseline, with exact normalized failure diagnostics retained; these are not relabeled passes. Scoped Clippy succeeds. Nine diagnostics mention changed files but their source fragments are unchanged on the baseline; none is in the new literal implementation. No lint suppression was introduced.

Eight integer compiler controls are frozen from the baseline: two check accepts, two programs agreeing at exit 0 natively and in Terminal Psi, three intended range/fractional refusals, and one pre-existing Terminal VerifiedOperationMalformed failure whose native execution exits 0. That last control is a preserved failure, not passing coverage. The separate nested-length and unchanged length/local programs must agree at exit 0, and the invalid count must still refuse at its required fact.

Current engine observations use optimized CLI producer `7c0ec3217be6b2f0aaa9663084472ecf446504c1` against integrated main base `4c23cc3391c2e539d4065563ac95de7b18f2fc67`. Complete declared compiler source objects match the integrated candidate exactly; reused executables retain original build receipts and identities. Nested-length and unchanged length/local agree natively and in Terminal Psi at exit 0, and the invalid count refuses at its required fact. All eight integer controls match frozen baseline output exactly, including the explicitly retained Terminal failure. Integrated owner tests and scoped Clippy pass; all 277 selected parser tests complete with their exact baseline outcomes, including six retained failures; the sole excluded sample-inventory policy failure remains archived separately. Pinned qualification captures these actual observations. Historical allocation/timing evidence remains bound to its measured baseline, not relabeled as integrated timing.


## Replay and remaining design

The working experiment is build/integer-representation. setup.py pins inputs, begin.py captures checks, measure.py compares complete forests and assembly, validate_primitive.py exercises the production owner, inspect_code.py extracts current generated code, timing.py runs the parser/publication repetitions, and build_candidate.py captures the optimized CLI and its consumer timing. Integration preserves historical measurement identities, qualifies the actual integrated producer and uses ordinary verify/enqueue/publish. Full evidence is archived under the primary checkout's build/measurement-archives/integer-representation-2026-10-22 after publication. One-shot mutation scripts require their declared baseline; do not rerun them over the selected implementation.

Layer 0 remains open: page-adoption header movement, generated-name ownership, expression/static-argument geometry, constructor paths, grammar retries, source-root associations and source/path buffers still have concrete design questions. This report establishes one storage improvement, not absolute saturation.

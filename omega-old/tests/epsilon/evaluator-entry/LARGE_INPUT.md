# Running the evaluator on the whole Omega compiler source

The frozen bootstrap compiler reported unsupported source at coordinate 895 on the full 4,651,287-byte request when its evaluator used an 8 MiB input limit and a private collecting Gamma runtime. With ordinary allocation the same cursor evaluator hit an 8 GiB resident-memory guard. The published 4 MiB input adapter refused the request before the bootstrap compiler read the product source.

| Epsilon evaluator | Input limit | Process outcome | Elapsed seconds | Peak RSS (KiB) |
| --- | ---: | --- | ---: | ---: |
| Eager stdin copy | 4 MiB | status 253 | 0.957 | 9,216 |
| Eager stdin copy | 8 MiB | supervisor `SIGKILL` | 229.743 | 8,392,612 |
| Sealed stdin cursor | 4 MiB | status 253 | 0.900 | 11,264 |
| Sealed stdin cursor | 8 MiB | supervisor `SIGKILL` | 240.553 | 8,391,356 |

All four processes left stdout and stderr empty. Status 253 is the adapter's input refusal. Both 8 MiB processes were still running when the supervisor stopped them. Timings and peak RSS describe one run per profile on a shared Linux x86-64 host. The output limit was 4 MiB throughout. A separate Delta program appended one byte to its input. Both adapters published exactly 4,194,304 bytes and refused a 4,194,305-byte result with raw status 254 and empty output.

A one-word change distinguishes the outer refusal from the running bootstrap compiler. Replacing the product's first little-endian word (package count 5) with 65 keeps the entire 4,651,287-byte envelope. The eager evaluator at 4 MiB still returned 253. At 8 MiB it returned raw status 0 and `0003000000`, the bootstrap compiler's Exit 3 for malformed framing, in 19.718 seconds at 833,060 KiB.

## Reproduce

Use `b95769558667b7a88654cb42d6673d08811ca4c4` for the native Gamma compiler, Delta compiler, bootstrap compiler and product sources. Use that revision for the eager evaluator and `720b1a10712feb649d13b7918a944d5c43508191` (PR #4166) for the cursor evaluator. The benchmark only varies the evaluator source and its input limit. Run from a checkout containing this benchmark, with Linux x86-64, Python 3, and room for an 8 GiB child plus the rest of the system.

```sh
bench=$PWD/tests/epsilon/evaluator-entry/large_input.py
scratch=$(mktemp -d)
mkdir "$scratch/eager" "$scratch/cursor"
git archive b95769558667b7a88654cb42d6673d08811ca4c4 | tar -x -C "$scratch/eager"
git archive 720b1a10712feb649d13b7918a944d5c43508191 | tar -x -C "$scratch/cursor"
(cd "$scratch/eager" && cargo build -p xtask --target-dir target)
export OMEGA_XTASK_BIN="$scratch/eager/target/debug/xtask"
python3 "$scratch/eager/bootstrap/independent/successor_test.py" --omega
python3 "$bench" "$scratch/eager" "$scratch/eager" "$scratch/eager-run" --measure
python3 "$bench" "$scratch/eager" "$scratch/cursor" "$scratch/cursor-run" --measure
```

The cold construction checks the native Gamma compiler against the reference and constructs Delta from source. The benchmark reuses those executables, checking their identities against the completed construction report. It changes the input comparison in the authored Gamma adapter and recomputes that member's 64-bit sum in Delta's support table. It runs the resulting Delta compiler on the same evaluator entry and checks that the complete Gamma output differs from the published receipt in exactly that adapter comparison. The same native Gamma compiler builds both executables.

A console fixture reads bytes 0, 128 and 255 and checks end of input twice. Empty source returns its usual rejection. The exact input limit admits a padded envelope and reports the trailing data at byte 52. One extra byte produces raw status 253 with empty output. Omit `--measure` to reconstruct both profiles and check these boundaries alone. With `--measure`, the supervisor samples the child's resident set every 120 ms, stops at 8 GiB or two hours, and reports the kernel's per-child peak. It reserves 4 GiB when the host exposes a finite cgroup v2 memory limit. The output directory retains both generated Gamma receipts, executables, and the complete request.

## Frozen input identities

The product request packs 284 units (3,822,481 source bytes) into 3,826,171 bytes. Its SHA-256 is `aa8891b09b5edbff0cb48bddb21eb20061b5e9ee1552e19a70339799211263dc`. The envelope includes those bytes, its 52-byte header, and 825,064 bytes of bootstrap compiler source including its entry. Changing the evaluator replaces only the 32-byte closure digest at offset 20 of the complete request.

| Evaluator | Whole request SHA-256 | Gamma receipt SHA-256 with 8 MiB input |
| --- | --- | --- |
| Eager | `2a078cce0cb5c671c9fd9ae91c29c00831dbabe5adc897ed170e59ed57dc4c26` | `d3586bcdbaac8c947814f1e9c593f6906041399563d17181e1884fc852127437` |
| Cursor | `eec989b92850e3d18f14586a22111e3dbad1a7cf3ac15bc1580ba4870c94ee34` | `5c9425d50aa62ca233f30e123a48ffcd424d6b6b451475c79cf4e82e3e934fbb` |

A smaller request kept all five packages, all four dependency edges and unit zero, the unchanged 13,510-byte `source/omega/application/construction/main.omg`. Encoding that single unit with the same entry produces 13,804 product bytes. The eager 8 MiB evaluator returned `00020000007f030000` in 268.335 seconds at 6,581,284 KiB. The bootstrap compiler reported unsupported source at coordinate 895, byte 765 of that frozen file, at `crashes Trap` on line 25.

## Collecting Gamma pairs

The frozen bootstrap compiler reached its first unsupported product declaration when its evaluator could reclaim unused pairs. It returned `00020000007f030000` on the entire 284-unit input, matching both the independent Epsilon model and the earlier one-unit diagnosis. The Linux process exited with status 0. The nine output bytes encode Epsilon Exit 2 and source coordinate 895, which points to `crashes Trap` on line 25 of the entry unit. This source revision produced a diagnostic before emitting a compiler.

| Sealed cursor, 8 MiB input | Outcome | Measured seconds | Peak RSS (KiB) |
| --- | --- | ---: | ---: |
| Ordinary cumulative allocation | Supervisor stopped the process | 240.553 elapsed | 8,391,356 |
| Two 1.5 GiB copying spaces | Compiler diagnostic at coordinate 895 | 2,522.924 active | 3,170,260 |

Both programs consumed the identical cursor request and came from the same 728,066-byte Gamma receipt in the preceding table. The collecting run used 2,466.443 user and 12.109 system CPU seconds. Including waits for its shared CPU, it took 5,920.700 seconds in 56 quanta. Its stderr was empty and the cgroup recorded no out-of-memory event. The smaller one-unit request also reached coordinate 895. Its peak fell from 6,579,260 KiB with ordinary allocation to 548,240 KiB with two 256 MiB spaces. The respective CPU times were 206.808 and 203.195 seconds.

The experiment changes the frozen Gamma compiler's lowering of pair allocation. Its nine ordinary Gamma definitions (137 lines, 5,857 bytes) emit existing instructions for the native and Alpha backends. Four changes in the frozen compiler initialize the heap, clear fresh local tags, call the allocator and include its instructions. The resulting 1.5 GiB profile takes 45,814 source bytes versus 40,173 for the frozen native compiler. Its self-rebuilt compiler grew from 1,635,203 to 1,783,751 bytes. Sharing one allocator reduced the cursor evaluator from 26,075,468 to 25,244,496 bytes. The numeric instruction operands in the lowering follow [the Alpha instruction table](../../../bootstrap/interpreted/0_alpha/SEMANTICS.md).

When the active space is full, the allocator copies pairs reachable from tagged frame slots and both operands of the pending allocation. It uses a forwarding address in each copied cell to preserve sharing, then visits the two fields of each copy. Frame links and scalar addresses carry the scalar tag. Established lexical slots can retain pairs until their frame returns, including after their last source use. Exhaustion now depends on reachable pairs. Each space holds 50,331,648 cells of 32 bytes. The frozen input's 3,822,611 nonzero blob bytes alone need at least 19,113,052 pairs in the authored sparse array representation. Each leaf uses two pairs and each branch uses three, with one fewer branch than leaves. The smaller 256 MiB space holds 8,388,608 cells. Its full run was stopped by the supervisor after establishing that lower bound.

To reproduce on Linux x86-64, follow the frozen and cursor setup above. `large_input.py` without `--measure` supplies the same receipt and request. The following opt-in test uses Python 3, GNU `patch` and the frozen Alpha seed. It reconstructs the collecting native compiler from source through two matching generations, builds the evaluator, and runs eight 32-cell controls using both native code and Alpha tape. The cases exercise active callers, duplicate references, both restored operands, unused frame slots, and exact and adjacent capacity failures. `--whole` executes the complete request under the same 8 GiB resident-memory guard used by `large_input.py`. Omit it to build the exact executable and run the small controls alone.

```sh
python3 tests/epsilon/evaluator-entry/collecting/compare.py \
  "$scratch/eager" "$scratch/cursor-run" "$scratch/collecting" --whole
```

The collecting compiler's SHA-256 is `242a3353102502d18b08fc625051172c92349d6a0c588ccd9af3708994f249b7`. The evaluator's is `c44852d8006ab8f78cc1d5f01096f53ead740c34eb01e0313d3f4ef35d1472be`, and the whole result's is `09cc2e49b328869319f3f447ee9bf21e492e6d396aab2ba199a2809b05b5010a`.

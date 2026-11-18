# Running the evaluator on the whole Omega compiler source

The published evaluator refused the 4,651,287-byte request before the bootstrap compiler read the product source. Widening its input limit to 8 MiB let execution proceed until a host guard stopped it at 8 GiB of resident memory. The evaluator that reads stdin through a cursor reached the same guard. None of these four runs produced a compiler or a source diagnostic.

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

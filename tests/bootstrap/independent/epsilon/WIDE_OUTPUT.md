# Publishing the whole native Epsilon compiler result

The customer is the 825,064-byte bootstrap Omega compiler and its executable entry from `b95769558667b7a88654cb42d6673d08811ca4c4`, SHA-256 `48f1b654605d1828c37af6e4472f2135367ecad1fc2435f2ef4919d862ab855f`. The Epsilon-to-Delta compiler, target runtime and lower stages come from `ab9a33cc15b11f6adc06ae411bdc653e1123aa4b`. Freezing these independently keeps a changing Omega compiler comparable across experiments.

The published native Epsilon compiler returned status 254 with empty stdout and stderr. It ran for 315.893 active seconds, took 1,268.914 seconds on the shared host, and peaked at 4,569,216 KiB RSS. This status comes from the generated `ConformanceBytesV1` adapter after `__d_main` returns a `Bytes` value longer than 4 MiB. The process completed within its 6 GiB RSS guard.

Both 128 MiB publishers returned byte-identical 29,522,323-byte Delta modules from the complete frozen source. Each native process returned status 0, emitted the storage marker and complete module, and wrote no stderr. The normal storage runtime and entry bring the program to 29,616,382 bytes. Its Delta compile request (DCREQ) is 29,619,396 bytes. The publishers also agreed byte for byte on that request.

| Full compiler run | Child CPU seconds | Scheduled seconds | Shared-host wall seconds | Peak RSS, KiB |
| --- | ---: | ---: | ---: | ---: |
| Indexed lookup | 2,467.432 | 2,506.901 | 8,209.417 | 4,576,976 |
| In-order traversal | 349.082 | 356.570 | 1,334.223 | 5,521,680 |

Indexed lookup used 7.07 times as much child CPU. Traversal used 944,704 KiB more peak memory. Child CPU and peak RSS come from Linux `wait4`. Scheduled time includes every CPU permit, and wall time includes pauses for other jobs on the host.

| Frozen compiler result | Bytes | SHA-256 |
| --- | ---: | --- |
| Generated Delta module | 29,522,323 | ef3ff813ca4c69693ad4afc34282eed47129492d0c78443385e7e12d65367897 |
| Delta program with runtime and entry | 29,616,382 | 59561e689e46291323256db5c089bb634e5acf09810a77c6423e582cc663d3d9 |
| DCREQ with published support | 29,619,396 | b25b41ed9b3e0c8a0cc593061aca59706b9c9a7c885df1df7724c3b5013dc4f2 |

The published Delta producer returned status 2 at the source envelope. The reader with a 64 MiB source limit admitted the request and reached its syntax arena limit at byte 5,214,811 of the program. It took 23.65 child CPU seconds and 119,296 KiB peak RSS to return the 40-byte refusal. Compilation stopped during parsing, before grammar validation, type checking or Gamma generation.

| Delta reader | DCOUT space | Code | Coordinate | Limit | Requested |
| --- | ---: | ---: | ---: | ---: | ---: |
| Published 4 MiB source envelope | 4 | 1 | 12 | 4,194,304 | 29,616,382 |
| 64 MiB source envelope | 1 | 7 | 5,214,811 | 114,294,752 | 114,294,800 |

The complete program has 7,501 top-level forms, 871,252 lists and 1,860,002 atoms. Delta's parser charges eight pairs per list, five per atom, one for the program root and 40 bytes per pair. A full parse would charge 650,801,080 bytes before grammar work. [Syntax storage contract](../../../../bootstrap/interpreted/3_delta/LANGUAGE.md) and [boundary test](../../../../_recycling/tests/delta/resource-boundary/syntax_storage.py).

## What changes at the two boundaries

The first experiment widens the maximum returned byte string from 4 MiB to 128 MiB in the ordinary Delta support member `support/adapter.gamma`. The Delta producer binds the member's new length, packed extent and rolling sum. Its maintained input cap is 4 MiB. The producer compiles the same 763,595-byte Epsilon-to-Delta source. Its generated Gamma differs from the published compiler at exactly the decimal integer in the output-length comparison. The original Gamma is 983,001 bytes, SHA-256 `4b0cc2d991e939b4cbb1f4cd5e7370fbd161e02a3d2f538a25523708f4149a8d`. The wide result is 983,003 bytes, SHA-256 `92685860046e45275e90a1b415d683169f2239d85525bfd85f385941f94428d9`. The resulting native compilers are each 34,801,979 bytes. They differ in exactly two bytes at zero-based offsets 34,797,088 and 34,797,089, both in the immediate that loads the result-length limit.

The second experiment changes the Delta producer's DCREQ source-length admission and matching diagnostic from 4 MiB to 64 MiB. Its source is 151,527 bytes, SHA-256 `41bc2cb6393d38be646770e96952a74b0c93c5a8bc360be3e5005978f718107f`. It consumes the published 2,998-byte support closure. The parser, grammar, type checker, lowering, emission limit and both Gamma backends are unchanged in this run. Both readers generated the same Gamma for an ordinary byte identity program. A header declaring one byte below or exactly at each limit reaches missing-body validation. One byte above it returns the corresponding exact source-provision frame.

A valid 4,194,305-byte source made by padding the identity program with spaces produced the exact same Gamma as its short source in the 64 MiB reader. The published reader refused the padded source at coordinate 12.

The ordinary identity program returned all input bytes at 4 MiB and refused a one-byte-larger sealed input with status 253 in both profiles. Appending one byte to a 4 MiB input returned status 254 in the published profile and the complete 4,194,305 bytes in the wider one. For the upper boundary, an ordinary Delta program repeated a 4,194,303-byte nonuniform sealed input 32 times and appended 31, 32 or 33 bytes.

| Requested return bytes | Wide native status | Published bytes | SHA-256 of stdout |
| ---: | ---: | ---: | --- |
| 134,217,727 | 0 | 134,217,727 | `860159a5064decb605bf083265a54f07aaffd7888a94d64d63184bfc1831a21d` |
| 134,217,728 | 0 | 134,217,728 | `53a649c6b6f5b8bc74421c263aaa6e50f86fcde8ac5d151eb07ba5c1bc221d74` |
| 134,217,729 | 254 | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Corrupting one byte of the bound support member while keeping its extent caused both Delta producers to return status 1 and the same 40-byte DCOUT frame. The byte-identity check reported space 5, code 2 and absolute member offset 2,590 for the identity source. No application code ran for these two requests.

## Publishing a concatenation tree

The indexed publisher runs the public byte lookup on the root for every index. A deeply nested concatenation makes those lookups traverse many of the same nodes. In a separate profile, three ordinary Gamma functions traverse the public byte tree in output order. Each visit to a concatenation allocates one pending-right pair. The native allocator retains these allocations for the rest of the process. Input views still read from the sealed input, and the adapter checks the total size before it writes the first byte. Both implementations eventually flush the ordinary native buffer with one Linux write call per byte.

An ordinary Delta program copied a 33-byte chunk 1,024 times. Each chunk contained the 32 input bytes and a singleton suffix. The left- and right-nested trees each returned the exact expected 33,792 bytes. The indexed native programs took 3.374 and 3.736 seconds respectively on this host. Walking the same trees took 0.017 and 0.013 seconds. Both also returned an empty result on empty input. Timing a small program this way isolates the publisher from the Epsilon frontend.

The direct publisher’s generated compiler is 984,196 bytes of Gamma, SHA-256 98047ef0276c9550a846b084fa661a3f22916392f1896605e4198bac84d8c3df. Its Linux ELF is 34,837,482 bytes, SHA-256 e5151e8e630d8ce35e6c7295baaf01228d9533110aff88d5331d563e21a66d1c. Four inputs from the published compiler gate produced the original bytes, including the 680,056-byte numeric scanner result, 11,334-byte pure lexer result, checked-source error and unsupported nominal expression. The numeric customer used 1.68 child CPU seconds on this host. The direct adapter also reproduced statuses 253 and 254 with empty output for the two oversized cases.

## Reproduce

Use Python 3.11 or newer on Linux x86-64. Keep the benchmark checkout, compiler checkout and frozen Omega sources separate. The selected source snapshots contain the seed, compiler members and test routes.

```sh
bench=$PWD/tests/epsilon/compiled-native/wide_output.py
scratch=$(mktemp -d)
mkdir "$scratch/peer" "$scratch/customer"
git archive ab9a33cc15b11f6adc06ae411bdc653e1123aa4b | tar -x -C "$scratch/peer"
git archive b95769558667b7a88654cb42d6673d08811ca4c4 bootstrap/interpreted | tar -x -C "$scratch/customer"
(cd "$scratch/peer" && cargo build -p xtask --target-dir target)
export OMEGA_XTASK_BIN="$scratch/peer/target/debug/xtask"
(cd "$scratch/peer" && python3 tests/bootstrap/independent/successor_test.py --compile-epsilon --seconds 3600)
python3 "$bench" "$scratch/peer" "$scratch/profiles" --calibrate
python3 "$bench" "$scratch/peer" "$scratch/traversal" --traverse --calibrate
source_root="$scratch/customer/bootstrap/interpreted"
"$OMEGA_XTASK_BIN" source-closure "$source_root/5_omega/omega_compiler.epsilon.sources" "$scratch/customer.body"
cat "$scratch/customer.body" "$source_root/construct_omega.epsilon" > "$scratch/customer.epsilon"
stock_status=0
"$scratch/peer/build/independent-successors/compiled-epsilon/compiler" < "$scratch/customer.epsilon" > "$scratch/stock.out" || stock_status=$?
test "$stock_status" -eq 254 && test ! -s "$scratch/stock.out"
"$scratch/profiles/compiler" < "$scratch/customer.epsilon" > "$scratch/wide.out"
"$scratch/traversal/compiler" < "$scratch/customer.epsilon" > "$scratch/traversal.out"
cmp "$scratch/wide.out" "$scratch/traversal.out"
python3 "$bench" "$scratch/peer" "$scratch/framed" --frame "$scratch/wide.out"
stock_status=0
"$scratch/peer/build/independent-successors/delta" < "$scratch/framed/program.dcreq" > "$scratch/stock-delta.out" || stock_status=$?
test "$stock_status" -eq 2
wide_status=0
"$scratch/profiles/reader" < "$scratch/framed/program.dcreq" > "$scratch/wide-delta.out" || wide_status=$?
test "$wide_status" -eq 2
sha256sum "$scratch/stock-delta.out" "$scratch/wide-delta.out"
```

The stock and source-widened DCOUT SHA-256 digests are `e3a6ea00f77825c87468c2dd5f75a392230333016ee1097c19862a0ed3b6d2b6` and `3299c898337296b8c82537a4f5abb2d47a07ff54d7073e882c4ff549e33f61b0`.

The benchmark constructs and measures private profiles. `--frame` packages the captured result with the pinned storage runtime, standard entry and published support. The production compiler and evaluator continue to use their published profiles.

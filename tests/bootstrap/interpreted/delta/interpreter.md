# Run Delta source through the direct interpreter

Run the same Delta programs on each host. The `gamma` row rebuilds Beta and
the Beta-written Gamma interpreter from the audited Alpha seed. The `alpha`
and `linux` rows take Gamma compiler executables already constructed by the
[Gamma host matrix](../../gamma_host_matrix.py), compile the direct Delta
interpreter for the selected target, and run that interpreter.

From the repository root, with xtask built and the Gamma host matrix output
saved under `/tmp/gamma-hosts`, use a fresh output directory for each run.

```sh
python3 tests/bootstrap/interpreted/delta/interpreter.py /tmp/delta-gamma --host gamma
python3 tests/bootstrap/interpreted/delta/interpreter.py /tmp/delta-alpha --host alpha \
  --runner /tmp/gamma-hosts/linux-to-alpha-compiler.bin
python3 tests/bootstrap/interpreted/delta/interpreter.py /tmp/delta-linux --host linux \
  --runner /tmp/gamma-hosts/linux-to-linux-compiler.bin --epsilon
```

Each route checks 27 Delta observations. They include three inputs to the
matrix's byte-doubling program, nominal fields and matches, mutual recursion,
strict call arguments, byte and arithmetic traps, publication and logical
length limits, malformed requests and whole-program static rejection. Four
scope cases cover equal-length and prefix-related names, name reuse in sibling
scopes, duplicate active binders and unbound references.
`--epsilon` adds the five portable Epsilon language observations and the two
invalid-token controls. Each one parses and checks the complete Delta-written
Epsilon evaluator and its entry, then executes it on the sealed EEREQ.
`--epsilon-case exit` selects just the Console exit observation. `--seconds`
sets a wall limit per invocation. The gate exits unsuccessfully on a mismatch
or timeout and prints the case name.

## Observed artifacts

The original runs used Gamma compiler sources at `35e76509787`. With the compact
Alpha backend the same interpreter source produced a 427,830-byte tape
(`1564536f75ee76d5b73582ebbc53ee08b1c2529cf2eccf785e19e2570c7df8f4`).
It passed the same 23 Delta checks on the seed, all five Epsilon language cases
and both syntax refusals.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Direct Delta interpreter in Gamma | 99,552 | `ac63405d4340438902cc273152798d24edc4f90c8229d3d01aeb6c400ffc24e5` |
| Interpreter compiled to Alpha | 822,922 | `7c9349e57e353b4513885618677848009bd39200c2586496fb224ea9c493546b` |
| Interpreter compiled to Linux x86-64 | 2,590,132 | `07ba048f85e5cde1b920cb7b5f08ad315d1f9605c311dffc5d047bed0d92edc4` |

All three hosts passed all 23 Delta checks. The Linux x86-64 interpreter also
passed all five Epsilon language cases and both refusal controls. The actual Delta source of the
Epsilon evaluator with its entry was 627,502 bytes. The exit case printed
`00 2a 00 00 00 41` with process status zero, including the guest's exit of
42 and its console byte `41`.

The canonical Epsilon `exit` request supplied 627,860 bytes to the Delta
interpreter, SHA-256
`7afcb01dbf939870d2f9019d7ec8ff39c33f506c77ee76d9547d2d254285a9b1`.
The 822,922-byte Alpha tape listed above and the original Beta-written Gamma
interpreter each reached a 240-second wall limit on this request before
returning an observation. Both ran as a single low-priority process on a
shared host. Reproduce with `--epsilon-case exit --seconds 240` and the
corresponding host command above.
The compact Alpha tape completed the same request in 64 seconds.

## Execute the Epsilon-built Omega constructor

Build the complete Epsilon compiler and use it to write the compiled Omega
module to `$work/module.out`. Stop after that module has been published.
The source-built Linux Gamma compiler is `$work/compiled/gamma-compiler`.
[Seed construction commands](../../independent/WHOLE_REQUEST.md)

Materialize the emitted module with its own storage runtime and entry. The
direct interpreter checks that complete Delta source before executing its
`main`. The following invocation supplies the linked two-package Omega
program as guest stdin. Use a new directory for the execution artifacts.

```sh
python3 tests/bootstrap/independent/epsilon/composition.py program \
  < "$work/module.out" > "$work/omega.delta"
python3 tests/bootstrap/independent/product_wire.py "$work/wire"
python3 tests/bootstrap/interpreted/delta/interpreter.py "$work/direct-wire" \
  --host linux --runner "$work/compiled/gamma-compiler" --profile construction \
  --source "$work/omega.delta" --stdin "$work/wire/linked.stdin"
```

The runner writes `guest.output` to the execution directory. A successful
Omega construction begins with five zero bytes (Epsilon exit 0), followed by
the complete Alpha tape. A constructor rejection has an Epsilon exit code
in the four bytes following the observation tag.

### Complete constructor on the 111-byte Omega package

The construction-profile direct Delta interpreter at `1068e79a837` ran the complete
Omega constructor compiled from the 909,531-byte Epsilon source at `bcf71f0bbfc`.
Given the same 111-byte Omega package, it and the 54,882,761-byte Linux constructor
returned identical 27-byte Epsilon observations. Each returned five zero bytes
followed by the 22-byte Alpha tape. The seed ran the tape and exited 42 with empty
stdout and stderr.

| Physical execution | CPU seconds | Peak RSS (KiB) |
| --- | ---: | ---: |
| Check and interpret 17,272,866 bytes of linked Delta | 656.06 | 302,136 |
| Run the compiled Linux constructor | 2.84 | 79,008 |

The direct request contained a four-byte little-endian Delta length, the linked
Delta program (SHA-256 `3fe7a0e0d34d0f473386185d73362b022be47223da707ce8b9b73151f750e76d`),
and the named package. The complete request hashed to
`92da76b5b800fc1d8a954e18eaf5719c0b4344b2722762916fdadf9279ebf8e1`.
The Linux interpreter hashed to `541313339a4c95e5dfb54c227835ef20954555a220dfe432757f1b4d3c18133d`.
Its run took 9,129 shared wall seconds, including scheduled pauses, with no wall limit.
The resulting tape hashed to `25c789ed6ce473847a1dc82854aad7b59d3ff336d4ed73a64a56c540b5d78384`.
[Named package and seed check](../../independent/WHOLE_REQUEST.md#compare-the-complete-omega-compiler-across-routes)

Omit `--source` and `--stdin` to run the direct interpreter's control suite.
The default conformance profile retains its 4 MiB source limit. The construction
controls exercise larger guest input, source admission, static rejection and
oversize output. [Direct interpreter source profiles](../../../../bootstrap/delta/interpreter/README.md)

### Local bindings and the linked constructor

The two-package input above is 829 bytes. Its expected result from the
Epsilon evaluator is five zero bytes followed by a 1,607-byte Alpha tape
that exits 42. Both direct runs checked the same 18,365,364-byte Delta
constructor and used the same compact Linux Gamma producer, 1,300-second
wall budget and 6 GiB resident-memory guard. The host stopped both runs
at the memory guard before either published a byte.

| Runtime local bindings | Wall seconds | CPU seconds | Peak RSS (KiB) | Output bytes |
| --- | ---: | ---: | ---: | ---: |
| Persistent name trie | 515.68 | 371.12 | 6,292,352 | 0 |
| Linked rows of source spans and values | 740.63 | 649.25 | 6,291,840 | 0 |

At 371.39 CPU seconds the linked rows had used 3,835,080 KiB. Each bind now
allocates three pairs. Lookup scans the active rows and compares identifier
bytes. To isolate the allocation cost, run this Delta source with the two
input bytes `XY` through the same Linux interpreters.

```lisp
(def count_down ((remaining_count Int) (result_at_end Bytes)) Bytes
  (if (eq remaining_count 0) result_at_end
    (let remaining_prior Int remaining_count
      (let remaining_after Int (- remaining_prior 1)
        (count_down remaining_after result_at_end)))))
(def main ((input Bytes)) Bytes (count_down 20000 input))
```

Both returned `XY`. Peak RSS fell from 85,760 to 15,744 KiB and CPU time
from 3.423 to 2.043 seconds. The Linux conformance and Alpha construction
controls passed all 27 and 30 observations respectively.

The compact Linux interpreters also executed the complete 4,107,257-byte
Delta signature-checking customer (SHA-256
`3a726430d1e02a312dba9a8d68ec54331a474f1dab671c5e6ad3bc394ebebbc6`)
with empty guest input. Both exited 0 and returned the frozen Epsilon
observation `00 2a 00 00 00 01 02 00`.

| Runtime local bindings | Wall seconds | CPU seconds | Peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| Persistent name trie | 17.17 | 15.51 | 277,248 |
| Linked rows of source spans and values | 17.59 | 16.53 | 255,232 |

With the full constructor and the 239-byte portable Omega product, the old
and new direct interpreters both reached the 600-second wall limit with
zero output bytes. The name trie used 455.38 CPU seconds and 9,455,744 KiB
peak RSS under a 14 GiB guard. The linked rows used 475.55 CPU seconds and
4,054,016 KiB under a 6 GiB guard. On the same product the Epsilon evaluator
returned the five-byte success observation and a 50-byte tape that exited 42.

### Calls and output from the real Epsilon compiler

A direct Delta call binds each argument into the callee’s frame while evaluating
all arguments in the caller. Constructors keep their ordered fields. A local
row retains the checked name node and its value. The output walker reads each
nonempty rope leaf in order. Dropping empty children at concatenation keeps a
shared subtree from multiplying empty wrappers during publication.

Both versions below used the same seed-built Linux Gamma compiler from #4690,
SHA-256 `16a7b5be0f53501a694eb397b24741881731462c0490f8c3fd5d0575e54eddae`.
The checked direct interpreter grew from 100,771 to 102,214 bytes of Gamma.
The named workload ran the 630,512-byte **Delta source** of the Epsilon
compiler at `93ad7259148`. It produced the exact 65,013-byte classifier module
(`bd1d5bd47e66…`) and returned `rejected 18 2032` for the wrong-base argument.
These are single CPU measurements on a shared host.

| Direct Delta workload | Previous CPU seconds | New CPU seconds | Previous / new peak KiB |
| --- | ---: | ---: | ---: |
| Compile the classifier | 26.10 | 23.58 | 31,420 / 29,332 |
| Refuse the wrong-base twin | 16.08 | 17.30 | 20,016 / 19,936 |
| Append 512 singletons to `Z`, then publish | 0.081 | 0.037 | 10,368 / 10,368 |
| Append 2,048 singletons | 0.978 | 0.149 | 10,368 / 10,368 |
| Append 4,096 singletons | 3.250 | 0.257 | 10,368 / 10,368 |
| Run the 20,000-call countdown above | 1.124 | 1.733 | 10,368 / 10,368 |

To reproduce the compiler workload, save the named classifier and wrong-base
rows using `route.py` on the pinned sources. Pass their `epsilon-compiler.delta`
to the direct interpreter as `--source`, and each row’s `subject.epsilon` as
`--stdin`. Select `--profile construction`. The complete framed positive input
has SHA-256 `14bda93ed27f1de45b28bc3079be535fe562ef7af4c9470723467a4cf11a80ff`.
For the append control, start with the program’s stdin `Z` and repeatedly append
`bytes_single(n % 256)` as `n` counts down from the indicated length to one.
The published bytes matched in every comparison.

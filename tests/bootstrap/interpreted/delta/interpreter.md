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
  --source "$work/omega.delta" --stdin "$work/wire/linked.stdin" --seconds 1200
```

The runner writes `guest.output` to the execution directory. A successful
Omega construction begins with five zero bytes (Epsilon exit 0), followed by
the complete Alpha tape. A constructor rejection has an Epsilon exit code
in the four bytes following the observation tag.

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

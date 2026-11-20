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

Each route checks 23 Delta observations. They include three inputs to the
matrix's byte-doubling program, nominal fields and matches, mutual recursion,
strict call arguments, byte and arithmetic traps, publication and logical
length limits, malformed requests and whole-program static rejection.
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

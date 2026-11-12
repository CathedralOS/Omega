# Standalone-language construction experiment

This peer constructs standalone Linux x86-64 compiler executables from Gamma
source. Generated successors execute their own native instructions and Linux
system calls: they carry no Alpha interpreter, seed container, bytecode dispatch
loop, or host compiler. The first customer is the actual Gamma compiler from
[`../compiled_vm/`](../compiled_vm/README.md), compiled into a native executable
that emits the ordinary Alpha compiler tape.

`native.gamma` replaces instruction sizing and emission in the shared Gamma
compiler. Source composition is explicit concatenation, in order:

1. `../compiled_vm/compiler.gamma` owns declaration validation, scoped expression
   lowering, tagged values, call frames, runtime checks, and label resolution.
2. `../compiled_vm/frontend.gamma` owns the Gamma source grammar and parsing.
3. `native.gamma` emits ELF64 headers, startup, and native instruction sequences.

No source text is rewritten or recognized as a special customer. The same
frontend and lowering serve the Alpha and native realizations. Every instruction
record becomes a fixed native sequence; labels resolve to actual code addresses.
The compiler validates all functions and branches before buffered publication.
The public [Gamma semantics](../interpreted/2_gamma/LANGUAGE.md) and existing
compiler's explicit accepted subset govern both realizations.

## Construction and acceptance

On Linux x86-64, from the repository root:

```sh
python3 bootstrap/independent/source_test.py
```

The cold route starts with the audited Alpha seed and separately admitted Beta
tape from `../interpreted/`. It reconstructs Beta from source byte for byte,
constructs the Gamma evaluator from Beta source byte for byte, then executes the
native compiler source through that evaluator. This constructs `compiler0.elf`.
Its successors rebuild the native compiler source into identical `compiler1.elf`
and `compiler2.elf`. These executions require neither the evaluator nor Alpha.

The native compiler then compiles the actual Alpha-targeting Gamma compiler
source into another ELF executable. That executable emits the Alpha compiler
tape from its real source; the stamped tape reconstructs the same tape exactly.
Each realization runs the existing 32 semantic and 15 rejected-source controls,
including wrapping arithmetic, signed division, scalar/pair separation, mixed
arity proper tail calls, complete source validation, sealed input, and atomic
publication. Five additional native controls cover exact/adjacent emitted-image
capacity and returned mapping/read/write errors. Capacity exhaustion returns 254
with no image, distinctly from a source refusal. The compiler fixed point is an intermediate construction check,
not the final Omega acceptance.

`--accelerated` explicitly adds the Python Gamma compiler during development.
It is a separate trust profile; normal construction never invokes that semantic
producer. Python and shell remain replaceable source concatenation, framing,
seed stamping, permission, process observation, and byte-comparison scaffolding.
No C/Rust compiler or host assembler/linker constructs the native executables.

Artifacts, exact requests, and `report.json` go to ignored
`build/independent-native-reference/` (or `-accelerated`). `--seconds N` sets a
positive process watchdog. A timeout is an observation failure. A source refusal
must exit 249 without output. Successful compiler results are complete images;
the harness writes retained artifacts only after successful construction.

## Runtime and audit boundary

The ELF has one read/execute load segment and no dynamic interpreter or shared
libraries. Startup zeros 256 word slots on the process stack and reserves a
128-GiB private anonymous virtual region using `mmap(MAP_NORESERVE)`. Host `r14`
points to the register slots and `r15` to that region. Every instruction loads
its known slots into scratch registers, computes directly, and stores its result.
Native calls and returns use the process stack; source tail calls use native
jumps. Decimal emission constants carry their exact little-endian hex bytes in
comments for review.

The shared compiler's disjoint frame, sealed-input, buffered-output, immutable
pair and call-depth guards remain authoritative. Pair handles retain a separate
unforgeable tag. Only compiler-generated memory instructions enter this backend;
this is not a compiler for arbitrary self-modifying Alpha programs. Native
instruction records cannot be supplied by Gamma source. Output has the existing
128-MiB buffer limit, including the 128-byte ELF header and 88-byte startup.
The Alpha backend separately retains its audited 16-MiB seed-hole limit.

Storage limits and authored traps retain the shared compiler's distinct statuses.
Returned `mmap`, read or write system-call errors exit 3. Host signals can
terminate execution separately: a pipe with no reader has the ordinary default
`SIGPIPE` behavior, rather than guaranteeing status 3. Physical memory and sufficient
process stack remain host dependencies; `MAP_NORESERVE` does not promise physical
memory availability. With the shared 131,072 call-depth limit, native return
addresses use at most 1,048,584 bytes plus the 2,048-byte register file and one
8-byte I/O scratch slot. Existing source bounds protect Gamma data accesses;
this experiment does not claim a machine-checked implementation proof.

## Delivery continuation

No peer is selected by this experiment. The remaining delivery path is to use
native Gamma to construct the actual bootstrap compiler implementation, then
compile the shared Omega product source outside this directory and reproduce
its self-rebuild. Source/compiler/runtime size, execution and memory costs, and
failure observations must stay tied to their exact source closures. Intermediate
self-hosting and small language controls do not complete that path. Additional
facilities must serve that actual successor; a second source recognizer,
precomputed compiler image, or proof-certificate subsystem is not part of it.

## Construction evidence

A fresh Linux x86-64 cold run constructed the following exact closure and
artifacts. Two native compiler rebuilds matched byte for byte; the native Alpha
compiler's output also matched its Alpha-executed self-rebuild. The disclosed
Python-assisted development route independently produced the same native image.

| Subject | Bytes | SHA-256 |
| --- | ---: | --- |
| Native compiler source closure | 40,173 | `c174cc8862efbfaf6279d77e0624f51b4f60b6a557a55a2c5b05a05b73e30f07` |
| Native compiler ELF | 1,635,203 | `15831075c8059252e707957e3880b66a42b3e1df240a30803bbe348333690961` |
| Alpha compiler source closure | 33,605 | `8cc6f6ce3d29ff38a7f94c48857ac6036f0fe09110a442087ebbbb98d48cee86` |
| Native Alpha compiler ELF | 1,453,814 | `c157f919ea2cc0b53f2826e817262ff8217233be77f114d5910facb108867a06` |
| Alpha compiler tape | 458,402 | `25b8c1875dd866f885654c5344a16153f3a434eff9555196657f5f0249ef1af1` |

All 32 semantic and 15 refused-source observations passed for each realization,
along with all five native capacity and returned-system-call-error controls.
Reference construction took 155.649 seconds; the largest child-process resident
set reported by Linux `getrusage` across the complete run was 48,384 KiB. This is
a measured largest child RSS, not total simultaneous memory or the 128-GiB
virtual reservation. These are functional observations on a shared host, not a
controlled speed comparison or a compiler-correctness proof. None constructs
the Omega compiler source or closes its self-rebuild acceptance.

## Native successor customers

The next construction check uses the real Delta compiler and Epsilon evaluator,
then optionally runs the actual bootstrap Omega compiler on its existing fixture:

```sh
python3 bootstrap/independent/successor_test.py --omega

# Explicitly reuse the retained source-constructed native compiler after
# checking its source and ELF identities against the construction report.
python3 bootstrap/independent/successor_test.py --reuse-constructed --omega
```

Default execution first repeats the cold construction above. Reuse does not
invoke another compiler; it accepts only the retained reference construction
whose exact source and executable match. The retained differential Gamma
evaluator must also match a freshly stamped image of the audited Alpha seed
and the bound Gamma tape reconstructed during cold construction. This extra
stamping is an identity check, not a semantic construction shortcut. The test
compiles Delta to native ELF,
compares its real Bytes-transformer receipt with the constructed Gamma evaluator,
and executes the native receipt on empty, ASCII and all-byte inputs. Native Delta
then reconstructs the bound canonical Epsilon Gamma receipt, and native Gamma
compiles that receipt to ELF. Existing Console exit/stdin/trap/refusal and
malformed-envelope controls run unchanged. Both peers share these actual
customers; the compiled VM peer retains Alpha stamping by default.

This is a construction steppingstone: Epsilon remains the existing evaluator,
including its explicitly documented [implementation profile](../interpreted/4_epsilon/README.md).
The bootstrap compiler's Epsilon source still executes through that evaluator.
These results do not establish a standalone compiled bootstrap compiler, the
full normative Epsilon language, or an Omega compiler self-rebuild.

A retained-source-constructed-compiler run passed all those customers and the
Omega fixture with these identities:

| Subject | Bytes | SHA-256 |
| --- | ---: | --- |
| Delta compiler Gamma source | 147,840 | `fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929` |
| Delta compiler native ELF | 3,873,465 | `e465c0c4015ad9b3474689f18f3ad6b1324e3bc89c688dff378e73f9e353a6bb` |
| Bound Epsilon Gamma receipt | 729,060 | `bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368` |
| Epsilon evaluator native ELF | 26,119,826 | `b1cc4441a52ec91feae60d958ada4c400122ce070a2ec66bdd8982c16bbfe2e0` |
| Exact Epsilon/Omega request | 734,042 | `326a8730e5aacf0880e638adaf16ecadbc5ae367c1318316d0885e001bba9e02` |
| Emitted Omega Alpha program | 50 | `7f22a6c26b9c2876da3ccba9c62fe1cc071cde61fb31399b0d35fb1fa7ca9cae` |

The Epsilon image exceeds Alpha's 16-MiB seed hole and serves the native backend's
larger image profile. The exact request includes the 714,400-byte bootstrap
compiler (`30e6ea13e8ccab73633af4887dfb3d5ef00d1c25a114f01496aa2290cd6c9511`)
and the existing OCREQ entry. It produced a 55-byte Epsilon observation; the
actual emitted program executed with exit 42 and empty output. That Omega stage
took 41.196 seconds. The largest child RSS across successor construction and
execution was 663,552 KiB. These are individual shared-host functional runs,
not a controlled performance comparison.

Exact sources, requests, artifacts and stage checkpoints go to ignored
`build/independent-successors/`. The current invocation starts a fresh preflight checkpoint with its PID and
start time before construction or identity checks. The report names passed
stages before starting the next, and marks the complete run passed only after
all requested checks.
A surviving `running` checkpoint is incomplete evidence. Invocation watchdogs
retain the existing customer-specific limits; `--seconds` bounds native Gamma
compilation and the optional Omega observation, while smaller controls retain
60-second process watchdogs. Host processes and resources remain execution
dependencies as documented above.

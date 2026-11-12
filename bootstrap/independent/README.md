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

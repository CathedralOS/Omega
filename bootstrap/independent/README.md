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

The native backend retains an **auditability failure**: numeric semantic
operation identities and the nested `i_size` ladder make meaning costly to
recover. Functional controls, scoped ISA/ELF review, and 214 source lines do not
establish auditability. Gamma already permits named operation functions,
predicates, and clearer decomposition; missing variants/matching do not excuse
this code or make a language addition inevitable. Retention comparison must
consider source refactoring/deletion before an earned facility or a different
rung. Necessary numeric encoding bytes are separate from semantic operation
identities. This milestone records the debt rather than claiming to repair it.

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

The next compiler's source encoder now uses Delta byte literals rather than
packed numeric text templates. Run its facility and actual encoder controls
with `successor_test.py --literals` (and optionally `--omega`). The encoder
consumes existing semantic name/input records; this remains an encoding module,
now used by the checked scalar lowering below. Its emitted Delta function is compiled
to native code and executed; generated-function runtime needs shared semantic
types, not the Epsilon parser or expression/state dispatcher.

The changed Delta closure is 151,525 bytes,
`de94946dd946c90c3311dba52aa4622a3c80a18866d5483531de1312cc62e0c6`.
Its native ELF is 3,984,546 bytes,
`fce5a6eb20973f6801309579b336b111e38d22d1cedb388b2fa35b4f86671a39`;
source-owned Alpha construction produced 1,219,139 tape bytes,
`6fde052af5b348f3b83f29635aaaedbd6959343352da3a3af1442000a102b1d0`,
matching the independent development producer. A focused all-escape/1,024-byte
literal and malformed twin agreed byte for byte through the selected Gamma
evaluator and Alpha Delta; the positive generated program also executed as
Alpha. Seven literal positives, ten malformed literals, three role/type refusals,
and the actual encoder/compiled-output controls passed. Existing frontend,
request, and Alpha customer controls passed; the canonical Epsilon receipt,
native Epsilon image and Omega fixture artifact above remained byte-identical.
These observations do not discharge the auditability debt or the existing open
whole-compiler cumulative arena-containment obligation.

The [shared native correspondence record](../compiled_vm/README.md#native-epsilon-correspondence-for-the-same-storage-customer)
now includes the full frozen SourceUnit storage customer through this same
native Epsilon evaluator: all 13 executable artifacts and 36 refusals matched
the Alpha VM observation. That record owns exact input/artifact provenance,
reused-runtime disclosure, allocation samples and shared-host timing limits;
it is stronger evaluator workload evidence, not compiled-Epsilon or Omega
self-rebuild acceptance.

## Checked Epsilon compilation

`successor_test.py --compile-epsilon --literals --omega` constructs a
Delta-written compiler using the source-constructed native predecessors, then
compiles and executes the complete unchanged production
`lexical_classification.epsilon` with its `OmegaNumericBase` declaration. This is a compiled source
customer: the generated executable contains direct functions and lifted states,
with no Epsilon parser, checker, expression tree or AST dispatcher in its runtime.
The default invocation repeats cold construction; `--reuse-constructed` retains
the earlier explicitly checked reuse meaning.

The compiler first applies the complete existing Epsilon body-checking boundary,
including its [legacy arm-return admission profile](../interpreted/4_epsilon/README.md).
It then lowers
every machine and state body, including unreachable bodies. The current pure
capability is scalar and payload-free enum parameters, literals, checked scalar
operators, free calls, scalar returns, and scalar or explicitly exhaustive enum
state transitions. Parameter identity and call/control
identity come from the checker's retained records. Storage reads/writes,
receiver access, Console operations, payload-bearing sums, enum returns,
enum wildcards, arrays, views and ordinary statements
remain explicit `unsupported` results. Invalid source returns `rejected` with
the existing reason and packed offset; contradictions return `internal`.
Success is `compiled` followed by a newline and one complete Delta module.
Rejected, unsupported and internal responses contain no module. This is an experimental module compiler,
not a standalone compiled bootstrap compiler or full Epsilon implementation.

Pure functions use direct `Int` parameters under the checked Epsilon scalar
contract and a named scalar/resultless/trap/internal outcome. Literal and typed
parameter reads lower directly; operations that can trap retain outcome
propagation and short-circuit evaluation order. The shared Epsilon I32 helpers
own overflow, division, remainder, shifts and bitwise meaning. Delta primitive
failure is not an Epsilon trap. Future effectful lowering must bridge these pure
results to the existing store/outcome contract where effects occur. Omega's
full-u64 `Wrapping` customers are not implemented by this I32 capability.
Named `EvaluateBoth`, `ShortOnTrue` and `ShortOnFalse` compiler cases express
eager and short-circuit control.

The complete Epsilon source still packs to exactly 617,354 bytes and the same
canonical Gamma/native evaluator artifacts. Only the source ownership changes:
binary operator and scalar outcome declarations now have shared leaf files.
`epsilon/composition.py` selects those complete files plus the checked scalar
implementation and a small named result adapter. It performs no host semantic
extraction or dead-code selection. The compiler executable currently includes
the full shared Epsilon closure; narrowing that retained checking dependency is
still a whole-chain cost opportunity.

The PR2737 scalar milestone first compiled four lexical machines. An initial implementation threaded generic runtime values and stores through
every scalar node. Its lexical target was 123,827 Delta source bytes and
5,308,895 native bytes, with 68,837 bytes of fixed runtime. The direct scalar
lowering reduced the marginal generated functions to 11,105 bytes and the
native target to 882,941 bytes; its shared runtime is 5,429 bytes. These are
source/artifact sizes, not runtime speed measurements. Exact current identities,
requests, Gamma receipts, images and observations are retained in
`build/independent-successors/compiled-epsilon/`.

The focused customer checks all 256 byte values against the canonical Epsilon
evaluator and an independent expected observation. Twenty scalar controls cover
call/state arguments, I32 overflow, division/remainder traps, shift bounds,
logical guards/short-circuiting, scalar transition fallthrough, comparisons and
bitwise operations. Invalid-name and valid unsupported twins cover refusal
publication; empty input and byte 255 distinguish EOF. These controls do not discharge the native auditability debt above
or the remaining path through actual storage/Console customers, the complete
bootstrap compiler, shared Omega compiler and Omega self-rebuild.

| PR2737 scalar artifact (historical) | Bytes | SHA-256 |
| --- | ---: | --- |
| Epsilon-to-Delta compiler source closure | 658,613 | `97631db8c1962aca6331e08e03cf18d0c5b3013f86e9795ada2d876f67b50c7e` |
| Compiler native ELF | 28,287,105 | `69cbea7eda03e57ec362e282daabcc1c542614d0f636d273e95944eee4fbea6a` |
| Target scalar runtime | 5,429 | `89f0634a158f6c7ae4ad3acc58347713ce385f38010335dc4f019b69f4d2db6a` |
| Generated lexical functions | 11,105 | `feb75abbca7ec4ad5d97a676f2e065edfab3c9f5e7bdcc03d5d10ade71b7a5ea` |
| Lexical native ELF | 882,941 | `d54ea84617373d07691ea6d8bf166e78d10ace940a39c3c1fce9c679ca43aac7` |

Validation retained the complete cold run, including native/Alpha compiler
controls and the Omega fixture. Final shared-file separators were revalidated
with explicit predecessor reuse, and the named-control refinement repeated the
focused compiled customer and all its controls. The last focused run took
45.526 seconds with 716,928 KiB maximum child RSS on the shared host. Cold native
compiler construction took 149.653 seconds and rebuilt byte-identically twice.
These observations are functional receipts, not controlled speed comparisons.

The next customer includes all five production lexical machines (2,520 bytes)
and the exact 100-byte `OmegaNumericBase` declaration. It accepts raw ASCII
scalar values: binary `48..49`, octal `48..55`, decimal `48..57`, and hexadecimal
also `65..70` and `97..102`. The source-Psi helper has the same predicate on
the tested `0..255` range, while its `u32 in Trapping` domain remains distinct
from Epsilon's `i32` domain. Test adapters retain their own observation wires.

The compiler emits named Delta enum declarations and direct matches. Typed
callee parameters guide argument lowering, including enum transport through
machine parameters, lifted states and free calls. Generated parameter/state
names retain authored names plus lexical coordinates, such as
`ep_local_1407_base` and `ep_state_1655_binary`. Nominal declarations precede
target functions even when the authored Epsilon declaration follows them.
Composition concatenates complete owners; it performs no host AST extraction.

All 256 raw bytes across four bases and 1,024 forwarded cases agree between
native, source-owned Alpha, canonical Epsilon and independent expected output.
Empty-input and byte-255 controls distinguish EOF. Two negative sources reject
as `TypeMismatch`: distinct owners with identical case spellings, and enum
equality under Epsilon's scalar-only comparison rule. Three reference-valid
sources return `unsupported` for payloads, enum returns and enum wildcards,
without emitting partial type/function text. Wildcard expansion remains a
deliberate boundary; explicit exhaustive matches lower each continuation once.
This customer exercises no storage/default initialization.

| Complete lexical customer artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Compiler source closure | 680,001 | `17b893307f4c98282e6364d50ce60cc0d4d1d58d9bd1b0b817d14a375e5805cc` |
| Compiler native ELF | 29,524,318 | `a6fa540c1f2253fc5076b35c791b62aa0bf5dbeddf4b68036ac2e1b8efe68d61` |
| Generated lexical functions/types | 22,431 | `61501a41c1d1fe445e1526a64749260dbb1ad0e587cc68c5d2d8f0f5bb452ceb` |
| Native ELF | 1,288,464 | `728acc958bfc6f2b17c23f238aab3957741dbec7927213ffe8806a72f5b3171c` |
| Alpha tape | 403,669 | `e92be2d814063d4e84e8f3a79f7c73f823f31a445eff80dec6027eefcccbf3f4` |
| Complete observation | 3,072 | `ff892de42bfed3f900a8adc303fa93a1f2cf91f6033986638e49ea36bc9d1597` |

The 5,429-byte arithmetic runtime remains unchanged. Source, requests, receipts,
images, refusals and the final invocation report remain under
`build/independent-successors/compiled-epsilon/`. The next construction boundary
is the actual receiver/storage operations used by the bootstrap compiler;
neither this module nor its successful controls completes Epsilon or the whole
bootstrap/Omega self-rebuild goal.

The final invocation used `--reuse-constructed --literals --compile-epsilon
--omega`, explicitly checking retained cold predecessor identities. Source-closure
and Epsilon identity/refusal gates, the updated literal encoder, all scalar and
nominal controls, unchanged canonical Epsilon artifacts, and the Omega smoke
fixture passed. It took 166.949 seconds with 748,544 KiB maximum child RSS on the
shared host. Earlier cold receipts remain historical evidence for their exact
sources; this invocation does not claim a new cold seed run or controlled timing.

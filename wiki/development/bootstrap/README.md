# Bootstrap experiments

For measured before-and-after examples of the Gamma scanner, Epsilon keywords and evaluator wire entry, see the [source audit comparison](source-audit.md).

## Select a route

From the checkout root, choose how Gamma runs the Delta implementation, how
Delta runs the Epsilon implementation, and how the Epsilon program itself
runs. Each selector accepts `interpret`, `alpha`, `native` (Linux x86-64), or
`all`. This command holds Gamma and Delta fixed while varying Epsilon.

```sh
python3 tests/bootstrap/route.py /tmp/omega-three --gamma native --delta interpret --epsilon all
```

The default control reads `Z`, writes it, and exits 90. All three routes
must produce `00 5a 00 00 00 5a`. Use `--source FILE --stdin FILE` for your
own Epsilon program and raw input. Add `--expect-observation FILE` to check
the exact binary Epsilon result. An exit or trap runs the guest. A six-byte
source rejection checks the frontend’s code and source offset on each route.
The row records the SHA-256 of the supplied observation. Every completed
invocation also records the executable and complete input digests, including
single-route runs.

Use `--plan` to inspect the stages without running them. The plan shows which
compiler runs, where it runs, and which runtime handoff is required. Interpreted Delta defaults to the direct interpreter.
Add `--delta-interpreter gamma` to translate Delta and interpret the emitted
Gamma, as in the original interpreted route. [Route runner](../../../tests/bootstrap/route.py)

Use `--case omega-byte-classes` to run the Omega compiler’s byte classifier
on all 256 input bytes. Its Epsilon entry writes twelve answers per byte,
including digit membership in four bases both directly and through a function
taking the named enum. This command checks every answer and exit 42 on all
three Epsilon routes.

```sh
python3 tests/bootstrap/route.py /tmp/omega-bytes --gamma native --delta native --epsilon all --case omega-byte-classes --reuse-stages
```

Use `--case omega-wrong-base` with the same command and a fresh output directory
to change the forwarding call from `omega_digit_in_base(value, selected)` to
`omega_digit_in_base(value, value)`. The second argument is now an `i32`.
The interpreter records the six-byte `TypeMismatch` in `guest.eeout`. Each
compiler records its complete ASCII response in `compiler.response` and reports
`expected-rejection` before building a guest. The batch compares error code 18
and the byte offset of the call head across all three choices.

Each run needs a new output directory and the repository's built `xtask` for
source manifests. Each row retains its source, input, intermediate artifacts,
and `result.json` with the stages, times, input hashes, generated artifact hashes and outcome. In a batch, each row logs live progress in `stages.log`. Rows that receive a binary Epsilon observation retain `guest.eeout` for byte comparison. A timeout records the
stage in the row report and exits 124. Other failures exit 1. Batch runs finish the other rows
and display the Epsilon choices side by side. Completed custom runs must agree on the observation hash.

For a long matrix run, add `--reuse-stages` to run each distinct compiler
invocation once. Two rows can share an intermediate artifact when both the
executable and its complete framed input have the same SHA-256. The runner
checks the saved output digest on reuse. Each row's `result.json` records
the two input digests and names the original artifact in `reused_from`.
Every row runs its final invocation, including calls that check the expected
compiler rejection. Omit the flag to rebuild each row’s intermediate artifacts
independently.

Compiled Gamma hosts start from the cold launcher. To reuse a previous build,
supply `--gamma-build DIR` with the corresponding `--gamma alpha` or
`--gamma native`. The runner checks its compiler source and fixed-point
rebuild and records the host executable's hash. Interpreted Gamma starts
from Beta. `--profile construction` selects the larger Delta byte limits for
whole-compiler inputs. [Cold build](../../../tests/bootstrap/linux_x86_64/README.md)

### Same stdin program across routes

All 27 routes ran the portable stdin control from `f040d53f89d` with a
120-second limit per producer or guest invocation. The table counts matching
Epsilon observations out of the three Epsilon choices for each pair of lower
rungs. The runs reused cold-built Alpha and Linux Gamma compilers.
Every completed route returned `00 5a 00 00 00 5a`.

| Gamma host | Direct Delta interpreter | Delta on Alpha | Delta on Linux |
| --- | --- | --- | --- |
| Interpreter | 3 timed out | 3 timed out | 3 timed out |
| Alpha | 3/3 | 3/3 | 3/3 |
| Linux | 3/3 | 3/3 | 3/3 |

Among those 18 completed routes, all twelve that compiled the guest produced
identical Delta and Gamma source. All six Alpha targets emitted the same
487,013-byte tape. All six Linux targets emitted the same 260,706-byte executable.

One of the nine timed-out routes completed with a 900-second per-stage limit.
With `--gamma interpret --delta native --epsilon interpret`, interpreted
Gamma ran the Delta compiler in 743.55 seconds and the native Gamma backend
in 555.79 seconds. Both the 726,601-byte evaluator source and the
1,977,558-byte Linux executable matched the native-hosted build byte for byte.
The Epsilon program then returned the same six bytes in 0.02 seconds.
Two further routes interpreted Gamma and Delta directly and ran the compiled
Epsilon guest on Alpha and Linux. The original Gamma interpreter also built
the Linux Epsilon compiler from the matched `5144ec25e81` sources, and both
of its generated guests echoed `Z` and exited 90. The portable stdin control
has therefore completed on **23/27** routes across the recorded snapshots.
[Direct-Delta receipts](interpreted-gamma-matrix.md#replay-the-compiled-guest) · [Linux Epsilon compiler receipts](interpreted-gamma-matrix.md#building-the-portable-stdin-program-with-the-original-gamma-and-linux-epsilon-compiler)

### Omega compiler byte classifier

**21/27 routes** ran the classifier and refused its wrong-type twin.

The positive program runs the production lexer’s byte classifier on all 256
byte values and checks twelve answers per byte. Its twin passes an integer to
a parameter requiring the numeric-base enum. **Both** means the route ran the
positive and its frontend refused the twin at byte 2032. **Positive** records
the successful execution alone. **Refusal** records the exact rejection while
the full positive is unfinished. **Open** has neither completed observation.

| Gamma | Delta | Epsilon interpreter | Epsilon to Alpha | Epsilon to Linux |
| --- | --- | --- | --- | --- |
| Interpreter | Interpreter | Open | Open | Open |
| Interpreter | Alpha | Both | Open | Open |
| Interpreter | Linux | Both | Both | Both |
| Alpha | Interpreter | Refusal | Both | Both |
| Alpha | Alpha | Both | Both | Both |
| Alpha | Linux | Both | Both | Both |
| Linux | Interpreter | Both | Both | Both |
| Linux | Alpha | Both | Both | Both |
| Linux | Linux | Both | Both | Both |

The 3,748-byte positive returned exit 42 and 3,072 answer bytes. All complete
observations have SHA-256
`9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225`.
The 3,745-byte twin changed just the forwarding argument. The interpreter
returned `02 12 f0 07 00 00`. Both compiler targets stopped at their shared
frontend with the complete response `rejected 18 2032`. Each positive
compiled target ran the generated guest. [Source and oracle](../../../tests/bootstrap/independent/epsilon/enum_test.py)

For a quick review, start with the Linux Gamma row, which completed every
pair. The Alpha-hosted Gamma compiler itself runs as a tape on the same VM
as the seed. Those two Gamma compilers share a frontend. The
original Beta-written Gamma interpreter gives a separate implementation of
Gamma semantics, making its completed comparisons especially informative.
[Gamma implementations and shared source](../../../bootstrap/gamma/README.md)

On the four-byte control, the Alpha-hosted direct Delta interpreter running
the Epsilon interpreter produced the expected 53 bytes in 267.92 CPU seconds.
It also refused the full classifier’s wrong-base twin in 134.90 CPU
seconds. The positive column above always requires all 256 inputs.
The optional translated-Delta interpreter completed the pair for **all three**
Epsilon choices on original Gamma. Each generated guest ran all 256 inputs.
The table above uses the direct implementation for its Delta interpreter cells.
[Interpreted guest](interpreted-gamma-matrix.md#run-the-generated-evaluator-directly-on-gamma) · [Compiled Alpha and Linux guests](https://github.com/CathedralOS/Omega/pull/4649)

With Gamma compiled to Alpha and Delta interpreted directly, both compiled
Epsilon guests exited 42. The Alpha tape was 880,161 bytes and the Linux
executable was 468,679 bytes. Both replayed refusals came from the same
Delta interpreter, SHA-256 `4c76020e9678898c8f74804ed2966087dbb4043aea38c691314c44def9488047`.

The compiled Gamma hosts used the toolchain and classifier from
`8da0d6a9f82`. The Linux-hosted direct-Delta run used `da31300384f` with
the same classifier, Delta implementations and Epsilon sources. Its
3,077-byte positive observation took 3,655.678 seconds in the nested
interpreters on the shared host. Each refusal replay checked the original
positive producer and complete framed-input digests before invoking that
producer with the twin. The original Gamma interpreter’s Linux-Delta result
has its own frozen inventory at `5144ec25e81` and two independently
executed EEREQ requests. [Original-interpreter receipts](interpreted-gamma-matrix.md)

The original Gamma interpreter also built the 2,204,117-byte Alpha evaluator
from the sources in change #4593. All three Gamma hosts emitted the same tape.
It ran the all-byte classifier and its wrong-base twin on the seed.
[Three-host comparison](interpreted-gamma-matrix.md#original-gamma-builds-the-classifiers-alpha-evaluator)

On the `93ad7259148` bootstrap tree, original Gamma also ran the Delta
compiler on the complete Epsilon compiler, emitting 813,584 bytes of
Gamma (`da44deee6b51…`) in 720.37 CPU seconds. On the linked
classifier, the same Delta compiler emitted 170,803 bytes of Gamma
(`99befce038db…`). The original interpreter then ran both Gamma backends,
producing an 880,161-byte Alpha tape (`d260052ecef6…`) and a 468,679-byte
Linux executable (`fc95d11d86c8…`). Both ran all 256 inputs. A complete
row also requires the Epsilon compiler that produced the linked classifier
and its observed refusal of the twin.

The independently constructed Linux Epsilon compiler from `5144ec25e81`
emitted the same 65,013-byte classifier module (`bd1d5bd47e66…`).
Fresh runs for the Alpha and Linux target selections both returned
`rejected 18 2032`. This snapshot has a different storage runtime, so
the original Gamma interpreter lowered the 161,036-byte linked guest
to 170,792 bytes of Gamma (`fc0b8d972176…`). A complete
row also requires the target backend and an execution of its output.

The Linux executable (468,679 bytes, `fc95d11d86c8…`) and Alpha tape (880,161 bytes, `d260052ecef6…`) ran the classifier on all 256 inputs and matched the full 3,077-byte observation. The Linux executable matches the one built from the 93ad sources byte for byte, although the two Gamma inputs differ by 11 bytes in length.

To repeat the experiment with the pinned compiler sources, use a fresh
checkout of the positive runner and select the two test files that added the
named refusal. A recent checkout also runs both names against its own sources.
Each output directory keeps the row receipts. [Route runner](../../../tests/bootstrap/route.py)

```sh
git worktree add --detach /tmp/omega-classifier-pair 8da0d6a9f82
cd /tmp/omega-classifier-pair
git restore --source=4a238010cd3 -- tests/bootstrap/route.py tests/bootstrap/independent/epsilon/enum_test.py
for program in omega-byte-classes omega-wrong-base; do
  python3 tests/bootstrap/route.py "/tmp/result-$program" \
    --gamma native --delta native --epsilon all --case "$program" --reuse-stages --seconds 7200
done
```

### Public Omega lexer

The same route selector can run the production scanner on whole source
fragments. Select a case with `--case` and keep each case’s output directory.

| Case | What the route must observe |
| --- | --- |
| `omega-lexer` | One scan at a chosen cursor and whole-source validation on 23 inputs, including nested comments, UTF-8, escapes and malformed strings. Exit 42 and 460 result bytes. |
| `omega-wrong-offset` | Change the caller from `scan_at(source, offset)` to `scan_at(source, source)`. The frontend must report type error 18 at the call head. |
| `omega-punctuation` | Scan each of 45 spellings at end of input and between letters. Emit the token’s extent and the spelling of the punctuation kind the scanner actually selected. |

The frozen 58,127-byte lexer and its wrong-cursor twin have complete
observations on **8 routes**. This measures the full scanner separately from
the byte classifier above. The 465-byte positive observation has SHA-256
`7d7a561d217ec1ce599a203fb8290b907c2582883c98dc71541c5bd0633f31aa`.
For this source the interpreter returns `02 12 94 e2 00 00` and the compiler
returns `rejected 18 58004`. A dash means this comparison has no result.

| Gamma | Delta | Epsilon interpreter | Epsilon to Alpha | Epsilon to Linux |
| --- | --- | --- | --- | --- |
| Interpreter | Alpha | Both | — | — |
| Interpreter | Linux | Both | Refusal | Refusal |
| Alpha | Alpha | Both | Refusal | Refusal |
| Alpha | Linux | Both | Refusal | Refusal |
| Linux | Alpha | Both | Refusal | Refusal |
| Linux | Linux | Both | Both | Both |

The two original-Gamma rows reused their previously constructed Alpha and
Linux evaluators. Both returned the complete 311-byte punctuation observation, SHA-256
`a59a295c5dbbaabad2dc98346265ed39109a4d2c63c621e6d870252676448c2a`.
[Alpha producer](https://github.com/CathedralOS/Omega/pull/4628) · [Linux producer](https://github.com/CathedralOS/Omega/pull/4640) · [Public scanner observations](https://github.com/CathedralOS/Omega/pull/4645)

[Public lexer cases](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py) ·
[Punctuation observations](../../../tests/bootstrap/independent/epsilon/lexer_punctuation_main.epsilon)

On the merged quoted scanner at `8f650128e09`, all three Epsilon choices
also passed the 90 punctuation scans with Gamma and Delta compiled to
Linux. The two compiler runs produced the same Delta module
(`3ec1904bd969…`). Its 3,687,166-byte Alpha tape and 3,524,692-byte
Linux executable both returned the interpreter’s 311 bytes.
[Shared writer and executable comparison](https://github.com/CathedralOS/Omega/pull/4648)

To reproduce the earlier numeric scanner, use the bootstrap at
`8da0d6a9f82` and the route tests at `e478143a55b`. Select
`--case omega-lexer` and `--case omega-wrong-offset` in separate runs.
The two 58,127-byte sources have SHA-256 prefixes `09f682a2abdf` and
`38425e9c5e2e`. Each takes the same 148-byte input.

## Tested threads through the matrix

Read each row from left to right after the common Alpha and Beta steps.

| Gamma runs the Delta implementation on | How the Epsilon tool runs | Epsilon program runs as | Result |
| --- | --- | --- | --- |
| Alpha and Linux | Direct Delta interpreter | Interpreted Epsilon | Both hosts passed five language checks and two syntax refusals. On Linux the four Omega lexer predicates matched on six input bytes. [Run](../../../tests/bootstrap/interpreted/delta/interpreter.py) |
| Interpreter | Interpreted Gamma | Interpreted Epsilon | Four lexer predicates matched on six inputs. Portable Omega compilation reached 300 s. [Run](../../../tests/bootstrap/interpreted/epsilon/README.md) |
| Alpha | Alpha | Interpreted Epsilon | The wire decoder and `Request.clear` compiled and ran. [Run](compiled-alpha.md#compare-a-production-request-operation-across-evaluator-hosts) |
| Linux | Linux | Interpreted Epsilon | Both Omega programs matched the Alpha-hosted builds byte for byte. [Run](gamma-hosts.md) |
| Linux | Alpha and Linux, compared | Alpha tape | Both Epsilon compilers emitted identical Delta for the string scanner. The string scanner passed 17 cases. [Run](native.md#public-omega-lexer-observations) |
| Linux | Linux | Alpha tape | The full public lexer passed 23 cases on the unchanged seed with a 7,342,855-byte tape. [Run](compiled-alpha.md#run-the-public-omega-lexer-on-alpha) |
| Linux | Linux | Alpha tape | The Omega state-signature checker accepted a unit signature and rejected a malformed return type. Its 15,398,168-byte tape matched the evaluator. [Checker](../../../bootstrap/omega/compiler/scalar_compilation.epsilon) |
| Linux | Linux | Linux executable | The string scanner passed 17 cases and the full public lexer passed 23. [Run](native.md#public-omega-lexer-observations) |
| Linux | Linux | Linux executable | The Epsilon-written Omega compiler built the two-package wire decoder. Its 1,607-byte Alpha tape matched the interpreter and exited 42 on the seed. [Whole compiler](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#whole-compiler-result) |

The compiled scanner runs reused source-built Linux tools to turn emitted Delta into
Gamma. The Gamma backends then built the scanner programs. Sharing the Alpha stack operations and the Epsilon compiler's outcome handling both reduced the tape size.

Running the 256-byte input through the direct Delta interpreter exceeded a
120-second wall limit on a shared Linux host. The same six-byte input as the Gamma-interpreted comparison (`20 30 5f 61 46 ff`) completed in
34 seconds, returning exit 42 and the expected 24 result bytes. Both requests
used the same Delta and Epsilon sources. The portable Omega compilation request reached the 300-second limit with empty output on this direct route. [Lexer source](../../../bootstrap/omega/compiler/source-to-tokens/byte_classes.epsilon) ·
[Test entry](../../../tests/bootstrap/independent/epsilon/lexical_main.epsilon)

The [interpreted route](interpreted.md), [compiled Alpha route](compiled-alpha.md) and [native route](native.md) record the construction steps and limits. The [Gamma host comparison](gamma-hosts.md) gives commands to reproduce the four host/target combinations.

## What the comparisons exercise

- The trusted starting artifacts are the Alpha VM and the admitted Beta tape.
  Beta rebuilds its own tape and compiles the Gamma interpreter. The reference
  route runs that interpreter to construct a Gamma compiler. The separately
  selectable Python accelerator supplies Gamma semantics and is excluded from
  these seed-built results. [Seed audit](../../../bootstrap/beta/AUDIT.md) ·
  [Cold build](../../../tests/bootstrap/linux_x86_64/README.md)
- The four Gamma host/target combinations compile the same Delta translator.
  Equality compares the hosts and emitters. All four still share Delta's
  checking and lowering. On the older interpreted path, a Gamma interpreter
  runs both the Delta translator and the Gamma it emits.
  [Gamma comparison](gamma-hosts.md) · [Delta source](../../../bootstrap/delta/README.md)
- The Epsilon compiler emits a Delta module and selects its runtime. The
  scanner tests build that module, run its Epsilon entry, and compare its
  observations to the evaluator. Some older scalar tests invoke individual
  generated functions through a test-written Delta driver. Those runs cover the
  called functions only. [Scanner tests](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py) ·
  [Function tests](../../../tests/bootstrap/independent/epsilon/compile_test.py)

## Whole Omega compiler

### Current 305-unit product

The Epsilon-written compiler with the quoted scanner and nested machine
scopes occupies 899,660 bytes with its construction entry (SHA-256
`2b30f2ed6238a98a885d7f86312e283d1742f443a1f6c72a410f81e1b5bc93de`).
The complete 305-unit input at `950caad840a` with the two length-policy corrections occupies
3,994,871 bytes (`f2a57db79ed0e90fd1bcacd9baa7df1e979094aa3e97cbd96bb04f7d7d65be65`).
[Compiler source](https://github.com/CathedralOS/Omega/pull/4646) · [Product input](https://github.com/CathedralOS/Omega/pull/4627)

The next product at `e8ef069cde7` changes five stores in `generics.omg`.
Its 305-unit input occupies 3,994,901 bytes, SHA-256
`4dd77c669bdd7efdc3834ffb37e2072e2ccae832c040f5bdd65c56b4dd570db5`.
Exercising the five stored values in isolation produced a 1,917-byte
Alpha tape that preserved 4,294,967,297 and exited 42. Restoring each
old conversion separately gave five type refusals. The native Rust peer
also checked the high bits and generation stamps against the original array
sizes. The full run of this later input is unfinished.
[Five stores, controls and product manifest](https://github.com/CathedralOS/Omega/pull/4652)

The follow-up compiler added checked reads and writes for bare `u64`
elements. The exact five right-hand sides with the original array
declarations produced a 2,467-byte Alpha tape that exited 42. Reverting
the handle conversion gave a refusal at byte 746. Its storage suite ran
16 tapes and checked 50 refusals or resets. At `76e6ca6e5ba` the complete
constructor source is 899,962 bytes (`9cc609b6e968…`). The whole-input
measurements below use the preceding 899,660-byte constructor.
[Indexed storage change](https://github.com/CathedralOS/Omega/pull/4654)

| Run on the seed-built toolchain | Observed result |
| --- | --- |
| Same compiler library with the public OCREQ entry, Alpha and Linux Epsilon evaluators | Both compiled `machine main() -> u8 { 42 }` to the same 22-byte tape. It exited 42 on Alpha. Both reported a corrupted OCREQ identity at byte 3. |
| Compile the complete 899,660-byte Epsilon constructor to Delta | The construction-profile Linux compiler emitted 18,248,100 bytes in 107.71 CPU seconds. Module SHA-256 `cebf0bfe41ec3c29f1a40cb3fcf9ae126086e46ecd20cca593f5a2222e4e9901`. Attaching the storage runtime produced 18,344,798 bytes of Delta. |
| Construction entry with a source view passed as its entry-unit number | The Linux Epsilon compiler returned `rejected 18 898277`. Its evaluator independently reported error 18 at the same call. |
| Entire 305-unit product through the Linux Epsilon evaluator | The 4,894,583-byte sealed request exceeded the conformance input limit and returned host status 253. The construction profile accepted its extent. A same-size damaged header returned a 40-byte `EEOUT` refusal. The valid request reached the 1.5 GiB process guard with empty output. |

An earlier complete 305-unit source-model run measured the effect of the
two length-policy changes alone. Its first unsupported construct moved
from byte 405,395 to byte 412,420, the nested helper at
`expressions.omg:366:9`. The subsequent compiler change resolved nested
machines by lexical scope. The seed-built evaluator then compiled the
shadowing case to a 523-byte tape that exited 42 and rejected access to a
sibling’s private helper at byte 154. [Complete input comparison](https://github.com/CathedralOS/Omega/pull/4627) · [Scope rules and controls](https://github.com/CathedralOS/Omega/pull/4644)

On that same 3,996,362-byte frozen product (`f240f7213f9b…`), the
899,660-byte compiler advanced another 9,592 source bytes. The complete
source-model run returned `00030000007c700600` after 494.61 CPU seconds.
Status 3 is the constructor’s `ResourceFailure`, at source byte 422,012
in the call to `payload_field`, `expressions.omg:546:72`. The nine-byte
receipt includes the source coordinate and carries no tape.

The separate full run on landed `950caad840a` returned the **same nine
bytes**, including the same coordinate, in 443.99 CPU seconds with peak
RSS of 1,018,824 KiB. The later five-store correction changes unit 19.
The measured stop on unit 17 precedes it in the input. With the earlier
units omitted, an eight-unit slice reached the second `as` in a
sequential cast at `operators.omg:195:90`. That reduced-input result
identifies a later language limitation. [Whole-input stopping point](https://github.com/CathedralOS/Omega/blob/950caad840a/source/subsystems/compiler/source_to_psi/pipeline/symbol_resolved_trees_to_typed_trees/expressions.omg#L546)

Tracing that exact 405-byte `payload_open` declaration identified one
resource check among the eleven parser states that visit the offending
`expression` token. It reserves a path member. Starting that check with
its table full reproduced `PathMembers` at the same token, requesting
entry 16,385 with a limit of 16,384. Filling the expression table instead
reported the following comma, ten bytes later. Only the path-member
control reproduced the whole-product coordinate. The whole-product result
itself records status and coordinate.
[Parser check](https://github.com/CathedralOS/Omega/blob/950caad840a/bootstrap/omega/compiler/parser.epsilon#L6148)

<details>
<summary>Reproduce the source-model inputs</summary>

At `950caad840a`, run `python3 source/application/construction/construct.py --input product.bin`
and pack `bootstrap/omega/compiler/omega_compiler.epsilon.sources` with
`xtask source-closure`. Append `bootstrap/omega/construct_omega.epsilon`
to the packed library. These commands reproduced both SHA-256 values above.
The independent development model starts from `8092cfd9c0a`. In
`parse_arm` and `probe_pattern_end`, change the integer-token predicate
from `token.kind == "int"` to `token.kind in ("int", "char")`. The resulting
file has SHA-256 `758286e8bfea3395414a673a0ba5228c761e94f06e88df7e0f5a889fc4a538c9`.
Pass the compiler bytes and `product.bin` to its `observation` function.
[Frozen model](https://github.com/CathedralOS/Omega/blob/8092cfd9c0a/tests/epsilon/refinement/model.py)

</details>

The public and construction entries exercise different inputs. On the public
entry frozen at #4634, the construction-profile Linux Epsilon compiler
also **compiled the 901,333-byte compiler program itself**. It emitted an
18,263,962-byte Delta module, SHA-256
`48f662488376b40d495ea747c152a87accc4aeca96ab652f12b3de0c3d912cfa`.
The Delta compiler translated that module and its storage runtime into
26,367,941 bytes of Gamma (SHA-256 `718ee1a63a37ba76f314f860833bacc19ae58120eb1d404661ca1c4b318fb2be`).
Both conformance hosts reached the 4 MiB output limit (status 254) on the
same source. A bad final call produced type error 18 at byte 900,855 on
both Alpha and Linux. [Public-entry receipts](source-audit.md#public-ocreq-on-alpha-and-linux) · [Construction replay](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#build-from-the-seed)

### Earlier whole-compiler builds

On the 852,096-byte compiler and construction entry, the Alpha-hosted and
Linux-hosted Epsilon compilers emitted identical Delta. The compact Gamma
backend then built a Linux executable that compiled the portable Omega program
and the two-package wire decoder. [Compared artifacts](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#whole-compiler-result)

With named payload bindings and zero omissions, the chain built a
58,490,251-byte Linux executable. It matched the independent source model
on 24 payload cases, including nine programs that ran on Alpha. On all 304
units, the native executable reached the 10 GiB memory guard while reading
its input. The source model advanced from the renamed binding in
`expressions.omg` at byte 87,497 to the qualified array element in
`scalar.omg` at byte 116,239. [Payload results](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#named-payload-bindings)

### Fitting the compiler and its product into one request

The 305-unit product occupied 3,996,333 bytes. Adding the 889,999-byte
Omega compiler and its 52-byte Epsilon request header brought the input to
**4,886,384 bytes**. The Delta profile determines whether the Epsilon
evaluator receives that input. Both source-built evaluators, one running
on Alpha and one on Linux, gave the following results on a guest that
read and echoed its first input byte. [Delta byte profiles](../../../bootstrap/delta/LANGUAGE.md#compiler-application-profile)

| Complete request | Conformance | Construction |
| --- | --- | --- |
| 4,194,304 bytes | Echoed Z | Echoed Z |
| 4,194,305 bytes | Exit 253 | Echoed Z |
| 4,886,384 bytes | Exit 253 | Echoed Z |
| 8,388,608 bytes | Exit 253 | Echoed Z |
| 8,388,609 bytes | Exit 253 | Exit 253 |

Every echo produced exactly `005a0000005a` (guest exit 90 and byte Z).
Exit 253 produced no output. With a bad first header byte at the product’s
full size, both construction evaluators returned the same 40-byte
`EEOUT` refusal as on a 52-byte input. This distinguishes a Delta input
limit from an Epsilon header refusal. The actual product request returned exit 253 with empty output on both
conformance targets. Its SHA-256 was `65852bc6f3bff86e6743fa264194a002f67c1b1a1f819ccb67daafb8e0daa2c6`.
[Omega constructor](../../../source/application/construction/construct.py) ·
[Epsilon request format](../../../bootstrap/epsilon/interpreter/EVALUATOR_ENTRY.md)

<details>
<summary>Repeat the full-size echo on both hosts</summary>

From `9df6f79be79`, create the guest and payload below. The test guest comes
from the portable suite. Padding brings its complete request to exactly the
size of the compiler and product above.

```sh
work=$(mktemp -d)
PYTHONPATH=tests/bootstrap python3 - "$work" <<'PYTEST'
from pathlib import Path
import sys
from conformance.vectors import epsilon_program_cases
w = Path(sys.argv[1])
_, source, _, _ = next(c for c in epsilon_program_cases() if c[0] == "stdin")
(w / "echo.epsilon").write_bytes(source)
(w / "stdin").write_bytes(b"Z" + bytes(4886384 - 52 - len(source) - 1))
PYTEST
for delta in alpha native; do
  python3 tests/bootstrap/route.py "$work/$delta" --gamma native --delta "$delta" \
    --epsilon interpret --profile construction --source "$work/echo.epsilon" \
    --stdin "$work/stdin" --seconds 7200
done
```

The construction profile emitted 718,770 bytes of Gamma, SHA-256
`f49b1b93c26c1f04827d90f9da2d547497f044cf71640d709c29e2dfb47f86d6`.
That source produced a 2,208,546-byte Alpha tape (`727575d9cb20`) and a
1,957,092-byte Linux executable (`5b17b0fb5fcf`). The conformance build
reproduced the 2,204,117-byte Alpha and 1,953,223-byte Linux evaluator
from the same Delta program. [Matrix runner](../../../tests/bootstrap/route.py)

</details>

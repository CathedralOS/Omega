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
Epsilon guest on Alpha and Linux. This brings the stdin control to **21/27**
on this source tree. [Receipts](interpreted-gamma-matrix.md#replay-the-compiled-guest)

### Omega compiler byte classifier

**18/27 routes** ran the classifier and refused its wrong-type twin.

The positive program runs the production lexer’s byte classifier on all 256
byte values and checks twelve answers per byte. Its twin passes an integer to
a parameter requiring the numeric-base enum. **Both** means the route ran the
positive and its frontend refused the twin at byte 2032. **Positive** records
the successful execution alone. **Open** means this comparison has no
completed positive receipt for that cell.

| Gamma | Delta | Epsilon interpreter | Epsilon to Alpha | Epsilon to Linux |
| --- | --- | --- | --- | --- |
| Interpreter | Interpreter | Open | Open | Open |
| Interpreter | Alpha | Open | Open | Open |
| Interpreter | Linux | Both | Open | Open |
| Alpha | Interpreter | Open | Both | Both |
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

# Bootstrap experiments

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
own Epsilon program and raw input. Use `--plan` to inspect the stages without
running them. The plan shows which compiler runs, where it runs, and which
runtime handoff is required. Interpreted Delta defaults to the direct interpreter.
Add `--delta-interpreter gamma` to translate Delta and interpret the emitted
Gamma, as in the original interpreted route. [Route runner](../../../tests/bootstrap/route.py)

Each run needs a new output directory and the repository's built `xtask` for
source manifests. Each row retains its source, input, intermediate artifacts,
and `result.json` with the stages, times, input hashes, generated artifact hashes and outcome. In a batch, each row logs live progress in `stages.log`. Completed
rows also retain `guest.eeout` for byte comparison. A timeout records the
stage in the row report and exits 124. Other failures exit 1. Batch runs finish the other rows
and display the Epsilon choices side by side. Completed custom runs must agree on the observation hash.

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

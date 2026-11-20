# Bootstrap experiments

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
used the same Delta and Epsilon sources. The portable Omega compilation request reached the 300-second limit with empty output on this direct route. [Lexer source](../../../bootstrap/omega/compiler/lexical_classification.epsilon) ·
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

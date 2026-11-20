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

On Alpha and Linux the Epsilon compiler produced the same Delta module from
the complete 852,096-byte Omega bootstrap compiler and construction entry.
With its storage runtime, the Delta program occupies 18,365,364 bytes. Delta checked
the entire program and published 26,398,215 bytes of Gamma. The compact
Linux backend built the 56,933,914-byte executable. It compiled the portable
Omega program and the two-package wire decoder into 50-byte and 1,607-byte
Alpha tapes. Both tapes matched the Epsilon evaluator and exited 42 on the seed. [Compiled route and byte identities](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#whole-compiler-result)

The product input contains 304 Omega source units in seven packages. Its
3,955,617 bytes become a 4,807,765-byte request when sent with the Epsilon
compiler source to the evaluator. Both original conformance targets refused
that packet at their 4 MiB input limit. The construction-profile Linux
evaluator reached the guest on a same-length malformed request. The earlier
480-second run on the full valid input produced no compiler outcome.
[Constructor input](../../../source/application/construction/construct.py) · [Interpreted measurements](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)

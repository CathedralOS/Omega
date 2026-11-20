# Bootstrap experiments

## Tested threads through the matrix

Read each row from left to right after the common Alpha and Beta steps.

| Gamma runs the Delta compiler on | Delta runs the Epsilon tool on | Epsilon program runs as | Result |
| --- | --- | --- | --- |
| Interpreter | Interpreted Gamma | Interpreted Epsilon | Four lexer predicates matched on six inputs. Portable Omega compilation reached 300 s. [Run](../../../tests/bootstrap/interpreted/epsilon/README.md) |
| Alpha | Alpha | Interpreted Epsilon | The wire decoder and `Request.clear` compiled and ran. [Run](compiled-alpha.md#compare-a-production-request-operation-across-evaluator-hosts) |
| Linux | Linux | Interpreted Epsilon | Both Omega programs matched the Alpha-hosted builds byte for byte. [Run](gamma-hosts.md) |
| Linux | Alpha and Linux, compared | Alpha tape | Both Epsilon compilers emitted identical Delta for the string scanner. The tape passed 17 cases. [Run](native.md#public-omega-lexer-observations) |
| Linux | Linux | Linux executable | The string scanner passed 17 cases and the full public lexer passed 23. [Run](native.md#public-omega-lexer-observations) |

The scanner runs reused source-built Linux tools to turn emitted Delta into
Gamma. The Gamma backends then built the actual scanner programs. The full
public scanner exceeded the Alpha tape limit.

The [interpreted route](interpreted.md), [compiled Alpha route](compiled-alpha.md) and [native route](native.md) record the construction steps and limits. The [Gamma host comparison](gamma-hosts.md) gives commands to reproduce the four host/target combinations.

## Whole Omega compiler

The complete construction input used here contains 304 Omega source units
in seven packages. Its 3,955,617 bytes fit the constructor's source-input
provision. Packaging it with the Epsilon-written bootstrap compiler for the
evaluator takes 4,807,765 bytes. Both stock targets returned outer status
253 with empty output at the Delta adapter's 4 MiB input check. A native
evaluator rebuilt with an 8 MiB input limit reached the constructor on a
same-size control. The real request ran for 480 s on that evaluator and
published neither an artifact nor a compiler diagnostic before the watchdog.
[Product input](../../../source/application/construction/construct.py) · [Evaluator experiment](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)

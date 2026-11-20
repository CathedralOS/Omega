# Epsilon

| How an Epsilon program runs | Implementation | Demonstrated workload |
| --- | --- | --- |
| Interpret | The [Delta-written Epsilon evaluator](interpreter/README.md) executes Epsilon source. Delta can itself run through interpreted Gamma, Alpha tape or Linux native code. | All three evaluator routes rebuilt the same Gamma for the evaluator and passed its portable language checks. |
| Compile to Alpha | The [Epsilon-to-Delta compiler](to-delta/) emits Delta. The Delta compiler emits Gamma, then the Gamma Alpha backend emits tape. | The 17-case string scanner fits in the Alpha seed. The full public lexer exceeds its tape limit. |
| Compile to native | The same Epsilon-to-Delta compiler and Delta compiler feed the Gamma Linux backend. | The 17-case string scanner and the 23-case public lexer run on Linux x86-64. |

The two compiled outcomes share the [target runtime](to-delta/runtime/). The Alpha- and Linux-hosted Epsilon compilers emitted identical Delta for the string scanner. [Language](LANGUAGE.md) · [Interpreter tests](../../tests/bootstrap/conformance/README.md) · [Compiled scanner results](../routes/native.md#public-omega-lexer-observations)

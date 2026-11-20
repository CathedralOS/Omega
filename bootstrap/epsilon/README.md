# Epsilon

| How an Epsilon program runs | Implementation | Demonstrated workload |
| --- | --- | --- |
| Interpret | The [Delta-written Epsilon evaluator](interpreter/README.md) executes Epsilon source. The [direct Delta interpreter](../delta/interpreter/) can run the evaluator. The Delta compiler can also translate it to Gamma for interpreted, Alpha or Linux execution. | The directly interpreted and translated evaluators both passed the five portable language checks. |
| Compile to Alpha | The [Epsilon-to-Delta compiler](compiler/) emits Delta. The Delta compiler emits Gamma, then the Gamma Alpha backend emits tape. | The 17-case string scanner and the 23-case public lexer both run on the unchanged Alpha seed. |
| Compile to native | The same Epsilon-to-Delta compiler and Delta compiler feed the Gamma Linux backend. | The 17-case string scanner and the 23-case public lexer run on Linux x86-64. |

The [interpreter entry](interpreter.delta) and [compiler](compiler/) both consume Epsilon source. The compiled Alpha and Linux programs share the [target runtime](compiler/runtime/). The Alpha- and Linux-hosted Epsilon compilers emitted identical Delta for the string scanner. [Language](LANGUAGE.md) · [Interpreter tests](../../tests/bootstrap/conformance/README.md) · [Compiled scanner results](../../wiki/development/bootstrap/native.md#public-omega-lexer-observations)

# Delta

Delta has two Gamma-written implementations. They share the source checker.
The interpreter evaluates the checked Delta program. The compiler emits a
Gamma program and leaves the choice of machine code to the Gamma backend.

| Run a Delta program | Construction |
| --- | --- |
| Interpret | The [Delta interpreter](interpreter/) evaluates its expressions, calls and matches. |
| Compile to Alpha | The [Delta compiler](compiler/) emits Gamma. The [Alpha backend](../gamma/compiler/alpha/) emits tape for the seed VM. |
| Compile to Linux x86-64 | The same compiler emits Gamma. The [Linux backend](../gamma/compiler/linux_x86_64/) emits an executable. |

Another tested composition runs the emitted Gamma on the [Gamma interpreter](../gamma/interpreter/).
It exercises the compiler's translation and provides a comparison with direct
Delta execution. The [Epsilon evaluator](../epsilon/interpreter/) is a Delta
customer on both paths. [Language](LANGUAGE.md) · [Tested routes](../../wiki/development/bootstrap/README.md)

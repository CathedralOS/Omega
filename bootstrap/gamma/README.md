# Gamma

| How a Gamma program runs | Source |
| --- | --- |
| Interpret | Beta compiles the [Gamma interpreter](interpreter/gamma_evaluator.beta) into Alpha tape. It reads a Gamma source program and its input. |
| Compile to Alpha | The [Alpha backend](compiler/alpha/alpha_backend.gamma) emits tape for the seed VM. |
| Compile to native | The [Linux x86-64 backend](compiler/linux_x86_64/native.gamma) emits ELF executables. |

Both compilers use the same [frontend](compiler/frontend.gamma) and [lowering](compiler/compiler.gamma). The interpreter can run the compiler's Gamma source to build its first executable. Once built, either host compiler can construct either target backend. [Language](LANGUAGE.md) · [Host and target measurements](../README.md#gamma-host-and-target-comparison)

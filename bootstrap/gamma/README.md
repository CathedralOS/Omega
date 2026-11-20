# Gamma

| How a Gamma program runs | Source |
| --- | --- |
| Interpret | Beta compiles the [Gamma interpreter](interpreter/gamma_evaluator.beta) into Alpha tape. It reads a Gamma source program and its input. |
| Compile to Alpha | The [Alpha backend](compiler/alpha/alpha_backend.gamma) emits tape for the seed VM. |
| Compile to native | The [Linux x86-64 backend](compiler/linux_x86_64/native.gamma) emits ELF executables. |

Both compilers use the same [frontend](compiler/frontend.gamma) and [lowering](compiler/compiler.gamma). The interpreter can run the compiler's Gamma source to build its first executable. Once built, either host compiler can construct either target backend. [Language](LANGUAGE.md) · [Host and target measurements](../../wiki/development/bootstrap/gamma-hosts.md)

The Alpha compiler emits plain instructions whenever they fit the 16,777,212-byte
stamp. For larger code it can put an Alpha decoder, an instruction dictionary
and the encoded program in that same stamp. The decoder expands the program
between the output buffer and pair heap, then runs it with the guest's stdin
at its original position. The expanded code is bounded at 128 MiB and the
complete tape must still fit the seed. [Format and replay](../../wiki/development/bootstrap/compiled-alpha.md#running-the-whole-omega-constructor-on-one-tape)

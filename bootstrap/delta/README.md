# Delta

The [Delta compiler](to-gamma/README.md) is written in Gamma. It translates a Delta program into Gamma source. All three outcomes use that compiler and its [runtime support](to-gamma/support/).

| How a Delta program runs | What consumes the emitted Gamma |
| --- | --- |
| Interpret | The [Gamma interpreter](../gamma/interpreter/README.md) executes it. |
| Compile to Alpha | The [Gamma-to-Alpha backend](../gamma/compiler/alpha/alpha_backend.gamma) produces a tape for the Alpha VM. |
| Compile to native | The [Gamma-to-Linux backend](../gamma/compiler/linux_x86_64/native.gamma) produces a Linux x86-64 executable. |

For example, the byte-string doubling program emits one 3,082-byte Gamma program. The comparison exercises its three outcomes on empty input, embedded NUL and every byte value. The larger [Epsilon evaluator](../epsilon/interpreter/README.md) is another Delta program. [Language](LANGUAGE.md) · [Comparison and commands](../README.md#gamma-host-and-target-comparison)

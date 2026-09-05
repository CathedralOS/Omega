# Gamma compiler

`gamma_compiler.gamma` is the former selected Gamma-to-Beta compiler. It validates
current Gamma source and emits canonical addressed Beta source, never Alpha
bytes. `gamma_compiler.beta` is its retained self-expansion receipt. The trusted
Beta compiler assembles that receipt into `gamma_compiler_bytecode.tape`.

The compiler uses two source passes. The first collects definitions, resolves
all reached words and transfer targets, assigns final Alpha addresses, and
preflights the 16,777,212-byte Alpha artifact maximum. The second emits
readable Beta mnemonics, numeric targets, address assertions, runtime helpers,
and source-definition comments. Generic Gamma output and Beta input admit
64 MiB so the textual receipt does not narrow the Alpha-output domain.

Generated Gamma programs use a separate 256 MiB runtime partition: 16 MiB
loaded tape, 16 MiB sealed input, 96 MiB explicit value stack, 80 MiB fixed
cells, 16 MiB reserved/output-word scratch, and 32 MiB for Alpha's hidden call
stack. The tape, input, stack, and cells therefore never borrow or overlap.

The native tape reconstructs without the former direct compiler:

```text
gamma_compiler.gamma
  -> Beta-authored Gamma evaluator
  -> gamma_compiler.beta
  -> Beta compiler
  -> gamma_compiler_bytecode.tape
```

Running the native tape on its own Gamma source reproduces the same Beta receipt,
which Beta assembles to the same native tape.

The former direct Gamma-to-Alpha compiler is test-owned at
[`../../../tests/gamma/gamma-to-beta-experiment/direct_compiler.gamma`](../../../tests/gamma/gamma-to-beta-experiment/direct_compiler.gamma)
as a differential comparator. It agrees with the selected route on the retained
corpus and a 16,777,211-byte near-limit Alpha witness. It supplies no selected
bootstrap premise and may be deleted after stronger correspondence evidence.

The executable reconstruction gate is
[`../../../tests/gamma/compiler-fixed-point.sh`](../../../tests/gamma/compiler-fixed-point.sh).
The broader edge/profile gate is
[`../../../tests/gamma/gamma-to-beta-experiment/run.sh`](../../../tests/gamma/gamma-to-beta-experiment/run.sh).
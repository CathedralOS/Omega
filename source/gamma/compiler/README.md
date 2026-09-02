# Gamma self-host compiler experiment

`gamma_compiler.gamma` directly compiles concatenative Gamma source to Alpha
tape. It uses two source passes: the first assigns one native address per word;
the second emits a fixed runtime prefix and direct code.

The runtime prefix is emitted as readable mnemonic-level Alpha operations with
explicit output-position assertions; it is not a packed byte blob. Compiler
state uses named cell accessors. Syntax, `main`, builtins, and user words resolve
by exact length and source bytes rather than hash identity.

Lowering is intentionally simple:

- hexadecimal literal: `imm` plus shared stack push;
- builtin: call one fixed runtime helper;
- user word: Alpha `call`;
- `jump`: Alpha `jmp`;
- `branch yes no`: shared pop, `jnz yes`, then `jmp no`; and
- word end: Alpha `ret`.

`gamma_compiler_bytecode.tape` is the first-generation native compiler produced
by running the source under the Beta-authored Gamma evaluator. Running that tape
on the same source reproduces it byte-for-byte.

This is an experimental side artifact, not a selected bootstrap edge or an
admitted replacement for the 738-line evaluator. The compiler accepts the
canonical compiler/customer corpus but does not yet claim every profile-v2
source rejection or bounded-failure observation. Its fixed point measures
Gamma's ability to implement its own translation job.

The executable gate is
[`../../../tests/gamma/compiler-fixed-point.sh`](../../../tests/gamma/compiler-fixed-point.sh).
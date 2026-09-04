# Rung: Gamma

[Chain overview](../bootstrap_chain.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is a typed scalar/effect functional source-transformer language. Its
evaluator is written in trusted Beta and assembled to Alpha tape by the admitted
Beta compiler.

Gamma has explicitly typed scalar functions, lexical `let`, conditionals,
integer and character literals, forward calls, sealed input, indexed byte reads,
byte output, and immutable pairs. It excludes source-declared algebraic data, pattern matching, `Bytes`, higher-order
functions, polymorphism, modules, and ambient host access.

Its exact contract is
[`source/gamma/LANGUAGE.md`](../../../../source/gamma/LANGUAGE.md). Its customers
are the staged Delta compiler and explicitly justified small bootstrap tools.
The selected 1,398-line Beta evaluator assembles to a 7,303-byte tape and runs
the scalar/effect plus self-augmentation gates. It executes the unchanged
85-line source augmenter, produces its exact source receipt, and evaluates the
expanded program to byte 42.

Proper tail execution, static validation of unreachable bodies, private
pair-reference provenance, and profile-owned arithmetic traps are complete.
Its exact resource outcomes are closed. Beta-root audit and the Gamma derivation
checker remain separate admission obligations. The former
concatenative Gamma implementation is retained only under
`source/gamma/bootstrap/concatenative/`.

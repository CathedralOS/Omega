# Rung: Gamma

[Chain overview](../bootstrap_chain.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is a strict first-order functional S-expression calculus. Its evaluator
is written in trusted Beta and compiled to Alpha tape by the admitted Beta
compiler.

Gamma keeps checked `Int`, immutable `Bytes`, closed constructors, exhaustive
pattern matching, `if`, single-binding `let`, first-order calls, mutual
recursion, proper tail calls, bounded allocation profiles, sealed input, and
returned values. It excludes mutation, raw memory, closures, higher-order
values, macros, polymorphism, general GC, continuations, exceptions, packages,
interactive evaluation, and ambient effects.

Its exact contract is
[`source/gamma/LANGUAGE.md`](../../../../source/gamma/LANGUAGE.md). Its customers
are the Delta compiler and explicitly justified small bootstrap tools. The
Beta-authored evaluator has a 42-case development slice; general calls,
constructors, `match`, proper tail calls, and its derived tape remain open.

# Rung: Beta

[Chain overview](../bootstrap_chain.md) | Prev: [Alpha](alpha.md) | Next: [Gamma](gamma.md)

Beta is a strict first-order functional S-expression calculus. One directly
audited Alpha evaluator tape executes Beta source; the evaluator is part of the
root rather than the product of another language rung.

Beta keeps checked `Int`, immutable `Bytes`, closed constructors, exhaustive
pattern matching, `if`, single-binding `let`, first-order calls, mutual
recursion, proper tail calls, bounded allocation profiles, sealed input, and
returned values. It excludes mutation, raw memory, closures, higher-order
values, macros, polymorphism, general GC, continuations, exceptions, packages,
interactive evaluation, and ambient effects.

Its exact contract is [`source/beta/LANGUAGE.md`](../../../../source/beta/LANGUAGE.md).
Its customers are the Gamma compiler and explicitly justified small bootstrap
tools. The evaluator tape and Gamma compiler source are currently absent.

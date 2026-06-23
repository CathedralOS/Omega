# Rung: Beta — names and structure

[Lattice overview](../bootstrap_lattice.md) | Prev: [Alpha](alpha.md) | Next: [Gamma](gamma.md)

Beta turns "painful raw machine construction" into something a human can inspect,
without yet adding an elaborate semantic theory. It is the first rung that exists
as an alpha *program* rather than native code.

## Adds

- labels instead of numeric jump offsets
- structured procedures and a basic stack convention
- canonical lists and tagged records
- a symbolic, textual representation

## Written in

Alpha. Beta's assembler/interpreter is an alpha program (a tape).

## Meaning

A beta program means what a **beta interpreter/assembler written in alpha** does
with it. (Today this is realized as an assembler that lowers beta text to an alpha
tape; the [lattice overview](../bootstrap_lattice.md) prefers a reference
*interpreter* as the definition, with the assembler/compiler demoted to an
acceleration — see [Honest edges / gamma reconciliation](../bootstrap_lattice.md#honest-edges).)

## Must not contain

No algebraic data types, no pattern matching, no type system, no proofs. Beta is
structure over raw alpha, not yet safe definitional computation — that is
[Gamma](gamma.md).

## Current repo reality

`compiler/beta/` is an assembler written in alpha-asm (`assembler.alpha`, ~683
lines): it reads human mnemonics, emits a tape, and memcpys it into a seed to make
a standalone executable. It **self-hosts**: beta assembles its own source to a
byte-identical fixed point (`beta1 == beta2`). A throwaway Rust on-ramp
(`beta-rs`) only cold-starts the very first beta.

Note: the byte-identical fixed point proves **consistency + provenance**, not
faithfulness — a Thompson seed reproduces identically too. It is a determinism
precondition for future Diverse Double-Compiling, not a correctness result. See
the [trust ledger](../bootstrap_lattice.md#the-irreducible-trust-ledger).

## Calling convention (proven 2026-06-22)

The one thing that turns beta from "an assembler" into "a language": a **frame
discipline**. The assembler dodged it (state survives a `call` only via fixed
global addresses → no recursion). The convention now exists and is verified on the
seed — **two stacks**: control on the VM's hidden return-address stack (`call`/
`ret`), data on an explicit program-managed stack (`r15` = data stack pointer,
grows down). `r0`–`r3` args, `r0` return, `r0`–`r5` caller-saved, `r6`–`r14`
callee-saved. Recursion proven: `factorial(5) → 120`, `fib(10) → 55` (tree
recursion). Spec + worked examples:
[`compiler/beta/CALLING_CONVENTION.md`](../../../../compiler/beta/CALLING_CONVENTION.md).
This is the foundation a procedure construct lowers to.

## Open questions

- Interpreter-first vs assembler-first: should beta's *definition* be a reference
  interpreter (with the assembler as an accelerator checked against it), per the
  lattice's meaning-by-interpreter principle?
- What structured-data and calling-convention surface gamma actually needs from
  beta (drive it from gamma's needs, not speculation).

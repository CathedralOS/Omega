# Rung: Alpha

[Chain overview](../bootstrap_chain.md) | Next: [Beta](beta.md)

Alpha is the unchanged deterministic tape executor and the only per-platform
native rung. [`source/alpha/SEMANTICS.md`](../../../../source/alpha/SEMANTICS.md)
defines its 21 instructions, bounded flat memory, byte I/O, calls, halt, and
trap.

Alpha contains no textual language, type system, theorem prover, compiler
framework, or higher-language primitive. Every compiler artifact is raw Alpha
tape. Host stamping is packaging, not compilation.

The selected root directly admits one future Beta evaluator tape and audits it
against Beta semantics. Alpha Tape Assembly is useful off-chain tooling; its
readable reconstruction does not become a language edge.

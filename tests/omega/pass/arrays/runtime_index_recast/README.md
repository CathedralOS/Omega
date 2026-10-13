# runtime_index_recast

`&mut main.bytes[main.position] as &mut RawView` — the recast origin is a
runtime-indexed array element; the byte at a checked index views through
the plan-laid `SignedByte<PortableByte>` record (`IntegerAt` signed-8).

Compile-checked only: `use omega::language::core::layout` is depend-free.

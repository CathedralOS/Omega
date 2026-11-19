# L5 Lowering

L5 turns an L4 prepared package into concrete, target-neutral Psi machines. L4 supplies the selected declaration, operand occurrences, frozen constants and closed signature types. L5 assigns identities to machines, types, blocks, operations, values, places and edges. The lowered-form checker owns the verdict about the resulting program. [Psi product](../../wiki/spec/terminal-psi/product.md) · [Control flow](../../wiki/spec/terminal-psi/control_flow.md)

## First handoff

The initial implementation lowers single-block machines with a unit return, a scalar constant or parameter return, or a fixed array returned by value from a parameter. The type table records the exact signedness and width of each integer and the element type and count at each array dimension. An absent result has no carrier. Machine declarations retain their source identities and effective contract ceilings. Typing supplies the published defaults. An omitted public crash clause and an explicit empty crash clause therefore produce the same ceiling. Numeric and Boolean constants define scalar values in the entry block. Recursive arrays of the supported scalars permit copying. Nothing in these types requires a target layout to lower. [Type table](src/types.rs) · [Entry blocks](src/body.rs)

A return carries the identity of its scalar value or structural source place. Two parameters of the same fixed-array type still have different places. For example, changing the body of `choose(a, b)` from `a` to `b` changes the source place on the return edge. Changing a static width instead changes the corresponding shape in the type table. The consumer can make both observations after the prepared trees and source text are dropped. [Handoff tests](tests/handoff.rs)

`omega source lower <package-relative files>` runs preparation and lowering and reports the first refusal with a source location. The Rust API returns the owned package. Its IDs are scoped to the produced package or machine. Artifact encoding, publication and execution are later consumers. [Source command](../../application/src/source.rs) · [Encoding](../../wiki/spec/terminal-psi/encoding.md)

## Open work

Runtime arithmetic and calls need ordered Psi operations, exact callee and operand identities, and the obligations attached to those operations. Branches need successor arguments and edge identities. Structural operations, loans and cleanup need paths and transfers, and contracts and target-sensitive folding need their corresponding evidence. The current entry point refuses declarations and bodies for which that information would be lost. A consumer that exposes a missing prepared fact should identify the specific occurrence and compare it with a working literal or directly selected control before reopening L4.

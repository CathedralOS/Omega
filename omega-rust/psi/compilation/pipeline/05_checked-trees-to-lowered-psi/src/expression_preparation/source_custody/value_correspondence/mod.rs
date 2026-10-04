//! Value correspondence complements the shared occurrence and read custody.
//! This matches retained operations; it never produces replacement expressions.
//! The caller rejoins namespaces, destinations, and call operand scopes. This
//! module preserves their literal/operator meaning. Terminal independently
//! proves emitted partial-operation obligations; checked range annotations are
//! not substituted for those proofs.
//! An eliminated subslice extent additionally needs statically established
//! formation bounds and an effect-free source. General slice-backed extents
//! need a retained view/bounds execution plan, which array operands do not yet
//! carry; matching literal endpoints alone never supplies that evidence.
//! Array elements and case fields share this receiving check.

mod extents;
mod integer_math;

// The observation vocabulary captures a place's authored spelling: fields
// name their member's symbol where the retained path keeps its spelling.

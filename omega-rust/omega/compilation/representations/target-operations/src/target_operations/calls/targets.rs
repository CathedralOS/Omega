//! Internal call control is independent of the requirement's data ABI.
//!
//! Dynamic selection retains the canonical interface and row. The selector is
//! one opaque word in ordinary value flow, never an added data parameter; the
//! requirement owns the call plan for every realization in its table.

/// Target carrier only; source selector identity stays in its checked operation.
pub fn selector_carrier_type() -> semantic_vocabulary::ScalarType {
    semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 carrier"),
    )
}

/// The native call target is the abstract call target verbatim.
pub type NativeCallTarget = abstract_operations::AbstractCallTarget;

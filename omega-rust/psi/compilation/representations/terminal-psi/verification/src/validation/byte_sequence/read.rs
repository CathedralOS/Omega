//! Exact byte reads, each an immutable scalar snapshot, with a direct,
//! dominating length witness.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, OperationKind, PlaceId, ScalarType, TerminalMachine,
    TerminalModule, ValueId,
};
pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let byte_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != byte_type)
    {
        return Err(ModuleError::ByteSequenceReadRequiresU8Result(operation.id));
    }
    // An exclusive loan that may be written may be read, under the same
    // custody rules its writes and length observations already use. A
    // write-only loan never takes this route.
    if terminal_semantics::arrival_parameter(machine, source)
        .is_some_and(|parameter| parameter.access == terminal_psi::StructuralAccess::MutableBorrow)
    {
        crate::validation::byte_sequence::write::validate_destination(
            module, machine, operation, source,
        )
        .map_err(|_| ModuleError::InvalidByteSequenceReadSource {
            operation: operation.id,
            source,
        })?;
    } else {
        crate::validation::byte_sequence::length::validate_source(
            module,
            machine,
            operation,
            source,
            || ModuleError::InvalidByteSequenceReadSource {
                operation: operation.id,
                source,
            },
        )?;
    }
    // The operand pass separately checks dominance and exact u64 types. A
    // parameter, alias, or merely equal integer cannot replace this producer.
    if !is_current_length(module, machine, operation, source, length) {
        return Err(ModuleError::InvalidByteSequenceReadLength {
            operation: operation.id,
            source,
            length,
        });
    }
    Ok(())
}

pub(in crate::validation) fn is_exact_length(
    machine: &TerminalMachine,
    source: PlaceId,
    length: ValueId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|candidate| {
            candidate
                .result
                .scalar()
                .is_some_and(|result| result.id == length)
                && matches!(candidate.kind,
                    OperationKind::ByteSequenceLength { source: measured } if measured == source)
        })
}

/// A matching measurement producer is insufficient after an owner replacement.
/// Reuse the all-path mutation fence rather than treating capacity as extent.
pub(in crate::validation) fn is_current_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> bool {
    let Some(producer) = machine.blocks.iter().flat_map(|block| &block.operations).find(|candidate| {
        candidate.result.scalar().is_some_and(|result| result.id == length)
            && matches!(candidate.kind, OperationKind::ByteSequenceLength { source: measured } if measured == source)
    }) else { return false; };
    crate::validation::structural::byte_sequence_fields::view_length_is_current(
        module,
        machine,
        producer.id,
        operation.id,
        source,
    )
}

/// Classify this immutable scalar snapshot, not the source's future contents.
/// The read still owes its independent indexing proof. UTF-8 alone supplies no
/// per-byte bound: a valid multi-byte encoding may contain bytes above 127.
pub(crate) fn class_bounds(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<Vec<semantic_vocabulary::Proposition>> {
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
    use terminal_psi::ByteSequencePredicate;

    let OperationKind::ByteSequenceRead { source, length, .. } = operation.kind else {
        return None;
    };
    validate(module, machine, operation, source, length).ok()?;
    let result = operation.result.scalar_ref()?;
    let ScalarType::Integer(integer) = result.scalar_type else {
        return None;
    };
    let predicates = terminal_semantics::byte_sequence_source_predicates(module, machine, source)?;
    let declares = |predicate| predicates.predicates().any(|present| present == predicate);
    let value = ScalarTerm::value(result.id, result.scalar_type);
    let literal = |byte| ScalarTerm::Integer {
        scalar_type: integer,
        value: IntegerValue::Unsigned(byte),
    };
    let mut bounds = Vec::new();
    if declares(ByteSequencePredicate::NoNul) {
        bounds.push(Proposition::LessOrEqual(literal(1), value.clone()));
    }
    if declares(ByteSequencePredicate::AsciiOnly) {
        bounds.push(Proposition::LessOrEqual(value, literal(127)));
    }
    Some(bounds)
}

//! Exact immutable byte reads with a direct, dominating length witness.

use super::*;

pub(super) fn validate(
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
    super::byte_sequence_length::validate_source(module, machine, operation, source, || {
        ModuleError::InvalidByteSequenceReadSource {
            operation: operation.id,
            source,
        }
    })?;
    // The operand pass separately checks dominance and exact u64 types. A
    // parameter, alias, or merely equal integer cannot replace this producer.
    let exact_length = machine
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
        });
    if !exact_length {
        return Err(ModuleError::InvalidByteSequenceReadLength {
            operation: operation.id,
            source,
            length,
        });
    }
    Ok(())
}

//! Evidence replay passes for object construction. Every retained
//! machine-code record is independently re-decoded from final bytes here;
//! no producer-supplied offset, peak, or shape crosses into the artifact
//! unverified. Installation-record validation reuses the same passes.

pub(crate) mod boundary;
pub(crate) mod instruction_loads;
pub(crate) mod scalar;
pub(crate) mod structural;
pub(crate) mod unit;

fn x86_register_info_writes_stack_pointer(info: &iced_x86::InstructionInfo) -> bool {
    info.used_registers().iter().any(|register| {
        matches!(
            register.register(),
            iced_x86::Register::RSP
                | iced_x86::Register::ESP
                | iced_x86::Register::SP
                | iced_x86::Register::SPL
        ) && matches!(
            register.access(),
            iced_x86::OpAccess::Write
                | iced_x86::OpAccess::CondWrite
                | iced_x86::OpAccess::ReadWrite
                | iced_x86::OpAccess::ReadCondWrite
        )
    })
}

pub(crate) fn exact_attribution_count(
    attribution: &[machine_code::SemanticCodeAttribution],
    operation: semantic_vocabulary::OperationId,
    ordinal: usize,
    interval: Option<(usize, usize)>,
) -> usize {
    attribution
        .iter()
        .filter(|row| {
            row.site == machine_code::SemanticCodeSite::Operation(operation)
                && row.operation_ordinal == ordinal
                && interval.is_none_or(|(offset, count)| {
                    row.code_offset == offset && row.byte_count == count
                })
                && (interval.is_some() || row.byte_count == 0)
        })
        .count()
}

pub(crate) fn zero_code_store_source_is_consistent(
    stores: &[machine_code::UnitWriteOnlyPrimitiveStoreRecord],
    source: machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord,
) -> bool {
    let Some(defining_operation) = source.defining_operation() else {
        return false;
    };
    let source_value = source.source_value();
    stores.iter().all(|candidate| {
        let candidate_source = candidate.source;
        if candidate_source.defining_operation() == Some(defining_operation)
            || candidate_source.source_value() == source_value
        {
            candidate_source == source
        } else {
            true
        }
    })
}

pub(crate) fn zero_code_field_store_source_is_consistent(
    stores: &[machine_code::UnitStructuralScalarFieldStoreRecord],
    source: machine_code::InternalUnitScalarArgumentSourceRecord,
) -> bool {
    let machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
        defining_operation,
        source_value,
        ..
    } = source
    else {
        return false;
    };
    stores.iter().all(|candidate| {
        let candidate_source = candidate.source;
        if matches!(
            candidate_source,
            machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                defining_operation: candidate_operation,
                source_value: candidate_value,
                ..
            } if candidate_operation == defining_operation || candidate_value == source_value
        ) {
            candidate_source == source
        } else {
            true
        }
    })
}

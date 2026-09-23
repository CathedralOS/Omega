//! Declaration-owned payload validity precedes atomic case establishment.

use super::{
    BTreeMap, BTreeSet, ModuleError, OperationKind, ScalarType, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralPlaceKind, StructuralTypeShape,
    TerminalMachine, TerminalModule, ValueId,
};
use terminal_psi::{RecordFieldValue, StructuralFieldDeclaration};

/// Every payload member of the selected case must carry an authored runtime
/// field: a relevant declaration, a scalar carrier, or a constructible owned
/// structural subtree. Erased proof members never reach the establishment.
fn constructible_case(module: &TerminalModule, fields: &[StructuralFieldDeclaration]) -> bool {
    fields.iter().all(|field| {
        field.relevance == terminal_psi::BindingRelevance::Relevant
            && match field.field_type {
                StructuralFieldType::Structural(child) => {
                    super::record::constructible_type(module, child)
                        || super::references::referent(module, child).is_some()
                }
                _ => field.field_type.scalar_type().is_some(),
            }
    })
}

/// Resolve the declaration roster a complete structural-case establishment
/// binds, after validating the result contract, the selected case and every
/// field correspondence. Returns the selected case's field declarations.
pub(crate) fn fields<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<&'a [StructuralFieldDeclaration], ModuleError> {
    let failure = || ModuleError::StructuralCaseResultMismatch(operation.id);
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    let exact_place = machine.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind,
            StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
    });
    let owned = matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    );
    if !owned
        || !exact_place
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(failure());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == result.structural_type)
        .ok_or_else(failure)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(ModuleError::StructuralCaseRequiresSum {
            operation: operation.id,
            structural_type: result.structural_type,
            result_case: *result_case,
        });
    };
    let member_failure = || ModuleError::StructuralCaseFieldMismatch {
        operation: operation.id,
        structural_type: result.structural_type,
        result_case: *result_case,
    };
    let selected = cases
        .iter()
        .find(|case| case.id == *result_case)
        .ok_or_else(member_failure)?;
    if selected.fields.len() != fields.len() || !constructible_case(module, &selected.fields) {
        return Err(member_failure());
    }
    for (declaration, binding) in selected.fields.iter().zip(fields) {
        if declaration.id != binding.field {
            return Err(member_failure());
        }
        match (&declaration.field_type, &binding.value) {
            (StructuralFieldType::Structural(expected), RecordFieldValue::Structural(argument)) => {
                let source =
                    super::structural_result_contracts::source_signature(machine, argument.place)
                        .ok_or_else(member_failure)?;
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == result.place
                    || source.structural_type != *expected
                    || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                    || !matches!(
                        source.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    || (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.multiplicity != StructuralMultiplicity::Unrestricted)
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.place == argument.place
                                && parameter.access != StructuralAccess::Owned
                        })
                    || machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|producer| {
                            producer.result.structural().is_some_and(|source| {
                                source.place == argument.place && !source.claims.is_empty()
                            })
                        })
                {
                    return Err(member_failure());
                }
            }
            (
                field_type,
                RecordFieldValue::Scalar {
                    range_obligation, ..
                },
            ) if field_type.scalar_type().is_some() => {
                if matches!(field_type, StructuralFieldType::BoundedInteger(_))
                    != range_obligation.is_some()
                {
                    return Err(member_failure());
                }
            }
            _ => return Err(member_failure()),
        }
    }
    Ok(&selected.fields)
}

/// Scalar payload operands are ordinary defined values whose runtime scalar
/// types match their declarations; structural children were consumed at the
/// establishment and need no value operand here.
pub(super) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let declarations = fields(module, machine, operation)?;
    let OperationKind::EstablishStructuralCase { fields, .. } = &operation.kind else {
        unreachable!()
    };
    for (declaration, binding) in declarations.iter().zip(fields) {
        if let RecordFieldValue::Scalar { value, .. } = binding.value {
            super::operations::require_defined(value, value_types, defined)?;
            if declaration.field_type.scalar_type() != value_types.get(&value).copied() {
                return Err(ModuleError::StructuralCaseResultMismatch(operation.id));
            }
        }
    }
    Ok(())
}

/// A completed structural-case establishment or a structural call returning a
/// plain-owned sum is a claim-free return source: the whole owned payload owes
/// no disposal and mints no frontier claims.
pub(super) fn plain_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: semantic_vocabulary::PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| {
            // A scalar-case establishment may produce a payloadless case of a Sum
            // whose other cases carry structural data (e.g. `Outcome::Declined`);
            // `scalar_case::plain_return_source` only admits all-scalar types, so
            // plain-owned Sum shapes route through this predicate as well.
            matches!(
                operation.kind,
                OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::EstablishScalarCase { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            ) && operation.result.structural().is_some_and(|result| {
                result.place == source
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && super::structural_result_contracts::has_plain_owned_shape(
                        module,
                        result.structural_type,
                    )
            })
        })
}

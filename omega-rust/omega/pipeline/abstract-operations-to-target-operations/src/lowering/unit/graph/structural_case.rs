//! Sum inspection retains declared order, layout, and edge-produced parameters.
use super::super::super::shared::*;
use super::LiveDefinitions;
use target_operations::{
    TargetScalarBlockValue, TargetUnitGraphCasePayload, TargetUnitGraphCaseSuccessor,
    TargetUnitTerminator,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    live: &LiveDefinitions,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetUnitTerminator, LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInUnitFunction(function.machine);
    let AbstractOperation::StructuralCase { source, cases } = operation else {
        return Err(invalid());
    };
    let home = live.structural_homes.get(source).ok_or_else(invalid)?;
    let declaration = structural_types
        .get(&home.result.structural_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Sum {
        cases: declared_cases,
    } = &declaration.shape
    else {
        return Err(invalid());
    };
    let layout = home.layout.sum().ok_or_else(invalid)?;
    if cases.len() != declared_cases.len() || cases.len() != layout.cases.len() {
        return Err(invalid());
    }
    let mut lowered = Vec::with_capacity(cases.len());
    for (case_ordinal, (case, declared)) in cases.iter().zip(declared_cases).enumerate() {
        let target = function
            .block_entries
            .iter()
            .find(|block| block.block == case.target)
            .ok_or_else(invalid)?;
        if case.case != declared.id
            || !target.structural_parameters.is_empty()
            || case.payloads.len() != target.parameters.len()
        {
            return Err(invalid());
        }
        // This checks the supported cleanup carrier, not ownership liveness.
        // The validated abstract graph remains the custody authority.
        let mut discards = BTreeSet::new();
        for place in &case.trivial_affine_discards {
            let discarded = live.structural_homes.get(place).ok_or_else(invalid)?;
            if !discards.insert(*place)
                || discarded.result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !discarded.result.claims.is_empty()
                || !discarded.result.qualifications.is_empty()
                || !discarded.result.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
        }
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if fields.len() != layout.cases[case_ordinal].fields.len() {
            return Err(invalid());
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&target.parameters) {
            let field_ordinal = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or_else(invalid)?;
            let field = fields[field_ordinal];
            let ScalarType::Integer(integer_type) = payload.scalar_type else {
                return Err(invalid());
            };
            let shape = crate::lowering::scalar_abi::fixed_native_integer_shape(integer_type)
                .ok_or_else(invalid)?;
            let field_layout = layout.cases[case_ordinal].fields[field_ordinal];
            if field.field_type != StructuralFieldType::Scalar(payload.scalar_type)
                || field_layout.shape != shape
                || parameter.value != payload.parameter
                || parameter.scalar_type != payload.scalar_type
            {
                return Err(invalid());
            }
            payloads.push(TargetUnitGraphCasePayload {
                field: payload.field,
                field_byte_offset: u32::from(field_layout.byte_offset),
                parameter: TargetScalarBlockValue {
                    block: target.block,
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            });
        }
        lowered.push(TargetUnitGraphCaseSuccessor {
            psi_edge: case.psi_edge,
            case: case.case,
            case_tag: i32::try_from(case_ordinal).map_err(|_| invalid())?,
            target: case.target,
            payloads,
            trivial_affine_discards: case.trivial_affine_discards.clone(),
        });
    }
    provenance
        .edges
        .extend(cases.iter().map(|case| case.psi_edge));
    Ok(TargetUnitTerminator::StructuralCase {
        source: home.clone(),
        cases: lowered,
    })
}

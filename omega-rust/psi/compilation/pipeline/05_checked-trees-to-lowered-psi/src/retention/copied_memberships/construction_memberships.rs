//! Complete formations after their actual operands acquire delayed payload rows.
//! View backing analysis can complete a source after initial emission. References
//! must retain every exclusive preservation debt; constructors must then retain
//! those same rows under their actual child coordinates. This fixed point adds
//! only operand evidence, never a receiving annotation or a new qualification.
use super::*;
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

pub(super) fn retained(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<PayloadMembershipContract> {
    let result = operation.result.structural()?;
    let shape = &module.structural_type(result.structural_type)?.shape;
    let mut projected_qualifications = Vec::new();
    match &operation.kind {
        OperationKind::EstablishReference { source, .. } => {
            let root = descriptors::terminal_payload_contract(machine, source.place)?;
            let path = terminal_semantics::reference_source_storage_path(
                module.structural_types.iter(),
                root.carrier,
                &source.path,
            );
            projected_qualifications = terminal_semantics::reference_source_memberships(
                module.structural_types.iter(),
                root.carrier,
                &root.qualifications,
                &root.projected_qualifications,
                path,
                source.access,
            )
            .ok()?;
        }
        OperationKind::EstablishStructuralArray { elements } => {
            for (ordinal, argument) in elements.iter().enumerate() {
                append(
                    machine,
                    argument,
                    &[StructuralPathSegment::FixedIndex(
                        u64::try_from(ordinal).ok()?,
                    )],
                    &mut projected_qualifications,
                )?;
            }
        }
        OperationKind::EstablishRecord { fields } => {
            append_fields(
                machine,
                fields,
                shape.record_fields()?,
                &[],
                &mut projected_qualifications,
            )?;
        }
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } => {
            let common = shape.direct_fields().unwrap_or(&[]);
            let case = shape
                .sum_cases()?
                .iter()
                .find(|case| case.id == *result_case)?;
            let (common_values, case_values) = fields.split_at_checked(
                common
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .count(),
            )?;
            append_fields(
                machine,
                common_values,
                common,
                &[],
                &mut projected_qualifications,
            )?;
            append_fields(
                machine,
                case_values,
                &case.fields,
                &[StructuralPathSegment::Case(*result_case)],
                &mut projected_qualifications,
            )?;
        }
        _ => return None,
    }
    projected_qualifications.sort();
    projected_qualifications.dedup();
    Some(PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        carrier: result.structural_type,
        qualifications: Vec::new(),
        projected_qualifications,
    })
}

fn append_fields(
    machine: &terminal_psi::TerminalMachine,
    values: &[terminal_psi::RecordFieldInitializer],
    declarations: &[terminal_psi::StructuralFieldDeclaration],
    prefix: &[StructuralPathSegment],
    destination: &mut Vec<StructuralPathQualification>,
) -> Option<()> {
    let declarations = declarations
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    if values.len() != declarations.len() {
        return None;
    }
    for (value, declaration) in values.iter().zip(declarations) {
        if value.field != declaration.id {
            return None;
        }
        if let terminal_psi::RecordFieldValue::Structural(argument) = &value.value
            && matches!(
                declaration.field_type,
                terminal_psi::StructuralFieldType::Structural(_)
            )
        {
            let mut path = prefix.to_vec();
            path.push(StructuralPathSegment::Field(declaration.identity));
            append(machine, argument, &path, destination)?;
        }
    }
    Some(())
}

fn append(
    machine: &terminal_psi::TerminalMachine,
    argument: &terminal_psi::StructuralArgument,
    prefix: &[StructuralPathSegment],
    destination: &mut Vec<StructuralPathQualification>,
) -> Option<()> {
    let root = descriptors::terminal_payload_contract(machine, argument.place)?;
    let (whole, projected) = StructuralPathQualification::select(
        &root.qualifications,
        &root.projected_qualifications,
        &argument.path,
    );
    StructuralPathQualification::append_under(&whole, &projected, prefix, destination);
    Some(())
}

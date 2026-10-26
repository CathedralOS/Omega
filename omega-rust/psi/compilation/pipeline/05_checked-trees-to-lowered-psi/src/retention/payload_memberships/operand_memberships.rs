//! Complete operand transport after sources acquire delayed payload rows.
//! View backing analysis can complete a source after initial emission. References
//! retain every exclusive preservation debt, constructors retain actual child
//! rows, and detached moves select the exact declared window from their physical
//! source. This fixed point adds only source evidence, never a receiving
//! annotation, a current-value inference or a new qualification.
use super::*;
use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

pub(super) fn retained(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    position: (semantic_vocabulary::BlockId, u32),
    analysis: &mut descriptors::DescriptorAnalysis<'_>,
    history: &mut descriptors::PayloadMembershipHistory<'_>,
    memberships: &super::membership_reachability::MembershipReachability,
) -> Option<PayloadMembershipContract> {
    let result = operation.result.structural()?;
    if !memberships.may_contain(result.structural_type) {
        return None;
    }
    let shape = &module.structural_type(result.structural_type)?.shape;
    let mut projected_qualifications = Vec::new();
    match &operation.kind {
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => {
            let root = descriptors::terminal_payload_contract(machine, *source)?;
            let projection = terminal_semantics::structural_window_projection(
                module.structural_types.iter(),
                root.carrier,
                path,
                *field,
            )?;
            if projection.structural_type != result.structural_type {
                return None;
            }
            let (qualifications, projected_qualifications) =
                StructuralPathQualification::select_restorable(
                    &root.qualifications,
                    &root.projected_qualifications,
                    &projection.path,
                )?;
            return Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: result.structural_type,
                qualifications,
                projected_qualifications,
            });
        }
        OperationKind::EstablishReference { source, storage } => {
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
            // Private arrivals can predate late producer-membership completion.
            // Rejoin all actual declared backings without strengthening the
            // arrival's intended receiving contract or inferring current facts.
            if let Ok(captured) = descriptors::reference_backing_memberships(
                &module.structural_types,
                &module.structural_domains,
                &module.identities,
                analysis,
                machine.id,
                position,
                source,
                storage.as_ref(),
                result,
                |place| descriptors::terminal_payload_contract(machine, place),
            ) {
                projected_qualifications.extend(captured);
            }
            // Shared observation may capture predicates proved at this exact
            // occurrence. Exclusive capture retains declared restoration debts;
            // an incidental current value must not invent a new invariant that
            // an otherwise legal later mutation would have to preserve.
            if source.access == terminal_psi::StructuralAccess::SharedBorrow
                && memberships.may_contain(root.carrier)
                && let Ok(current) = history.current_selected_payload_contract_in(
                    analysis,
                    position,
                    source.place,
                    path,
                )
            {
                StructuralPathQualification::append_under(
                    &current.qualifications,
                    &current.projected_qualifications,
                    &[StructuralPathSegment::Referent],
                    &mut projected_qualifications,
                );
            }
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
    if projected_qualifications.is_empty() {
        return None;
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

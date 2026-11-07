//! Owning placement operations return the same backing descriptor they consume.
//!
//! The private operation supplies this relation, not an assumed source promise.
//! Successful initialization nests the original extent in its owner; rejection
//! returns it unchanged, and extraction returns that owner's extent. These are
//! scalar geometry facts only: neither equality issues a grant or transfers
//! content custody. Each fact is guarded by the exact completed result's case.

use super::call_preparation::PreparedNormalCall;
use super::{CapturedResultEquation, StructuralJudge, StructuralTerm, owner_symbol};
use crate::proof_contracts::contract_entailment::structural_judgment::conditional_guarantees::{
    ConditionalGuarantees, ObservedCondition,
};
use typed_trees::data::DataMember;
use typed_trees::typed_trees::{
    AccessContentShape, AccessExecutionDeclaration, AccessLifecycleOperation,
};

impl StructuralJudge<'_> {
    pub(super) fn completed_placement_geometry(
        &self,
        prepared: &PreparedNormalCall<'_, '_>,
        result: Option<&StructuralTerm>,
    ) -> Option<Vec<ConditionalGuarantees>> {
        let program = self.program;
        let Some(declaration) =
            AccessExecutionDeclaration::authenticate(program, owner_symbol(&prepared.owner))
                .ok()?
        else {
            return Some(Vec::new());
        };
        let outcomes = declaration.backing_outcomes();
        if outcomes.is_empty() {
            return (declaration.operation() == AccessLifecycleOperation::View).then(Vec::new);
        }
        let result @ StructuralTerm::CapturedValue { result_type, .. } = result? else {
            return None;
        };
        let result_type = program
            .type_reference_table
            .peeled_type_reference(*result_type);
        let definition = program
            .data_application_definition(result_type)
            .or_else(|| program.closed_data_definition(result_type))?;
        let mut guarantees = Vec::with_capacity(outcomes.len());
        for outcome in outcomes {
            let case = program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.name.as_str() == outcome.case => Some(case),
                    _ => None,
                })?;
            let mut equations = Vec::new();
            for transfer in outcome.transfers {
                let parameter = prepared.parameters.get(transfer.parameter)?.symbol;
                let source = &prepared
                    .bindings
                    .entry_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter)?
                    .1;
                for field in ["base", "length"] {
                    equations.push(CapturedResultEquation::public((
                        project(result, transfer.output, field),
                        project(source, transfer.input, field),
                    )));
                }
            }
            if declaration.operation() == AccessLifecycleOperation::Relocate
                && declaration.content() == AccessContentShape::Run
            {
                let parameter = prepared.parameters.first()?.symbol;
                let source = &prepared
                    .bindings
                    .entry_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter)?
                    .1;
                let owner = match outcome.case {
                    "Ready" => "view.run",
                    "Rejected" => "returned.source",
                    _ => return None,
                };
                equations.push(CapturedResultEquation::public((
                    project(result, owner, "count"),
                    project(source, "", "count"),
                )));
            }
            guarantees.push(ConditionalGuarantees {
                conditions: vec![(
                    ObservedCondition::Case {
                        subject: result.clone(),
                        data: definition.symbol,
                        case: case.symbol,
                    },
                    true,
                )],
                equations,
            });
        }
        Some(guarantees)
    }
}

fn project(subject: &StructuralTerm, owner: &str, field: &str) -> StructuralTerm {
    StructuralTerm::Projection {
        subject: Box::new(subject.clone()),
        path: if owner.is_empty() {
            field.to_owned()
        } else {
            format!("{owner}.{field}")
        },
    }
}

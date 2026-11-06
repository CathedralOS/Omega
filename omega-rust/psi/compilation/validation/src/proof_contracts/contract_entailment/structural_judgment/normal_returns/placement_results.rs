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
        let outcomes: &[(&str, &[(usize, &str, &str, &[&str])])] =
            match (declaration.operation(), declaration.content()) {
                (AccessLifecycleOperation::Initialize, AccessContentShape::Run) => &[
                    ("Ready", &[(0, "", "view.storage", &["base", "length"])]),
                    (
                        "Rejected",
                        &[(0, "", "returned.source", &["base", "length"])],
                    ),
                ],
                (AccessLifecycleOperation::Initialize, AccessContentShape::Resident) => &[
                    ("Ready", &[(0, "", "view", &["base", "length"])]),
                    (
                        "Rejected",
                        &[(0, "", "returned.source", &["base", "length"])],
                    ),
                ],
                (AccessLifecycleOperation::MoveOut, AccessContentShape::Run) => {
                    &[("Moved", &[(0, "storage", "storage", &["base", "length"])])]
                }
                (AccessLifecycleOperation::MoveOut, AccessContentShape::Resident) => {
                    &[("Moved", &[(0, "", "storage", &["base", "length"])])]
                }
                (AccessLifecycleOperation::View, _) => return Some(Vec::new()),
                (AccessLifecycleOperation::Relocate, AccessContentShape::Run) => &[
                    (
                        "Ready",
                        &[
                            (1, "", "view.run.storage", &["base", "length"]),
                            (0, "storage", "view.vacated", &["base", "length"]),
                            (0, "", "view.run", &["count"]),
                        ],
                    ),
                    (
                        "Rejected",
                        &[
                            (0, "storage", "returned.source.storage", &["base", "length"]),
                            (1, "", "returned.custody", &["base", "length"]),
                            (0, "", "returned.source", &["count"]),
                        ],
                    ),
                ],
                (AccessLifecycleOperation::Relocate, AccessContentShape::Resident) => return None,
            };
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
        for (case_name, coordinates) in outcomes {
            let case = program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.name.as_str() == *case_name => Some(case),
                    _ => None,
                })?;
            let mut equations = Vec::new();
            for (position, input_path, result_path, fields) in *coordinates {
                let parameter = prepared.parameters.get(*position)?.symbol;
                let source = &prepared
                    .bindings
                    .entry_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter)?
                    .1;
                for field in *fields {
                    equations.push(CapturedResultEquation::public((
                        project(result, result_path, field),
                        project(source, input_path, field),
                    )));
                }
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

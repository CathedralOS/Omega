//! Name and module resolution.
//!
//! This crate owns the first post-parse view of names. The current report is
//! intentionally shallow: it records imports, top-level definitions, and
//! syntactic references without pretending that every reference is fully bound.
//! That gives later phases a concrete spine to grow from.

use omega_ast::expression::Expression;
use omega_ast::item::{DataMember, Item, Machine, State, StateSignature};
use omega_ast::statement::{Statement, TransitionGuard, TransitionTarget};
use omega_ast::types::{TypeConstraint, TypeReference};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveReport {
    pub definitions: Arena<ResolvedDefinition>,
    pub imports: Arena<ResolvedImport>,
    pub references: Arena<ResolvedReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedDefinition {
    pub name: String,
    pub kind: ResolvedDefinitionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedDefinitionKind {
    Data,
    Invariant,
    Machine,
    Platform,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedImport {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedReference {
    pub name: String,
    pub kind: ResolvedReferenceKind,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedReferenceKind {
    CallTarget,
    ExpressionName,
    Invariant,
    StructLiteral,
    TransitionTarget,
    Type,
    #[default]
    Unknown,
}

pub fn build_resolve_report(items: &[Item]) -> ResolveReport {
    let mut report = ResolveReport::default();

    for item in items {
        match item {
            Item::Data(data_definition) => {
                report.definitions.insert(ResolvedDefinition {
                    name: data_definition.name.clone(),
                    kind: ResolvedDefinitionKind::Data,
                });

                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        collect_type_reference(
                            &mut report,
                            &field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Invariant(invariant) => {
                report.definitions.insert(ResolvedDefinition {
                    name: invariant.name.clone(),
                    kind: ResolvedDefinitionKind::Invariant,
                });

                collect_constraints(
                    &mut report,
                    &invariant.constraints,
                    &format!("invariant `{}`", invariant.name),
                );
            }
            Item::Use(use_item) => {
                report.imports.insert(ResolvedImport {
                    path: use_item.path.join("::"),
                });
            }
            Item::Machine(machine) => {
                report.definitions.insert(ResolvedDefinition {
                    name: machine.name.clone(),
                    kind: ResolvedDefinitionKind::Machine,
                });

                collect_machine_references(&mut report, machine);
            }
            Item::Platform(platform) => {
                report.definitions.insert(ResolvedDefinition {
                    name: platform.name.clone(),
                    kind: ResolvedDefinitionKind::Platform,
                });

                for state in &platform.states {
                    collect_state_signature_references(
                        &mut report,
                        state,
                        &format!("platform `{}` state `{}`", platform.name, state.name),
                    );
                }
            }
        }
    }

    report
}

fn collect_machine_references(report: &mut ResolveReport, machine: &Machine) {
    for contained_object in &machine.contains {
        insert_reference(
            report,
            &contained_object.type_name,
            ResolvedReferenceKind::Type,
            &format!(
                "machine `{}` contains `{}`",
                machine.name, contained_object.name
            ),
        );
    }

    for owned_data in &machine.owned_data {
        collect_type_reference(
            report,
            &owned_data.type_reference,
            &format!("machine `{}` owns `{}`", machine.name, owned_data.name),
        );

        if let Some(initial_value) = &owned_data.initial_value {
            collect_expression(
                report,
                initial_value,
                &format!(
                    "machine `{}` owned `{}` initializer",
                    machine.name, owned_data.name
                ),
            );
        }
    }

    for state in &machine.states {
        collect_state_references(report, machine, state);
    }
}

fn collect_state_references(report: &mut ResolveReport, machine: &Machine, state: &State) {
    collect_state_signature_parts(
        report,
        &state.parameters,
        state.return_type.as_ref(),
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in &state.statements {
        collect_statement(
            report,
            statement,
            &format!("machine `{}` state `{}`", machine.name, state.name),
        );
    }
}

fn collect_state_signature_references(
    report: &mut ResolveReport,
    state: &StateSignature,
    owner: &str,
) {
    collect_state_signature_parts(report, &state.parameters, state.return_type.as_ref(), owner);
}

fn collect_state_signature_parts(
    report: &mut ResolveReport,
    parameters: &[omega_ast::item::StateParameter],
    return_type: Option<&TypeReference>,
    owner: &str,
) {
    for parameter in parameters {
        collect_type_reference(
            report,
            &parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if let Some(return_type) = return_type {
        collect_type_reference(report, return_type, &format!("{owner} return type"));
    }
}

fn collect_statement(report: &mut ResolveReport, statement: &Statement, owner: &str) {
    match statement {
        Statement::Assignment(assignment) => {
            collect_expression(
                report,
                &assignment.target,
                &format!("{owner} assignment target"),
            );
            collect_expression(
                report,
                &assignment.value,
                &format!("{owner} assignment value"),
            );
        }
        Statement::Call(call) => {
            let target = call
                .receiver
                .as_ref()
                .map(|receiver| format!("{receiver}::{}", call.target))
                .unwrap_or_else(|| call.target.clone());

            insert_reference(report, &target, ResolvedReferenceKind::CallTarget, owner);

            for argument in &call.arguments {
                collect_expression(report, argument, &format!("{owner} call argument"));
            }
        }
        Statement::Expression(expression) => collect_expression(report, expression, owner),
        Statement::LocalData(local_data) => collect_type_reference(
            report,
            &local_data.type_reference,
            &format!("{owner} local `{}`", local_data.name),
        ),
        Statement::Transition(transition) => {
            collect_transition_target(report, &transition.target, owner);

            if let Some(continuation) = &transition.continuation {
                collect_transition_target(report, continuation, owner);
            }

            if let TransitionGuard::When(guard) = &transition.guard {
                collect_expression(report, guard, &format!("{owner} transition guard"));
            }
        }
    }
}

fn collect_transition_target(report: &mut ResolveReport, target: &TransitionTarget, owner: &str) {
    if let TransitionTarget::Named { path, arguments } = target {
        insert_reference(
            report,
            &path.join("::"),
            ResolvedReferenceKind::TransitionTarget,
            owner,
        );

        for argument in arguments {
            collect_expression(report, argument, &format!("{owner} transition argument"));
        }
    }
}

fn collect_type_reference(report: &mut ResolveReport, type_reference: &TypeReference, owner: &str) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference(report, base_type, owner);
            collect_constraints(report, constraints, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_type_reference(report, element_type, owner);
        }
        TypeReference::Named(name) => {
            insert_reference(report, name, ResolvedReferenceKind::Type, owner);
        }
    }
}

fn collect_constraints(report: &mut ResolveReport, constraints: &[TypeConstraint], owner: &str) {
    for constraint in constraints {
        match constraint {
            TypeConstraint::Named(name) => {
                insert_reference(report, name, ResolvedReferenceKind::Invariant, owner);
            }
            TypeConstraint::Range { minimum, maximum } => {
                collect_expression(report, minimum, &format!("{owner} range minimum"));
                collect_expression(report, maximum, &format!("{owner} range maximum"));
            }
        }
    }
}

fn collect_expression(report: &mut ResolveReport, expression: &Expression, owner: &str) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                collect_expression(report, value, owner);
            }
        }
        Expression::Binary(binary) => {
            collect_expression(report, &binary.left, owner);
            collect_expression(report, &binary.right, owner);
        }
        Expression::Indexed(indexed) => {
            collect_expression(report, &indexed.collection, owner);
            collect_expression(report, &indexed.index, owner);
        }
        Expression::Mutable(inner_expression) => {
            collect_expression(report, inner_expression, owner)
        }
        Expression::Name(path) => {
            insert_reference(
                report,
                &path.join("::"),
                ResolvedReferenceKind::ExpressionName,
                owner,
            );
        }
        Expression::StructLiteral(struct_literal) => {
            insert_reference(
                report,
                &struct_literal.type_name,
                ResolvedReferenceKind::StructLiteral,
                owner,
            );

            for field in &struct_literal.fields {
                collect_expression(report, &field.value, owner);
            }
        }
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => {}
    }
}

fn insert_reference(
    report: &mut ResolveReport,
    name: &str,
    kind: ResolvedReferenceKind,
    owner: &str,
) {
    report.references.insert(ResolvedReference {
        name: name.to_owned(),
        kind,
        owner: owner.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use omega_ast::item::{Contains, Item, Machine, OwnedData, State, StateParameter, UseItem};
    use omega_ast::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use omega_ast::types::TypeReference;

    use super::{ResolvedDefinitionKind, ResolvedReferenceKind, build_resolve_report};

    #[test]
    fn collects_definitions_imports_and_references() {
        let report = build_resolve_report(&[
            Item::Use(UseItem {
                path: vec!["platform".to_owned(), "console".to_owned()],
            }),
            Item::Machine(Machine {
                name: "main".to_owned(),
                contains: vec![Contains {
                    name: "console".to_owned(),
                    type_name: "Console".to_owned(),
                }],
                owned_data: vec![OwnedData {
                    name: "score".to_owned(),
                    type_reference: TypeReference::named("i32"),
                    initial_value: None,
                }],
                states: vec![State {
                    name: "entry".to_owned(),
                    parameters: vec![StateParameter {
                        name: "amount".to_owned(),
                        type_reference: TypeReference::named("i32"),
                        is_const: false,
                        is_mutable: false,
                        is_self: false,
                    }],
                    return_type: None,
                    statements: vec![Statement::Transition(Transition {
                        target: TransitionTarget::Named {
                            path: vec!["finish".to_owned()],
                            arguments: Vec::new(),
                        },
                        continuation: None,
                        guard: TransitionGuard::Always,
                    })],
                }],
            }),
        ]);

        assert_eq!(report.imports.len(), 1);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.references.len(), 4);

        let (_, definition) = report
            .definitions
            .iter()
            .find(|(_, definition)| definition.name == "main")
            .expect("main definition should be collected");
        assert_eq!(definition.kind, ResolvedDefinitionKind::Machine);

        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "finish"
                    && reference.kind == ResolvedReferenceKind::TransitionTarget
            }),
            "state transition target should be collected"
        );
    }
}

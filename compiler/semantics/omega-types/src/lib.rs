//! Type, ownership, mutability, and bounded-value analysis.
//!
//! This crate currently owns the source-facing type surface report. Deeper
//! ownership, borrow, and proof-aware type solving can grow here without living
//! inside the compiler orchestration crate.

use omega_abstract_syntax_tree::item::{
    CapabilityMember, DataMember, Item, Machine, State, StateParameter, StateSignature,
};
use omega_abstract_syntax_tree::types::{TypeConstraint, TypeReference};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeSurfaceReport {
    pub declarations: Arena<TypeDeclaration>,
    pub references: Arena<TypeReferenceUse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeDeclaration {
    pub name: String,
    pub kind: TypeDeclarationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeDeclarationKind {
    Capability,
    Data,
    Invariant,
    Library,
    Machine,
    Platform,
    State,
    Target,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeReferenceUse {
    pub name: String,
    pub kind: TypeReferenceUseKind,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeReferenceUseKind {
    Constraint,
    RangeConstraint,
    ReturnType,
    Storage,
    Parameter,
    #[default]
    Unknown,
}

pub fn build_type_surface_report(items: &[Item]) -> TypeSurfaceReport {
    let mut report = TypeSurfaceReport::default();

    for item in items {
        match item {
            Item::Capability(capability) => {
                insert_declaration(
                    &mut report,
                    &capability.name,
                    TypeDeclarationKind::Capability,
                );

                for member in &capability.members {
                    match member {
                        CapabilityMember::Field(field) => {
                            collect_type_reference(
                                &mut report,
                                &field.type_reference,
                                TypeReferenceUseKind::Storage,
                                &format!("capability `{}` field `{}`", capability.name, field.name),
                            );
                        }
                        CapabilityMember::State(state) => {
                            collect_state_signature(
                                &mut report,
                                &state.signature,
                                &format!(
                                    "capability `{}` state `{}`",
                                    capability.name, state.signature.name
                                ),
                            );
                        }
                    }
                }
            }
            Item::Data(data_definition) => {
                insert_declaration(
                    &mut report,
                    &data_definition.name,
                    TypeDeclarationKind::Data,
                );

                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        collect_type_reference(
                            &mut report,
                            &field.type_reference,
                            TypeReferenceUseKind::Storage,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Invariant(invariant) => {
                insert_declaration(&mut report, &invariant.name, TypeDeclarationKind::Invariant);

                collect_constraints(
                    &mut report,
                    &invariant.constraints,
                    &format!("invariant `{}`", invariant.name),
                );
            }
            Item::Library(library) => {
                if let Some(name) = &library.name {
                    insert_declaration(&mut report, name, TypeDeclarationKind::Library);
                }

                for function in &library.functions {
                    collect_state_signature(
                        &mut report,
                        &function.signature,
                        &format!(
                            "library `{}` function `{}`",
                            library
                                .name
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| library.path.clone()),
                            function.signature.name
                        ),
                    );
                }
            }
            Item::Use(_) => {}
            Item::Machine(machine) => collect_machine(&mut report, machine),
            Item::Platform(platform) => {
                insert_declaration(&mut report, &platform.name, TypeDeclarationKind::Platform);

                for state in &platform.states {
                    collect_state_signature(
                        &mut report,
                        state,
                        &format!("platform `{}` state `{}`", platform.name, state.name),
                    );
                }
            }
            Item::Target(target) => {
                insert_declaration(&mut report, &target.name, TypeDeclarationKind::Target);
            }
            Item::TrustDefinition(_) => {}
        }
    }

    report
}

fn collect_machine(report: &mut TypeSurfaceReport, machine: &Machine) {
    insert_declaration(report, &machine.name, TypeDeclarationKind::Machine);

    for contained_object in &machine.contains {
        insert_reference(
            report,
            &contained_object.type_name,
            TypeReferenceUseKind::Storage,
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
            TypeReferenceUseKind::Storage,
            &format!("machine `{}` owns `{}`", machine.name, owned_data.name),
        );
    }

    for state in &machine.states {
        collect_state(report, machine, state);
    }
}

fn collect_state(report: &mut TypeSurfaceReport, machine: &Machine, state: &State) {
    insert_declaration(
        report,
        &format!("{}::{}", machine.name, state.name),
        TypeDeclarationKind::State,
    );

    collect_state_parts(
        report,
        &state.parameters,
        state.return_type.as_ref(),
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );
}

fn collect_state_signature(report: &mut TypeSurfaceReport, state: &StateSignature, owner: &str) {
    collect_state_parts(report, &state.parameters, state.return_type.as_ref(), owner);
}

fn collect_state_parts(
    report: &mut TypeSurfaceReport,
    parameters: &[StateParameter],
    return_type: Option<&TypeReference>,
    owner: &str,
) {
    for parameter in parameters {
        collect_type_reference(
            report,
            &parameter.type_reference,
            TypeReferenceUseKind::Parameter,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if let Some(return_type) = return_type {
        collect_type_reference(
            report,
            return_type,
            TypeReferenceUseKind::ReturnType,
            &format!("{owner} return type"),
        );
    }
}

fn collect_type_reference(
    report: &mut TypeSurfaceReport,
    type_reference: &TypeReference,
    kind: TypeReferenceUseKind,
    owner: &str,
) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference(report, base_type, kind, owner);
            collect_constraints(report, constraints, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_type_reference(report, element_type, kind, owner);
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            insert_reference(report, base_name, kind, owner);

            for argument in arguments {
                collect_type_reference(report, argument, kind, owner);
            }
        }
        TypeReference::Named(name) => insert_reference(report, name, kind, owner),
        TypeReference::Unit => {}
    }
}

fn collect_constraints(
    report: &mut TypeSurfaceReport,
    constraints: &[TypeConstraint],
    owner: &str,
) {
    for constraint in constraints {
        match constraint {
            TypeConstraint::Named(name) => {
                insert_reference(report, name, TypeReferenceUseKind::Constraint, owner);
            }
            TypeConstraint::Range { minimum, maximum } => {
                insert_reference(
                    report,
                    &format!(
                        "range<{}, {}>",
                        minimum.display_name(),
                        maximum.display_name()
                    ),
                    TypeReferenceUseKind::RangeConstraint,
                    owner,
                );
            }
        }
    }
}

fn insert_declaration(report: &mut TypeSurfaceReport, name: &str, kind: TypeDeclarationKind) {
    report.declarations.insert(TypeDeclaration {
        name: name.to_owned(),
        kind,
    });
}

fn insert_reference(
    report: &mut TypeSurfaceReport,
    name: &str,
    kind: TypeReferenceUseKind,
    owner: &str,
) {
    report.references.insert(TypeReferenceUse {
        name: name.to_owned(),
        kind,
        owner: owner.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use omega_abstract_syntax_tree::identifier::Identifier;
    use omega_abstract_syntax_tree::item::{Item, Machine, OwnedData, State, StateParameter};
    use omega_abstract_syntax_tree::types::{TypeConstraint, TypeReference};
    use omega_core::source::SourceText;

    use super::{TypeDeclarationKind, TypeReferenceUseKind, build_type_surface_report};

    #[test]
    fn collects_state_signatures_and_constraints() {
        let report = build_type_surface_report(&[Item::Machine(Machine {
            name: Identifier::generated("main"),
            contains: Vec::new(),
            owned_data: vec![OwnedData {
                name: Identifier::generated("speed"),
                type_reference: TypeReference::Constrained {
                    base_type: Box::new(TypeReference::named("f32")),
                    constraints: vec![
                        TypeConstraint::Named(Identifier::generated("finite")),
                        TypeConstraint::Range {
                            minimum: omega_abstract_syntax_tree::expression::Expression::Float(
                                SourceText::generated("0.0f"),
                            ),
                            maximum: omega_abstract_syntax_tree::expression::Expression::Float(
                                SourceText::generated("100000.0f"),
                            ),
                        },
                    ],
                },
                initial_value: None,
            }],
            states: vec![State {
                name: Identifier::generated("entry"),
                parameters: vec![StateParameter {
                    name: Identifier::generated("value"),
                    type_reference: TypeReference::named("f32"),
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                }],
                return_type: Some(TypeReference::named("i32")),
                statements: Vec::new(),
            }],
        })]);

        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "main::entry" && declaration.kind == TypeDeclarationKind::State
            }),
            "state declaration should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "finite" && reference.kind == TypeReferenceUseKind::Constraint
            }),
            "constraint references should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "range<0.0f, 100000.0f>"
                    && reference.kind == TypeReferenceUseKind::RangeConstraint
            }),
            "range constraints should be recorded"
        );
        assert_eq!(report.references.len(), 5);
    }
}

//! Proof surface collection and invariant checking.
//!
//! This crate currently records the source-level invariant and bounded-type
//! sites that should eventually become proof obligations. The driver still owns
//! the deeper lowered proof plan until shared IR leaves `omega-driver`.

use omega_ast::item::{DataMember, Item, Machine, Platform, State, StateParameter, StateSignature};
use omega_ast::statement::Statement;
use omega_ast::types::{TypeConstraint, TypeReference};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofSurfaceReport {
    pub invariants: Arena<InvariantSurface>,
    pub bounded_sites: Arena<BoundedTypeSite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvariantSurface {
    pub name: String,
    pub constraints: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundedTypeSite {
    pub owner: String,
    pub base_type: String,
    pub constraints: String,
}

pub fn build_proof_surface_report(items: &[Item]) -> ProofSurfaceReport {
    let mut report = ProofSurfaceReport::default();

    for item in items {
        match item {
            Item::Capability(_) => {}
            Item::Data(data_definition) => {
                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        collect_bounded_type_site(
                            &mut report,
                            &field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Invariant(invariant) => {
                report.invariants.insert(InvariantSurface {
                    name: invariant.name.clone(),
                    constraints: constraints_name(&invariant.constraints),
                });
            }
            Item::Use(_) => {}
            Item::Machine(machine) => collect_machine(&mut report, machine),
            Item::Platform(platform) => collect_platform(&mut report, platform),
            Item::Target(_) | Item::TrustDefinition(_) => {}
        }
    }

    report
}

fn collect_machine(report: &mut ProofSurfaceReport, machine: &Machine) {
    for owned_data in &machine.owned_data {
        collect_bounded_type_site(
            report,
            &owned_data.type_reference,
            &format!("machine `{}` owns `{}`", machine.name, owned_data.name),
        );
    }

    for state in &machine.states {
        collect_state(report, machine, state);
    }
}

fn collect_state(report: &mut ProofSurfaceReport, machine: &Machine, state: &State) {
    collect_signature_parts(
        report,
        &state.parameters,
        state.return_type.as_ref(),
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in &state.statements {
        let Statement::LocalData(local_data) = statement else {
            continue;
        };

        collect_bounded_type_site(
            report,
            &local_data.type_reference,
            &format!(
                "machine `{}` state `{}` local `{}`",
                machine.name, state.name, local_data.name
            ),
        );
    }
}

fn collect_platform(report: &mut ProofSurfaceReport, platform: &Platform) {
    for state in &platform.states {
        collect_state_signature(
            report,
            state,
            &format!("platform `{}` state `{}`", platform.name, state.name),
        );
    }
}

fn collect_state_signature(report: &mut ProofSurfaceReport, state: &StateSignature, owner: &str) {
    collect_signature_parts(report, &state.parameters, state.return_type.as_ref(), owner);
}

fn collect_signature_parts(
    report: &mut ProofSurfaceReport,
    parameters: &[StateParameter],
    return_type: Option<&TypeReference>,
    owner: &str,
) {
    for parameter in parameters {
        collect_bounded_type_site(
            report,
            &parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if let Some(return_type) = return_type {
        collect_bounded_type_site(report, return_type, &format!("{owner} return type"));
    }
}

fn collect_bounded_type_site(
    report: &mut ProofSurfaceReport,
    type_reference: &TypeReference,
    owner: &str,
) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            report.bounded_sites.insert(BoundedTypeSite {
                owner: owner.to_owned(),
                base_type: type_reference_name(base_type),
                constraints: constraints_name(constraints),
            });
            collect_bounded_type_site(report, base_type, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_type_site(report, element_type, owner);
        }
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_type_site(report, argument, owner);
            }
        }
        TypeReference::Named(_) => {}
        TypeReference::Unit => {}
    }
}

fn type_reference_name(type_reference: &TypeReference) -> String {
    match type_reference {
        TypeReference::Constrained { base_type, .. } => type_reference_name(base_type),
        TypeReference::FixedArray {
            element_type,
            length,
        } => {
            format!("[{}; {length}]", type_reference_name(element_type))
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            let arguments = arguments
                .iter()
                .map(type_reference_name)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base_name}<{arguments}>")
        }
        TypeReference::Named(name) => name.clone(),
        TypeReference::Unit => "()".to_owned(),
    }
}

fn constraints_name(constraints: &[TypeConstraint]) -> String {
    if constraints.is_empty() {
        return "[]".to_owned();
    }

    let mut output = String::new();
    output.push('[');

    for (index, constraint) in constraints.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&constraint_name(constraint));
    }

    output.push(']');
    output
}

fn constraint_name(constraint: &TypeConstraint) -> String {
    match constraint {
        TypeConstraint::Named(name) => name.clone(),
        TypeConstraint::Range { minimum, maximum } => {
            format!(
                "range<{}, {}>",
                minimum.display_name(),
                maximum.display_name()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use omega_ast::item::{InvariantDefinition, Item, Machine, OwnedData};
    use omega_ast::types::{TypeConstraint, TypeReference};

    use super::build_proof_surface_report;

    #[test]
    fn collects_invariants_and_bounded_type_sites() {
        let report = build_proof_surface_report(&[
            Item::Invariant(InvariantDefinition {
                name: "speed_range".to_owned(),
                constraints: vec![TypeConstraint::Named("finite".to_owned())],
            }),
            Item::Machine(Machine {
                name: "main".to_owned(),
                contains: Vec::new(),
                owned_data: vec![OwnedData {
                    name: "speed".to_owned(),
                    type_reference: TypeReference::Constrained {
                        base_type: Box::new(TypeReference::named("f32")),
                        constraints: vec![TypeConstraint::Named("speed_range".to_owned())],
                    },
                    initial_value: None,
                }],
                states: Vec::new(),
            }),
        ]);

        assert_eq!(report.invariants.len(), 1);
        assert_eq!(report.bounded_sites.len(), 1);

        let (_, bounded_site) = report
            .bounded_sites
            .iter()
            .next()
            .expect("bounded site should be collected");
        assert_eq!(bounded_site.base_type, "f32");
        assert_eq!(bounded_site.constraints, "[speed_range]");
    }
}

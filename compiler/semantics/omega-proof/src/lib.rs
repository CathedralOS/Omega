//! Proof surface collection, proof obligation building, and invariant checking.

use omega_core::arena::Arena;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{DataMember, Item, Machine, Platform, State, StateSignature};
use omega_syntax_trees::types::{
    TypeConstraint, TypeConstraintNode, TypeReference, TypeReferenceHandle, TypeReferenceNode,
};

pub mod checker;
pub mod obligations;

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

pub fn build_proof_surface_report(syntax_trees: &SyntaxTrees) -> ProofSurfaceReport {
    let mut report = ProofSurfaceReport::default();

    for item in syntax_trees.root_items() {
        match item {
            Item::Capability(_) => {}
            Item::Data(data_definition) => {
                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        collect_bounded_type_site_tree(
                            &mut report,
                            &field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Invariant(invariant) => {
                report.invariants.insert(InvariantSurface {
                    name: invariant.name.to_string(),
                    constraints: constraints_name(&invariant.constraints),
                });
            }
            Item::Library(library) => {
                for function in &library.functions {
                    collect_state_signature(
                        &mut report,
                        syntax_trees,
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
            Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
            Item::Platform(platform) => collect_platform(&mut report, syntax_trees, platform),
            Item::Target(_) | Item::TrustDefinition(_) => {}
        }
    }

    report
}

fn collect_machine(report: &mut ProofSurfaceReport, syntax_trees: &SyntaxTrees, machine: &Machine) {
    for state in &machine.states {
        collect_state(report, syntax_trees, machine, state);
    }
}

fn collect_state(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    state: &State,
) {
    collect_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in syntax_trees.items.statements(state.statements) {
        let omega_syntax_trees::statement::StatementNode::LocalData(local_data) =
            syntax_trees.statements.statement(*statement)
        else {
            continue;
        };

        collect_bounded_type_site(
            report,
            syntax_trees,
            local_data.type_reference,
            &format!(
                "machine `{}` state `{}` local `{}`",
                machine.name, state.name, local_data.name
            ),
        );
    }
}

fn collect_platform(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    platform: &Platform,
) {
    for state in &platform.states {
        collect_state_signature(
            report,
            syntax_trees,
            state,
            &format!("platform `{}` state `{}`", platform.name, state.name),
        );
    }
}

fn collect_state_signature(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    state: &StateSignature,
    owner: &str,
) {
    collect_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        owner,
    );
}

fn collect_signature_parts(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    parameters: omega_core::arena::HandleSpan<omega_syntax_trees::item::StateParameterHandle>,
    return_type: TypeReferenceHandle,
    owner: &str,
) {
    for parameter in syntax_trees.items.state_parameters(parameters) {
        let parameter = syntax_trees.items.state_parameter(*parameter);
        collect_bounded_type_site(
            report,
            syntax_trees,
            parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if return_type.is_valid() {
        collect_bounded_type_site(
            report,
            syntax_trees,
            return_type,
            &format!("{owner} return type"),
        );
    }
}

fn collect_bounded_type_site(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    owner: &str,
) {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_type_site(report, syntax_trees, *referee, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            report.bounded_sites.insert(BoundedTypeSite {
                owner: owner.to_owned(),
                base_type: type_reference_name(syntax_trees, *base_type),
                constraints: constraint_handle_name(syntax_trees, *constraints),
            });
            collect_bounded_type_site(report, syntax_trees, *base_type, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_type_site(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_type_site(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_bounded_type_site(report, syntax_trees, *argument, owner);
            }
        }
        TypeReferenceNode::Named(_) => {}
        TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_type_site_tree(
    report: &mut ProofSurfaceReport,
    type_reference: &TypeReference,
    owner: &str,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collect_bounded_type_site_tree(report, referee, owner);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            report.bounded_sites.insert(BoundedTypeSite {
                owner: owner.to_owned(),
                base_type: type_reference_name_tree(base_type),
                constraints: constraints_name(constraints),
            });
            collect_bounded_type_site_tree(report, base_type, owner);
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_type_site_tree(report, element_type, owner);
        }
        TypeReference::Slice { element_type } => {
            collect_bounded_type_site_tree(report, element_type, owner);
        }
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_type_site_tree(report, argument, owner);
            }
        }
        TypeReference::Named(_) | TypeReference::Unit => {}
    }
}

fn type_reference_name(syntax_trees: &SyntaxTrees, type_reference: TypeReferenceHandle) -> String {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => {
            let qualifier = if *is_mutable { "mut " } else { "" };
            format!(
                "&{qualifier}{}",
                type_reference_name(syntax_trees, *referee)
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name(syntax_trees, *base_type)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            format!(
                "[{}; {length}]",
                type_reference_name(syntax_trees, *element_type)
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            format!("[{}]", type_reference_name(syntax_trees, *element_type))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => {
            let arguments = syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_name(syntax_trees, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base_name}<{arguments}>")
        }
        TypeReferenceNode::Named(name) => name.to_string(),
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn type_reference_name_tree(type_reference: &TypeReference) -> String {
    match type_reference {
        TypeReference::Reference {
            referee,
            is_mutable,
        } => {
            let qualifier = if *is_mutable { "mut " } else { "" };
            format!("&{qualifier}{}", type_reference_name_tree(referee))
        }
        TypeReference::Constrained { base_type, .. } => type_reference_name_tree(base_type),
        TypeReference::FixedArray {
            element_type,
            length,
        } => {
            format!("[{}; {length}]", type_reference_name_tree(element_type))
        }
        TypeReference::Slice { element_type } => {
            format!("[{}]", type_reference_name_tree(element_type))
        }
        TypeReference::Generic {
            base_name,
            arguments,
        } => {
            let arguments = arguments
                .iter()
                .map(type_reference_name_tree)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base_name}<{arguments}>")
        }
        TypeReference::Named(name) => name.to_string(),
        TypeReference::Unit => "()".to_owned(),
    }
}

fn constraint_handle_name(
    syntax_trees: &SyntaxTrees,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
) -> String {
    let constraints = syntax_trees.type_references.constraints(constraints);
    if constraints.is_empty() {
        return "[]".to_owned();
    }

    let mut output = String::new();
    output.push('[');

    for (index, constraint) in constraints.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&match constraint {
            TypeConstraintNode::Named(name) => name.to_string(),
            TypeConstraintNode::Range { minimum, maximum } => format!(
                "range<{}, {}>",
                syntax_trees
                    .expressions
                    .expression(*minimum)
                    .display_name(&syntax_trees.expressions),
                syntax_trees
                    .expressions
                    .expression(*maximum)
                    .display_name(&syntax_trees.expressions)
            ),
        });
    }

    output.push(']');
    output
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
        TypeConstraint::Named(name) => name.to_string(),
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
    use omega_core::arena::HandleSpan;
    use omega_syntax_trees::SyntaxTrees;
    use omega_syntax_trees::identifier::Identifier;
    use omega_syntax_trees::item::{InvariantDefinition, Item, Machine, State, StateParameterNode};
    use omega_syntax_trees::types::{TypeConstraint, TypeReference};

    use super::build_proof_surface_report;

    #[test]
    fn collects_invariants_and_bounded_type_sites() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let parameter_type = syntax_trees.type_references.insert_tree(
            &TypeReference::Constrained {
                base_type: Box::new(TypeReference::named("f32")),
                constraints: vec![TypeConstraint::Named(Identifier::generated("speed_range"))],
            },
            &mut syntax_trees.expressions,
        );
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("speed"),
                type_reference: parameter_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter_handle = syntax_trees.items.append_state_parameter_handle(parameter);

        syntax_trees.push_root_item(Item::Invariant(InvariantDefinition {
            name: Identifier::generated("speed_range"),
            constraints: vec![TypeConstraint::Named(Identifier::generated("finite"))],
        }));
        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            states: vec![State {
                name: Identifier::generated("entry"),
                parameters: HandleSpan::from_parts(parameter_handle, 1),
                return_type: omega_syntax_trees::types::TypeReferenceHandle::invalid(),
                statements: HandleSpan::empty(),
            }],
        }));

        let report = build_proof_surface_report(&syntax_trees);

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

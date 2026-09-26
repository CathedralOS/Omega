use super::error::DependencyProjectionError;
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME, constructor_parts};
use build_declarations::{DependencyOperation, is_dependency_call_name};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::{CapabilityMember, ConformanceBody, ConformanceMember, Item};
use syntax_trees::statement::{StatementHandle, StatementNode};

const BUILD_TYPE_NAME: &str = "Build";

pub(super) fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(item) {
            Some(name)
                if matches!(
                    machine_leaf_name(name),
                    BUILD_TYPE_NAME | SOURCE_TYPE_NAME | PACKAGE_SELECTION_TYPE_NAME
                ) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: machine_leaf_name(name).to_owned(),
                });
            }
            _ => {}
        }
        match item {
            Item::Machine(machine)
                if machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str() == BUILD_TYPE_NAME)
                    && is_dependency_call_name(machine_leaf_name(machine.name.as_str())) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: format!("Build::{}", machine.name.as_str()),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn package_authored_type_name(item: &Item) -> Option<&str> {
    match item {
        Item::Data(data) => Some(data.name.as_str()),
        Item::Domain(domain) => Some(domain.name.as_str()),
        Item::Trait(definition) => Some(definition.name.as_str()),
        _ => None,
    }
}

fn machine_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

/// Reject dependency vocabulary anywhere retained syntax can carry it.
///
/// Statements live in three places: machine states (root and conformance
/// member machines), trait requirement `default_body` rows, and capability
/// state signatures. `StatementNode::Call` is the only dependency-call shape
/// that is not also a retained expression, so the statement walk must visit
/// every container the parser can populate — depending on the expression
/// scan alone would silently admit a depend() row nested in a trait default
/// or conformance member the moment call conversion stops retaining its
/// expression.
///
/// `Source::` and `PackageSelection::` literals are toolchain vocabulary whose
/// only legal occurrence is inside a projected row: a source literal must be
/// a row's direct source argument, and a package-selection literal must be
/// the `selection` field of an accepted `Source::Git`. Every other occurrence
/// is an unprojected dependency shape.
pub(super) fn reject_unprojected_dependency_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_statements: &[StatementHandle],
    accepted_sources: &[ExpressionHandle],
    accepted_aliases: &[ExpressionHandle],
) -> Result<(), DependencyProjectionError> {
    let mut accepted_selections = Vec::new();
    for source_handle in accepted_sources {
        let ExpressionNode::StructLiteral(literal) =
            syntax_trees.expressions.expression(*source_handle)
        else {
            continue;
        };
        for field in syntax_trees.expressions.struct_fields(literal.fields) {
            if field.name.as_str() == "selection" {
                accepted_selections.push(field.value);
            }
        }
    }

    for item in syntax_trees.root_items() {
        match item {
            Item::Machine(machine) => {
                reject_machine_dependency_statements(syntax_trees, machine, accepted_statements)?;
            }
            Item::Trait(definition) => {
                for signature_handle in syntax_trees.items.state_signatures(definition.machines) {
                    let signature = syntax_trees.items.state_signature(*signature_handle);
                    reject_dependency_statements(
                        syntax_trees,
                        syntax_trees.items.statements(signature.default_body),
                        accepted_statements,
                    )?;
                }
            }
            Item::Conformance(conformance) => {
                let ConformanceBody::Closed { members } = &conformance.body else {
                    continue;
                };
                for member in syntax_trees.items.conformance_members(*members) {
                    match member {
                        ConformanceMember::Machine(machine)
                        | ConformanceMember::TraitDefault { machine, .. } => {
                            reject_machine_dependency_statements(
                                syntax_trees,
                                machine,
                                accepted_statements,
                            )?;
                        }
                        ConformanceMember::Reference { .. } => {}
                    }
                }
            }
            Item::Capability(capability) => {
                for member in syntax_trees.items.capability_members(capability.members) {
                    if let CapabilityMember::State(state) = member {
                        reject_dependency_statements(
                            syntax_trees,
                            syntax_trees.items.statements(state.signature.default_body),
                            accepted_statements,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }

    for (expression_handle, expression) in syntax_trees.expressions.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if constructor_parts(literal).0 == SOURCE_TYPE_NAME =>
            {
                if !accepted_sources.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::StructLiteral(literal)
                if constructor_parts(literal).0 == PACKAGE_SELECTION_TYPE_NAME =>
            {
                if !accepted_selections.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::Call(call) if is_dependency_call_name(call.target.as_str()) => {
                match (
                    DependencyOperation::classify(call.target.as_str()),
                    syntax_trees.expressions.expression_handles(call.arguments),
                ) {
                    (Some(operation), [source])
                        if !operation.takes_alias() && accepted_sources.contains(source) => {}
                    (Some(operation), [alias, source])
                        if operation.takes_alias()
                            && accepted_aliases.contains(alias)
                            && accepted_sources.contains(source) => {}
                    _ => return Err(DependencyProjectionError::UnsupportedDependencyShape),
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn reject_machine_dependency_statements(
    syntax_trees: &SyntaxTrees,
    machine: &syntax_trees::item::Machine,
    accepted_statements: &[StatementHandle],
) -> Result<(), DependencyProjectionError> {
    for state_handle in syntax_trees.items.state_handles(machine.states) {
        let state = syntax_trees.items.state(*state_handle);
        reject_dependency_statements(
            syntax_trees,
            syntax_trees.items.statements(state.statements),
            accepted_statements,
        )?;
    }
    Ok(())
}

fn reject_dependency_statements(
    syntax_trees: &SyntaxTrees,
    statements: &[StatementHandle],
    accepted_statements: &[StatementHandle],
) -> Result<(), DependencyProjectionError> {
    for statement_handle in statements {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        if is_dependency_call_name(call.target.as_str())
            && !accepted_statements.contains(statement_handle)
        {
            return Err(DependencyProjectionError::UnsupportedDependencyShape);
        }
    }
    Ok(())
}

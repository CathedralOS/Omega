//! The toolchain Build activation: its exact vocabulary and the proof that
//! source cannot replace, copy, store, or construct it. The Build carries
//! no target -- build evaluation cannot observe or branch on one
//! (`wiki/spec/build/configuration.md#multi-target-compilation`).

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

use crate::admission::vocabulary::is_exact_toolchain_build_prelude_data;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuildActivationVocabulary {
    pub(crate) build_symbol: SymbolHandle,
    pub(crate) x86_deployment_features_field_symbol: SymbolHandle,
}

fn type_reference_names_exact_data(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expected: SymbolHandle,
) -> bool {
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_names_exact_data(typed, *referee, expected)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_names_exact_data(typed, *base_type, expected)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol == expected,
        _ => false,
    }
}

/// Admit `Build.target` and its deployment-feature companion only when every
/// field and closed enum type comes from the exact toolchain virtual source.
/// `BuildTimeValue` is structurally named, so this nominal check must precede
/// argument construction.
/// The exact toolchain Build, when the program's build uses it. An authored
/// legacy `Build` receives no compiler-owned fields and yields `None`.
pub(crate) fn build_activation_vocabulary(
    typed: &TypedTrees,
) -> Result<Option<BuildActivationVocabulary>, Vec<Diagnostic>> {
    let exact_builds = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, "Build")
        })
        .collect::<Vec<_>>();
    let build = match exact_builds.as_slice() {
        [] => return Ok(None),
        [build] => *build,
        _ => {
            return Err(vec![Diagnostic::error(
                "the program declares more than one toolchain Build activation",
            )]);
        }
    };
    let x86_feature_fields = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == "x86_deployment_features" =>
            {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [x86_feature_field] = x86_feature_fields.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "toolchain Build declares {} `x86_deployment_features` fields; the activation requires exactly one",
            x86_feature_fields.len()
        ))]);
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(x86_feature_field.type_reference)
    else {
        return Err(vec![Diagnostic::error(
            "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type",
        )]);
    };
    if !is_exact_toolchain_build_prelude_data(typed, *symbol, "X86DeploymentFeatures") {
        return Err(vec![Diagnostic::error(
            "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type",
        )]);
    }
    Ok(Some(BuildActivationVocabulary {
        build_symbol: build.symbol,
        x86_deployment_features_field_symbol: x86_feature_field.symbol,
    }))
}

fn expression_denotes_exact_build(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    build_symbols: &[SymbolHandle],
) -> bool {
    use typed_trees::expression::ExpressionNode;
    match typed.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            expression_denotes_exact_build(typed, borrow.target, build_symbols)
        }
        ExpressionNode::Name(path) => {
            build_symbols.contains(&path.head_symbol) || build_symbols.contains(&path.symbol)
        }
        _ => false,
    }
}

/// Prove source cannot transiently overwrite, replace, or lend exclusive
/// access to the compiler-issued target occurrence. Final-value equality is
/// retained as corruption defense, but is not used as the immutability proof.
pub(crate) fn validate_build_activation_integrity(
    typed: &TypedTrees,
    vocabulary: BuildActivationVocabulary,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::{data::DataMember, expression::ExpressionNode, statement::StatementNode};

    let mut build_value_symbols = Vec::new();
    let mut diagnostics = Vec::new();
    for definition in typed.data_definitions() {
        for member in typed.data_members(definition) {
            let DataMember::Field(field) = member else {
                continue;
            };
            if type_reference_names_exact_data(typed, field.type_reference, vocabulary.build_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in field `{}`",
                    field.name.as_str()
                )));
                build_value_symbols.push(field.symbol);
            }
        }
    }
    for machine in typed.machines() {
        if !machine.body_is_present
            && typed.machine_states(machine).iter().any(|state| {
                typed.state_parameters(state).iter().any(|parameter| {
                    type_reference_names_exact_data(
                        typed,
                        parameter.type_reference,
                        vocabulary.build_symbol,
                    )
                })
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "bodyless machine `{}` cannot receive the compiler-owned Build activation",
                machine.name.as_str()
            )));
        }
        for owned in typed.machine_owned_data(machine) {
            if type_reference_names_exact_data(typed, owned.type_reference, vocabulary.build_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in machine field `{}`",
                    owned.name.as_str()
                )));
                build_value_symbols.push(owned.symbol);
            }
        }
        for state in typed.machine_states(machine) {
            for parameter in typed.state_parameters(state) {
                if type_reference_names_exact_data(
                    typed,
                    parameter.type_reference,
                    vocabulary.build_symbol,
                ) {
                    build_value_symbols.push(parameter.symbol);
                }
            }
            for statement in typed.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if expression_denotes_exact_build(
                            typed,
                            assignment.target,
                            &build_value_symbols,
                        ) {
                            diagnostics.push(Diagnostic::error(
                                "the compiler-owned Build activation cannot be replaced as a whole value",
                            ));
                        }
                    }
                    StatementNode::LocalData(local)
                        if type_reference_names_exact_data(
                            typed,
                            local.type_reference,
                            vocabulary.build_symbol,
                        ) =>
                    {
                        if !matches!(
                            typed
                                .type_reference_table
                                .type_reference(local.type_reference),
                            typed_trees::types::TypeReferenceNode::Reference { .. }
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                                "the compiler-owned Build activation cannot be copied into local `{}`",
                                local.name.as_str()
                            )));
                        }
                        build_value_symbols.push(local.symbol);
                    }
                    _ => {}
                }
            }
        }
    }
    for (_, expression) in typed.expression_table.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if literal.type_symbol == vocabulary.build_symbol =>
            {
                diagnostics.push(Diagnostic::error(
                    "source cannot construct the compiler-owned Build activation",
                ));
            }
            ExpressionNode::ZeroValue(type_reference)
                if type_reference_names_exact_data(
                    typed,
                    *type_reference,
                    vocabulary.build_symbol,
                ) =>
            {
                diagnostics.push(Diagnostic::error(
                    "source cannot construct a zero value of the compiler-owned Build activation",
                ));
            }
            _ => {}
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

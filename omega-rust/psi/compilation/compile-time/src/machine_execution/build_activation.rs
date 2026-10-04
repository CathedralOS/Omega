//! The toolchain Build activation: its exact vocabulary and the proof that
//! source cannot replace, copy, store, or construct it. The Build carries
//! no target -- build evaluation cannot observe or branch on one
//! (`wiki/spec/build/configuration.md#multi-target-compilation`).

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Admit the compiler-issued Build before any build effects execute.
///
/// Recognition alone grants nothing: the same operation checks the exact
/// toolchain declaration and rejects source construction, storage, copying,
/// or whole-value replacement. Returns true for an admitted toolchain Build;
/// false leaves a legacy authored Build without compiler-issued fields.
pub fn admit_toolchain_build_activation(typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    let Some(build_symbol) = build_activation_symbol(typed)? else {
        return Ok(false);
    };
    validate_build_activation_integrity(typed, build_symbol)?;
    Ok(true)
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

/// Resolve the exact toolchain Build and deployment-feature field before
/// argument construction. A source-authored legacy Build yields no activation.
fn build_activation_symbol(typed: &TypedTrees) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
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
    Ok(Some(build.symbol))
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
/// access to the compiler-issued Build occurrence. Final-value equality is
/// retained as corruption defense, but is not used as the immutability proof.
fn validate_build_activation_integrity(
    typed: &TypedTrees,
    build_symbol: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::{data::DataMember, expression::ExpressionNode, statement::StatementNode};

    let mut build_value_symbols = Vec::new();
    let mut diagnostics = Vec::new();
    for definition in typed.data_definitions() {
        for member in typed.data_members(definition) {
            let DataMember::Field(field) = member else {
                continue;
            };
            if type_reference_names_exact_data(typed, field.type_reference, build_symbol) {
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
                    type_reference_names_exact_data(typed, parameter.type_reference, build_symbol)
                })
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "bodyless machine `{}` cannot receive the compiler-owned Build activation",
                machine.name.as_str()
            )));
        }
        for owned in typed.machine_owned_data(machine) {
            if type_reference_names_exact_data(typed, owned.type_reference, build_symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in machine field `{}`",
                    owned.name.as_str()
                )));
                build_value_symbols.push(owned.symbol);
            }
        }
        for state in typed.machine_states(machine) {
            for parameter in typed.state_parameters(state) {
                if type_reference_names_exact_data(typed, parameter.type_reference, build_symbol) {
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
                            build_symbol,
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
            ExpressionNode::StructLiteral(literal) if literal.type_symbol == build_symbol => {
                diagnostics.push(Diagnostic::error(
                    "source cannot construct the compiler-owned Build activation",
                ));
            }
            ExpressionNode::ZeroValue(type_reference)
                if type_reference_names_exact_data(typed, *type_reference, build_symbol) =>
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

/// Recognize one exact nominal data declaration in the toolchain Build prelude.
/// This predicate alone does not admit an activation or any build effects.
pub fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == expected_name
            && typed
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
    })
}

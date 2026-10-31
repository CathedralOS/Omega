//! The toolchain Build activation: its exact vocabulary and the proof that
//! source cannot replace, copy, store, or construct it. The Build carries
//! no target -- build evaluation cannot observe or branch on one
//! (`wiki/spec/build/configuration.md#multi-target-compilation`).

use crate::build_time_evaluation::type_reference_table;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildStorage {
    Owned,
    Borrowed,
}

// Storing a loan delegates the original activation's access; it does not store
// the activation itself. Whole-value writes below query their exact target
// type through the shared place projection, including fields and call results.
// Ordinary carried-loan checking owns exclusivity, lifetime and projection.
fn build_storage(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expected: SymbolHandle,
) -> Option<BuildStorage> {
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            build_storage(typed, *referee, expected).map(|_| BuildStorage::Borrowed)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            build_storage(typed, *base_type, expected)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == expected => {
            Some(BuildStorage::Owned)
        }
        _ => None,
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
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        type_reference_table(typed).type_reference(x86_feature_field.type_reference)
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

/// Prove source cannot construct, copy or replace the compiler-issued Build
/// occurrence. Borrowing it preserves the original activation; final-value
/// equality is corruption defense rather than the immutability proof.
fn validate_build_activation_integrity(
    typed: &TypedTrees,
    build_symbol: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::{expression::ExpressionNode, statement::StatementNode};

    let mut diagnostics = Vec::new();
    for definition in typed.data_definitions() {
        for member in typed.data_members(definition) {
            let Some(field) = member.as_field() else {
                continue;
            };
            if build_storage(typed, field.type_reference, build_symbol) == Some(BuildStorage::Owned)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in field `{}`",
                    field.name.as_str()
                )));
            }
        }
    }
    for machine in typed.machines() {
        if !machine.body_is_present
            && typed.machine_states(machine).iter().any(|state| {
                typed.state_parameters(state).iter().any(|parameter| {
                    build_storage(typed, parameter.type_reference, build_symbol).is_some()
                })
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "bodyless machine `{}` cannot receive the compiler-owned Build activation",
                machine.name.as_str()
            )));
        }
        for owned in typed.machine_owned_data(machine) {
            if build_storage(typed, owned.type_reference, build_symbol) == Some(BuildStorage::Owned)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in machine field `{}`",
                    owned.name.as_str()
                )));
            }
        }
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if validation::expression_place_type_reference(
                            typed,
                            assignment.target,
                            &[],
                        )
                        .is_some_and(|reference| {
                            build_storage(typed, reference, build_symbol).is_some()
                        }) {
                            diagnostics.push(Diagnostic::error(
                                "the compiler-owned Build activation cannot be replaced as a whole value",
                            ));
                        }
                    }
                    StatementNode::LocalData(local)
                        if build_storage(typed, local.type_reference, build_symbol)
                            == Some(BuildStorage::Owned) =>
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "the compiler-owned Build activation cannot be copied into local `{}`",
                            local.name.as_str()
                        )));
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
                if build_storage(typed, *type_reference, build_symbol).is_some() =>
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
            && typed.symbols.is_build_prelude_symbol(symbol)
    })
}

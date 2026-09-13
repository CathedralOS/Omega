//! Exact toolchain build vocabulary and immutable target admission.

use diagnostics::Diagnostic;
use std::path::Path;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

const BUILD_MACHINE: &str = "build";

/// Whether a machine is the program's canonical free build machine, declared
/// in the exact companion `build.omg` selected by project
/// discovery. Typed symbols retain authored source identity, so neither a
/// filename scan nor a machine-name handoff is authority.
/// A wrong-arity build machine still refuses at evaluation with the arity
/// error (pinned by fail/build/build_machine_wrong_arity).
pub fn is_build_machine(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    build_source_id: Option<source::SourceId>,
) -> bool {
    if machine.name.as_str() != BUILD_MACHINE {
        return false;
    }
    let Some(build_source_id) = build_source_id else {
        return false;
    };
    typed
        .symbols
        .symbol_source_span(machine.symbol)
        .is_some_and(|span| span.source_id == build_source_id)
}

pub(super) fn has_exact_toolchain_build_facet(typed: &TypedTrees, name: &str) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == name
            && typed
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
    })
}

fn is_exact_toolchain_build_filesystem_facet_call(
    typed: &TypedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> bool {
    typed.machines().iter().any(|machine| {
        if machine.symbol != target_machine
            || !machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| matches!(attached.as_str(), "BuildSource" | "BuildOutput"))
            || !typed
                .symbols
                .symbol_source_span(machine.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
        {
            return false;
        }
        typed.machine_states(machine).iter().any(|state| {
            state.symbol == target_state
                && matches!(
                    (
                        machine.attached_data.as_ref().map(|name| name.as_str()),
                        state.name.as_str(),
                    ),
                    (Some("BuildSource"), "open" | "read" | "close")
                        | (
                            Some("BuildOutput"),
                            "create" | "write" | "close" | "include_source"
                        )
                )
        })
    })
}

pub(super) fn build_reaches_filesystem_facet(
    typed: &TypedTrees,
    operational: &flow_effects::OperationalPlan,
    root: SymbolHandle,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(machine) = operational
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
        else {
            continue;
        };
        for state in operational.states.span_or_empty(machine.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                // Attached calls through the compiler-created private facet
                // value are intentionally unresolved at this typed prepass;
                // the evaluator below still admits only the exact prelude
                // declaration and private activation marker. A same-named
                // ordinary call can at most provision an unused sponsor: it
                // cannot obtain either rooted facet value.
                if is_exact_toolchain_build_filesystem_facet_call(
                    typed,
                    call.target_machine_symbol,
                    call.target_state_symbol,
                ) || (!call.target_state_symbol.is_valid()
                    && matches!(
                        call.target_name.as_str(),
                        "open" | "read" | "close" | "create" | "write" | "include_source"
                    ))
                {
                    return true;
                }
                if call.target_machine_symbol.is_valid() {
                    pending.push(call.target_machine_symbol);
                }
            }
        }
    }
    false
}

pub(super) fn is_exact_toolchain_build_prelude_data(
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
                        && file.path == Path::new("<build-prelude>")
                })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TargetBuildVocabulary {
    pub(super) build_symbol: SymbolHandle,
    pub(super) target_field_symbol: SymbolHandle,
    pub(super) x86_deployment_features_field_symbol: SymbolHandle,
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
pub(super) fn target_build_vocabulary(
    typed: &TypedTrees,
    selected_target: Option<target::TargetProfile>,
) -> Result<Option<TargetBuildVocabulary>, Vec<Diagnostic>> {
    let Some(_selected_target) = selected_target else {
        return Ok(None);
    };
    let exact_builds = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, "Build")
        })
        .collect::<Vec<_>>();
    let [build] = exact_builds.as_slice() else {
        return Err(vec![Diagnostic::error(
            "exact-target build activation requires the toolchain-provided Build.target vocabulary; an authored legacy Build cannot receive a hidden target field",
        )]);
    };
    let target_fields = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "target" => {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [target_field] = target_fields.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "toolchain Build declares {} `target` fields; exact-target activation requires exactly one",
            target_fields.len()
        ))]);
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(target_field.type_reference)
    else {
        return Err(vec![Diagnostic::error(
            "toolchain Build.target must have the exact toolchain TargetProfile type",
        )]);
    };
    if !is_exact_toolchain_build_prelude_data(typed, *symbol, "TargetProfile") {
        return Err(vec![Diagnostic::error(
            "toolchain Build.target must have the exact toolchain TargetProfile type",
        )]);
    }
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
            "toolchain Build declares {} `x86_deployment_features` fields; exact-target activation requires exactly one",
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
    Ok(Some(TargetBuildVocabulary {
        build_symbol: build.symbol,
        target_field_symbol: target_field.symbol,
        x86_deployment_features_field_symbol: x86_feature_field.symbol,
    }))
}

fn expression_mentions_exact_field(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    field: SymbolHandle,
    build_value_symbols: &[SymbolHandle],
) -> bool {
    use typed_trees::expression::ExpressionNode;
    match typed.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            expression_mentions_exact_field(typed, borrow.target, field, build_value_symbols)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_exact_field(typed, indexed.collection, field, build_value_symbols)
        }
        ExpressionNode::Member(member) => {
            member.member_symbol == field
                || (member.member.as_str() == "target"
                    && expression_denotes_exact_build(typed, member.receiver, build_value_symbols))
                || expression_mentions_exact_field(
                    typed,
                    member.receiver,
                    field,
                    build_value_symbols,
                )
        }
        _ => false,
    }
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
pub(super) fn validate_immutable_build_target(
    typed: &TypedTrees,
    vocabulary: TargetBuildVocabulary,
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
                    "compiler-owned Build.target forbids storing the exact toolchain Build in field `{}`",
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
                    "compiler-owned Build.target forbids storing the exact toolchain Build in machine field `{}`",
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
                        if expression_mentions_exact_field(
                            typed,
                            assignment.target,
                            vocabulary.target_field_symbol,
                            &build_value_symbols,
                        ) {
                            diagnostics.push(Diagnostic::error(
                                "Build.target is compiler-owned and cannot be assigned",
                            ));
                        } else if expression_denotes_exact_build(
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
                                "compiler-owned Build.target forbids copying the Build activation into local `{}`",
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
            ExpressionNode::Borrow(borrow)
                if borrow.access.is_exclusive()
                    && expression_mentions_exact_field(
                        typed,
                        borrow.target,
                        vocabulary.target_field_symbol,
                        &build_value_symbols,
                    ) =>
            {
                diagnostics.push(Diagnostic::error(
                    "Build.target is compiler-owned and cannot enter a mutable or write-only borrow",
                ));
            }
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

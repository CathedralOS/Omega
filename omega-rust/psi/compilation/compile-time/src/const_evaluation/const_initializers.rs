//! Declaration values are ready before index normalization asks for them.
//!
//! A target observation instead defers its complete selected initializer
//! closure. `target_observations` resumes unqualified scalar declarations after
//! layout closure, orders aliases by their actual dependencies and consumes
//! completed values at exact selected uses. Source-call inventories remain
//! separate from parent operator meaning and from invocation permission.
//!
//! Index the authored declarations without substituting pending values and
//! complete only the helpers their initializer roots demand. Their
//! exact source-owned dependencies determine evaluation order, including unused
//! declarations and references in unselected branches. Then use the same typed
//! scalar probes as index expressions, one per pending scalar leaf. Aggregate
//! declarations retain their constructors and fields while evaluated leaves
//! replace only their exact authored expressions. Their identity is encoded from
//! the complete materialized value. Private probes retain unfinished declarations
//! without substituting them. Declaration types and authored expressions jointly
//! order readiness, while each leaf retains only its actual value origins:
//! scheduling a geometry dependency does not invent a value use in that leaf.
//! Every returned origin must rejoin a completed value and original selection. Only evaluated leaves and
//! declaration-owned receipts leave this module.
//!
//! A scalar declaration is not a request for runtime bytes. Payloadless NaNs
//! therefore retain their format and meaning as an internal expression, so
//! aliases and later evaluation can classify them. Encoding remains a separate
//! demand and cannot choose a payload. Typed replay reconstructs the value from
//! the authored initializer and rejoins dependency and operator custody before
//! trusting that semantic expression. Aggregate encoding remains a materializing
//! boundary; this scalar route does not relax its recursive bit requirements.
//!
//! Ordinary calls extend that same scalar evaluation sequence. Their selected
//! helper closure contributes dependencies before readiness is decided; helper
//! bodies undergo ordinary checking and invocation admission before execution.
//! The synthetic scalar expression is not checked as a runtime body: a call's
//! concrete result can establish arithmetic that its full return-type range
//! cannot. Final typing retains both initializer roots for independent replay.
//!
//! The graph is prepared once; independent declarations share a typed probe batch.
//! Deep dependency chains still require one frontend pass per dependency layer.
//! That is a performance limitation, not a reason to erase declared landings or
//! promote a provisional value into a public constant identity.
//!
//! A literal initializer can still wait for its declared geometry: a named
//! array extent may be computed by another declaration. Its lack of pending
//! leaves does not supply a completed value. Before a ready probe reads any
//! selected dependency, complete missing literal values against the current
//! syntax and the original declaration selector. The dependency graph already
//! orders that extent's evaluation; neither a probe stand-in nor a provisional
//! layout may provide the encoded value.

use std::collections::HashMap;
use std::sync::Arc;

use diagnostics::Diagnostic;
use language_core::constant_value::EvaluatedConstant;
use language_semantics::const_value::DecodedCanonicalConstValue;
use source::{SourceMap, SourceSpan};
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::{
    ConstDefinition, ConstInitializerCustody, ConstInitializerNormalization, Item, ItemHandle,
};
use syntax_trees::types::ConstArgumentOrigin;

mod completed_values;
mod invocations;
mod materialize;
mod replay;
mod target_observations;

pub(crate) use completed_values::read_back as read_back_completed_values;
pub(crate) use invocations::CheckedInitializers;
pub(crate) use replay::validate as validate_retained_invocations;
pub(crate) use target_observations::settle as settle_target_observations;

struct PendingLeaf {
    expression: ExpressionHandle,
    destination: syntax_trees::types::TypeReferenceHandle,
    /// Structured leaves are aggregate-producing expressions evaluated through
    /// the checked interpreter; scalar leaves keep the exact probe evaluator.
    structured: bool,
    dependencies: Vec<(SourceSpan, usize)>,
    calls: Vec<(SourceSpan, SourceSpan)>,
}

struct Declaration {
    item: ItemHandle,
    definition: ConstDefinition,
    evaluated_value: Option<EvaluatedConstant>,
    leaves: Vec<PendingLeaf>,
    dependencies: Vec<(SourceSpan, usize)>,
    calls: Vec<(SourceSpan, SourceSpan)>,
    authored_dependencies: Vec<(SourceSpan, usize)>,
    authored_calls: Vec<(SourceSpan, SourceSpan)>,
    authored_operators: Vec<SourceSpan>,
    readiness_operators: Vec<SourceSpan>,
    normalization_owners: Vec<SourceSpan>,
}

pub(crate) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    use syntax_trees_to_symbol_resolved_trees::pre_resolution::requires_const_initializer_evaluation;
    syntax = super::const_qualifications::prove_ready(syntax, sources.clone(), bindings)?;
    if !syntax.root_items().any(|item| {
        matches!(item, Item::Const(definition)
        if requires_const_initializer_evaluation(&syntax, definition))
    }) {
        return Ok(syntax);
    }
    // Selection discovery cannot prove a literal declaration's qualification
    // while its index still depends on another initializer. Keep its actual
    // value and carrier in this private forest, but defer that qualification
    // just as the later typed probes do for provisional indices.
    let mut selection_syntax = syntax.clone();
    super::const_qualifications::defer_unproved(&mut selection_syntax);
    let selection_arguments =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
            &syntax,
            sources.clone(),
            bindings.to_vec(),
        )?;
    crate::syntax_probes::defer_pending_const_qualifications(
        &mut selection_syntax,
        &selection_arguments
            .iter()
            .map(|(argument, _, _)| *argument)
            .collect::<Vec<_>>(),
    );
    let preparation =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::prepare_const_initializer_selection(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: selection_syntax,
                sources: sources.clone(),
                top_level_bindings: bindings.to_vec(),
            },
        )?;
    let resolved = preparation.trees();
    let mut declarations = Vec::new();
    for item in syntax.root_item_handles() {
        let Item::Const(definition) = syntax.root_item(*item) else {
            continue;
        };
        let retained = resolved
            .const_declarations
            .iter()
            .find(|declaration| {
                resolved.symbols.symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| {
                failure(
                    definition.name.source_span(),
                    "initializer lost its exact declaration",
                )
            })?;
        let leaves = preparation
            .pending_leaves(&syntax, definition)?
            .into_iter()
            .map(|leaf| PendingLeaf {
                expression: leaf.expression,
                destination: leaf.destination,
                structured: leaf.structured,
                dependencies: Vec::new(),
                calls: Vec::new(),
            })
            .collect();
        declarations.push(Declaration {
            item: *item,
            definition: definition.clone(),
            evaluated_value: retained.evaluated_value.clone(),
            leaves,
            dependencies: Vec::new(),
            calls: Vec::new(),
            authored_dependencies: Vec::new(),
            authored_calls: Vec::new(),
            authored_operators: Vec::new(),
            readiness_operators: Vec::new(),
            normalization_owners: Vec::new(),
        });
    }
    let mut target_observations = vec![false; declarations.len()];
    for ordinal in 0..declarations.len() {
        let selected =
            preparation.initializer_dependencies(&syntax, &declarations[ordinal].definition)?;
        target_observations[ordinal] = selected.pending_target_observation;
        let authored =
            preparation.initializer_authored_dependencies(&declarations[ordinal].definition)?;
        declarations[ordinal].authored_calls = authored.calls;
        declarations[ordinal].authored_operators = authored.initializer_operators;
        for (reference, source) in authored.constants {
            let dependency = declarations
                .iter()
                .position(|candidate| candidate.definition.name.source_span() == source)
                .ok_or_else(|| {
                    failure(reference, "authored dependency lost its exact declaration")
                })?;
            declarations[ordinal]
                .authored_dependencies
                .push((reference, dependency));
        }
        declarations[ordinal].readiness_operators = selected.initializer_operators;
        declarations[ordinal].calls = selected.calls;
        declarations[ordinal].normalization_owners = selected.normalization_owners;
        for (reference, declaration) in selected.constants {
            let dependency = declarations
                .iter()
                .position(|candidate| candidate.definition.name.source_span() == declaration)
                .ok_or_else(|| {
                    failure(reference, "selected declaration dependency lost its owner")
                })?;
            let selected = (reference, dependency);
            if !declarations[ordinal].dependencies.contains(&selected) {
                declarations[ordinal].dependencies.push(selected);
            }
        }
        if declarations[ordinal].leaves.is_empty() {
            continue;
        }
        let leaf_count = declarations[ordinal].leaves.len();
        for leaf_ordinal in 0..leaf_count {
            let expression = declarations[ordinal].leaves[leaf_ordinal].expression;
            let selected = preparation.initializer_expression_dependencies(
                &syntax,
                &declarations[ordinal].definition,
                expression,
            )?;
            let mut dependencies = Vec::new();
            for (reference, declaration) in selected.constants {
                let dependency = declarations
                    .iter()
                    .position(|candidate| candidate.definition.name.source_span() == declaration)
                    .ok_or_else(|| {
                        failure(
                            reference,
                            "selected initializer dependency lost its declaration",
                        )
                    })?;
                dependencies.push((reference, dependency));
            }
            declarations[ordinal].leaves[leaf_ordinal].dependencies = dependencies;
            declarations[ordinal].leaves[leaf_ordinal].calls = selected.calls;
        }
    }
    // Readiness follows the complete selected dependency closure, including
    // aliases and helper bodies. Keep whole expressions symbolic, rather than
    // trying to evaluate a nested observation as an ordinary machine call.
    retain_pending_observations(&mut syntax, &mut declarations, &mut target_observations);
    // Evaluation roots demand their complete declaration graph. A leafless
    // literal is not ready merely because it has nothing to execute: its
    // declared geometry may still depend on a pending computation. Keep that
    // distinction until canonicalization supplies the actual encoded value.
    let mut demanded = declarations
        .iter()
        .map(|declaration| {
            !declaration.definition.pending_target_observation && !declaration.leaves.is_empty()
        })
        .collect::<Vec<_>>();
    let mut pending = demanded
        .iter()
        .enumerate()
        .filter_map(|(ordinal, demanded)| demanded.then_some(ordinal))
        .collect::<Vec<_>>();
    while let Some(ordinal) = pending.pop() {
        for (_, dependency) in &declarations[ordinal].dependencies {
            if !demanded[*dependency] {
                demanded[*dependency] = true;
                pending.push(*dependency);
            }
        }
    }
    while declarations
        .iter()
        .enumerate()
        .any(|(ordinal, declaration)| {
            demanded[ordinal]
                && (!declaration.leaves.is_empty() || declaration.evaluated_value.is_none())
        })
    {
        let mut ready = declarations
            .iter()
            .enumerate()
            .filter_map(|(ordinal, declaration)| {
                (demanded[ordinal]
                    && (!declaration.leaves.is_empty() || declaration.evaluated_value.is_none())
                    && declaration.dependencies.iter().all(|(_, dependency)| {
                        declarations[*dependency].leaves.is_empty()
                            && declarations[*dependency].evaluated_value.is_some()
                    }))
                .then_some(ordinal)
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let ordinal = declarations
                .iter()
                .enumerate()
                .find_map(|(ordinal, declaration)| {
                    (demanded[ordinal]
                        && (!declaration.leaves.is_empty()
                            || declaration.evaluated_value.is_none()))
                    .then_some(ordinal)
                })
                .ok_or_else(|| {
                    failure(
                        SourceSpan::default(),
                        "initializer readiness lost its pending declaration",
                    )
                })?;
            return Err(failure(
                declarations[ordinal].definition.name.source_span(),
                "constant initializer dependency cycle",
            ));
        }
        for ordinal in &ready {
            if declarations[*ordinal].leaves.is_empty() {
                let value = preparation
                    .canonicalize_initializer_value(&syntax, &declarations[*ordinal].definition)
                    .map_err(|reason| {
                        failure(declarations[*ordinal].definition.name.source_span(), reason)
                    })?;
                declarations[*ordinal].evaluated_value = Some(value.encoding.into());
            }
        }
        ready.retain(|ordinal| !declarations[*ordinal].leaves.is_empty());
        if ready.is_empty() {
            continue;
        }
        let mut probe = syntax.clone();
        // Retain authored pending values through the resolver's existing
        // initializer frontier. Unproved qualifications remain private and no
        // unready constant can enter a ready probe's execution closure.
        super::const_qualifications::defer_unproved(&mut probe);
        let declaration_frontier = probe.root_item_handles().len();
        let mut probe_sources = Vec::new();
        for ordinal in &ready {
            for leaf in &declarations[*ordinal].leaves {
                crate::const_evaluation::const_generic_expressions::append_probe(
                    &mut probe,
                    probe_sources.len(),
                    leaf.expression,
                    leaf.destination,
                );
                probe_sources.push(syntax.expressions.source_span(leaf.expression));
            }
        }
        let mut probe_declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
        let owners = ready
            .iter()
            .flat_map(|ordinal| declarations[*ordinal].normalization_owners.iter().copied())
            .collect::<Vec<_>>();
        crate::syntax_probes::extend_normalization_owners(
            &probe,
            &mut probe_declarations,
            &owners,
        )?;
        let probe = crate::syntax_probes::normalize_initializer_probe_data(
            probe,
            sources.clone(),
            bindings,
            None,
            &probe_declarations,
        )?;
        let frontier = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: probe,
                sources: sources.clone(),
                top_level_bindings: bindings.to_vec(),
            },
        )?;
        // Ready values still need real helper implementations and their type,
        // proof and operator dependencies. Reuse the scalar probe's retained
        // body demand; unrelated implementations remain with their owners and
        // never enter the private execution program or its ordinary checks.
        let (typed, domain) =
            crate::syntax_probes::prepare_evaluation(frontier, &probe_sources, &[], &[])?;
        let mut authored_probes = Vec::new();
        for ordinal in &ready {
            for leaf in &declarations[*ordinal].leaves {
                let expected =
                    dependency_origins(&syntax, &declarations, &leaf.dependencies, true)?;
                let reference = syntax.expressions.source_span(leaf.expression);
                let custody = crate::const_evaluation::const_generic_expressions::validate_authored_probe_custody(
                    &typed, reference, &expected, &leaf.calls, &syntax,
                ).map_err(|reason| failure(reference, reason))?;
                authored_probes.push((leaf.expression, custody));
            }
        }
        let checked = ready
            .iter()
            .any(|ordinal| {
                declarations[*ordinal]
                    .leaves
                    .iter()
                    .any(|leaf| leaf.structured || !leaf.calls.is_empty())
            })
            .then(|| {
                let mut symbols = Vec::new();
                for ordinal in &ready {
                    for leaf in &declarations[*ordinal].leaves {
                        let reference = syntax.expressions.source_span(leaf.expression);
                        let matching = typed
                            .machines()
                            .iter()
                            .filter(|machine| {
                                typed.symbols.symbol_source_span(machine.symbol) == Some(reference)
                            })
                            .collect::<Vec<_>>();
                        let [machine] = matching.as_slice() else {
                            return Err(failure(
                                reference,
                                "initializer probe lost its unique source owner",
                            ));
                        };
                        symbols.push(machine.symbol);
                    }
                }
                let prepared = crate::PreparedBuildMachineProgram::prepare_in(&typed, &domain)?;
                invocations::CheckedInitializers::prepare(
                    &prepared,
                    authority.clone(),
                    &symbols,
                    &domain.accounting,
                )
            })
            .transpose()?;
        if let Some(checked) = checked.as_ref() {
            for ordinal in &ready {
                for leaf in &declarations[*ordinal].leaves {
                    let reference = syntax.expressions.source_span(leaf.expression);
                    if checked
                        .source_needs_execution_selection(reference)
                        .map_err(|reason| failure(reference, reason))?
                    {
                        target_observations[*ordinal] = true;
                    }
                }
            }
            // A selected boundary body is target-scoped supply. Retain the
            // complete declaration and every dependent alias until Omega has
            // supplied that exact execution; none receives a provisional value.
            retain_pending_observations(&mut syntax, &mut declarations, &mut target_observations);
            for (ordinal, pending) in target_observations.iter().copied().enumerate() {
                if pending {
                    demanded[ordinal] = false;
                }
            }
            ready.retain(|ordinal| !target_observations[*ordinal]);
            if ready.is_empty() {
                continue;
            }
        }
        let typed = checked
            .as_ref()
            .map_or(&typed, invocations::CheckedInitializers::typed);
        for ordinal in ready {
            let authored_expression = declarations[ordinal].definition.value;
            let mut replacements = HashMap::new();
            let mut selections = Vec::new();
            let mut builtin_operators = Vec::new();
            let mut call_selections = Vec::new();
            let mut scalar_value = None;
            for leaf in &declarations[ordinal].leaves {
                let reference = syntax.expressions.source_span(leaf.expression);
                let authored = &authored_probes
                    .iter()
                    .find(|(expression, _)| *expression == leaf.expression)
                    .ok_or_else(|| {
                        failure(reference, "initializer lost its checked source custody")
                    })?
                    .1;
                let mut expected = Vec::new();
                for (reference, dependency) in &leaf.dependencies {
                    let dependency = &declarations[*dependency];
                    let origin = ConstArgumentOrigin {
                        reference: *reference,
                        declaration: dependency.definition.name.source_span(),
                        initializer: syntax.expressions.source_span(dependency.definition.value),
                        evaluated_value: dependency.evaluated_value.clone().ok_or_else(|| {
                            failure(
                                *reference,
                                "initializer selected a dependency without an evaluated value",
                            )
                        })?,
                    };
                    if !expected.contains(&origin) {
                        expected.push(origin);
                    }
                    if let Some(normalization) = &dependency.definition.normalization {
                        for origin in &normalization.selections {
                            if !expected.contains(origin) {
                                expected.push(origin.clone());
                            }
                        }
                    }
                }
                if leaf.structured {
                    let checked = checked
                        .as_ref()
                        .expect("aggregate leaf has checked preparation");
                    let calls = checked
                        .calls_for_source(reference)
                        .map_err(|reason| failure(reference, reason))?;
                    // The leaf probe's checked interpreter supplies the value;
                    // every authored call inside the leaf is still admitted at
                    // its concrete arguments, and the reconstructed origins and
                    // call roster must match the original selection exactly.
                    let evaluation = calls
                        .evaluate_leaf(reference, authored, &syntax)
                        .map_err(|reason| failure(reference, reason))?;
                    for selection in &evaluation.calls {
                        if !call_selections.contains(selection) {
                            call_selections.push(*selection);
                        }
                    }
                    for origin in evaluation.origins {
                        if !selections.contains(&origin) {
                            selections.push(origin);
                        }
                    }
                    for operator in evaluation.operators {
                        if !builtin_operators.contains(&operator) {
                            builtin_operators.push(operator);
                        }
                    }
                    let materialized = materialize::literal(
                        &mut syntax,
                        typed,
                        evaluation.destination,
                        &evaluation.value,
                        reference,
                    )
                    .map_err(|reason| failure(reference, reason))?;
                    replacements.insert(leaf.expression, materialized);
                    for warning in &evaluation.warnings {
                        eprintln!("{warning}");
                    }
                    continue;
                }
                let result = if leaf.calls.is_empty() {
                    crate::const_evaluation::const_generic_expressions::evaluate_probe(
                        typed,
                        reference,
                        false,
                        authority.as_deref(),
                        Some(&expected),
                        &syntax,
                    )
                } else {
                    let checked = checked
                        .as_ref()
                        .expect("call-bearing declaration has checked preparation");
                    let calls = checked
                        .calls_for_source(reference)
                        .map_err(|reason| failure(reference, reason))?;
                    crate::const_evaluation::const_generic_expressions::evaluate_probe_with_calls(
                        typed,
                        reference,
                        false,
                        authority.as_deref(),
                        authored,
                        &syntax,
                        &calls,
                    )
                }
                .map_err(|reason| failure(reference, reason))?;
                for selection in &result.call_selections {
                    if !call_selections.contains(selection) {
                        call_selections.push(*selection);
                    }
                }
                for origin in result.origins {
                    if !selections.contains(&origin) {
                        selections.push(origin);
                    }
                }
                // Several fields may depend on the same computed declaration.
                // Its authored operators remain one provenance roster, not one
                // synthetic occurrence for every leaf which reads the value.
                for operator in result.operators {
                    if !builtin_operators.contains(&operator) {
                        builtin_operators.push(operator);
                    }
                }
                scalar_value = Some(result.value.declaration_value());
                use crate::const_evaluation::const_generic_expressions::value::ScalarValue;
                let literal = match result.value {
                    ScalarValue::UndeterminedNaN(format) => ExpressionNode::UndeterminedNaN(format),
                    ScalarValue::Float { format, bits } => {
                        let spelling = match format {
                            numerics::literals::FloatFormat::F32 => {
                                format!("{:?}f32", f32::from_bits(bits as u32))
                            }
                            numerics::literals::FloatFormat::F64 => {
                                format!("{:?}f64", f64::from_bits(bits))
                            }
                        };
                        ExpressionNode::Float(syntax_trees::FloatSpelling::new(spelling, reference))
                    }
                    ScalarValue::Index(value) => match value.decode_encoding() {
                        Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                            let spelling = value.to_string();
                            ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_parts(
                                    spelling.starts_with('-'),
                                    numerics::literals::IntegerRadix::Decimal,
                                    spelling.strip_prefix('-').unwrap_or(&spelling),
                                )
                                .map_err(|reason| failure(reference, reason))?,
                            )
                        }
                        Some(DecodedCanonicalConstValue::Boolean(value)) => {
                            ExpressionNode::Boolean(value)
                        }
                        _ => {
                            return Err(failure(
                                reference,
                                "initializer probe did not produce a scalar value",
                            ));
                        }
                    },
                };
                let materialized = syntax.expressions.insert(literal);
                syntax.expressions.set_source_span(materialized, reference);
                replacements.insert(leaf.expression, materialized);
                for warning in result.warnings {
                    eprintln!("{warning}");
                }
            }
            let value = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
            {
                *replacements
                    .get(&declarations[ordinal].leaves[0].expression)
                    .expect("scalar initializer replacement")
            } else {
                materialize_value(&mut syntax, authored_expression, &replacements)?
            };
            let mut definition = declarations[ordinal].definition.clone();
            // A scalar leaf retains its evaluated meaning without demanding bits; a
            // structured or multi-leaf value currently encodes from the complete
            // materialized literal instead.
            let result_value = if declarations[ordinal].leaves.len() == 1
                && authored_expression == declarations[ordinal].leaves[0].expression
                && !declarations[ordinal].leaves[0].structured
            {
                scalar_value.ok_or_else(|| {
                    failure(
                        syntax.expressions.source_span(authored_expression),
                        "scalar initializer lost its evaluated value",
                    )
                })?
            } else {
                let mut materialized_definition = definition.clone();
                materialized_definition.value = value;
                preparation
                    .canonicalize_initializer_value(&syntax, &materialized_definition)
                    .map_err(|reason| {
                        failure(syntax.expressions.source_span(authored_expression), reason)
                    })?
                    .encoding
                    .into()
            };
            // A declaration's geometry is part of its normalization custody,
            // even when a scalar payload leaf never reads that extent. Keep
            // those completed origins on the declaration receipt, not in the
            // leaf evaluator's value-use expectations. Only already evaluated
            // dependency receipts can contribute transitive operator meaning.
            for (reference, dependency) in &declarations[ordinal].authored_dependencies {
                let dependency = &declarations[*dependency];
                let origin = ConstArgumentOrigin {
                    reference: *reference,
                    declaration: dependency.definition.name.source_span(),
                    initializer: syntax.expressions.source_span(dependency.definition.value),
                    evaluated_value: dependency.evaluated_value.clone().ok_or_else(|| {
                        failure(
                            *reference,
                            "declaration geometry selected an unfinished dependency",
                        )
                    })?,
                };
                if !selections.contains(&origin) {
                    selections.push(origin);
                }
                if let Some(normalization) = &dependency.definition.normalization {
                    for origin in &normalization.selections {
                        if !selections.contains(origin) {
                            selections.push(origin.clone());
                        }
                    }
                    for operator in &normalization.builtin_operators {
                        if !builtin_operators.contains(operator) {
                            builtin_operators.push(*operator);
                        }
                    }
                    for call in &normalization.call_selections {
                        if !call_selections.contains(call) {
                            call_selections.push(*call);
                        }
                    }
                }
            }
            let authored_custody = ConstInitializerCustody {
                selections: dependency_origins(
                    &syntax,
                    &declarations,
                    &declarations[ordinal].authored_dependencies,
                    true,
                )?,
                builtin_operators: declarations[ordinal].authored_operators.clone(),
                call_selections: declarations[ordinal].authored_calls.clone(),
            };
            let declaration_custody = ConstInitializerCustody {
                selections: dependency_origins(
                    &syntax,
                    &declarations,
                    &declarations[ordinal].dependencies,
                    false,
                )?,
                builtin_operators: declarations[ordinal].readiness_operators.clone(),
                call_selections: declarations[ordinal].calls.clone(),
            };
            definition.normalization = Some(ConstInitializerNormalization {
                authored_expression,
                authored_custody,
                declaration_custody,
                result_value,
                selections,
                builtin_operators,
                call_selections,
            });
            definition.value = value;
            definition.qualification = None;
            syntax
                .items
                .replace_item(declarations[ordinal].item, Item::Const(definition.clone()));
            declarations[ordinal].definition = definition;
            declarations[ordinal].evaluated_value = Some(
                declarations[ordinal]
                    .definition
                    .normalization
                    .as_ref()
                    .unwrap()
                    .result_value
                    .clone(),
            );
            declarations[ordinal].leaves.clear();
        }
        syntax = super::const_qualifications::prove_ready(syntax, sources.clone(), bindings)?;
        for declaration in &mut declarations {
            if let Item::Const(definition) = syntax.root_item(declaration.item) {
                declaration.definition = definition.clone();
            }
        }
    }
    Ok(syntax)
}

/// Scalar declarations use the same selected-execution boundary as endpoints.
/// The declaration remains symbolic; this query grants no value or invocation.
pub(crate) fn pending_initializers_need_execution_selection(
    typed: &typed_trees::TypedTrees,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<Vec<typed_trees::constant::ConstDeclaration>, Vec<Diagnostic>> {
    let declarations = typed
        .const_declarations()
        .iter()
        .filter(|declaration| {
            domain.contains(declaration.symbol)
                && declaration.evaluated_value.is_none()
                && super::const_generic_expressions::scalar_probe_destination(
                    typed,
                    declaration.declared_type,
                )
                .is_some()
        })
        .collect::<Vec<_>>();
    let mut needs_selection = Vec::new();
    let mut dependencies = Vec::new();
    for (ordinal, declaration) in declarations.iter().enumerate() {
        let table = &typed.expression_table;
        if !table.expression_is_valid(declaration.materialized_initializer)
            || table.source_span(declaration.materialized_initializer)
                != declaration.initializer_source_span
        {
            return Err(failure(
                declaration.initializer_source_span,
                "pending scalar declaration lost its exact source initializer",
            ));
        }
        let mut probes = typed.clone();
        let owner = invocations::append_probe(
            &mut probes,
            declaration.symbol,
            format!("@pending-constant-{ordinal}"),
            declaration.materialized_initializer,
            declaration.declared_type,
        );
        let mut work = domain.clone();
        work.include(owner);
        let work = work.demanded_roots(&probes, &[owner]);
        let references = work.references(&probes);
        let selected_dependencies = references
            .authored_selection_occurrences
            .iter()
            .filter_map(|occurrence| probes.authored_declaration_selections().get(*occurrence))
            .filter_map(|selection| {
                selection
                    .target()
                    .as_resolved()
                    .map(|selected| selected.selected_symbol())
            })
            .filter_map(|symbol| {
                declarations
                    .iter()
                    .position(|candidate| candidate.symbol == symbol)
            })
            .collect::<Vec<_>>();
        let observes_layout = references.expressions.iter().any(|expression| {
            probes
                .expression_table
                .call_node(*expression)
                .is_some_and(|call| call.layout_query.is_some())
        });
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(&probes, &work)?;
        let execution = prepared.typed();
        let facts = validation::derive_pre_flow_operator_selections(execution);
        let admission = work.admission(execution, authority.clone());
        needs_selection.push(
            observes_layout
                || admission.closure_needs_execution_preparation(execution, owner, &facts),
        );
        dependencies.push(selected_dependencies);
    }
    // Only genuine selected-execution demands and their exact source-selected
    // dependency successors move to the target closure. Unrelated pending work
    // cannot borrow a sibling declaration's reason to defer.
    loop {
        let mut changed = false;
        for (ordinal, selected) in dependencies.iter().enumerate() {
            if !needs_selection[ordinal]
                && selected
                    .iter()
                    .any(|dependency| needs_selection[*dependency])
            {
                needs_selection[ordinal] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(declarations
        .into_iter()
        .enumerate()
        .filter_map(|(ordinal, declaration)| needs_selection[ordinal].then(|| declaration.clone()))
        .collect())
}

fn retain_pending_observations(
    syntax: &mut SyntaxTrees,
    declarations: &mut [Declaration],
    target_observations: &mut [bool],
) {
    loop {
        let mut changed = false;
        for (ordinal, declaration) in declarations.iter().enumerate() {
            if !target_observations[ordinal]
                && declaration
                    .dependencies
                    .iter()
                    .any(|(_, dependency)| target_observations[*dependency])
            {
                target_observations[ordinal] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (ordinal, declaration) in declarations.iter_mut().enumerate() {
        declaration.definition.pending_target_observation = target_observations[ordinal];
        if target_observations[ordinal] {
            syntax.items.replace_item(
                declaration.item,
                Item::Const(declaration.definition.clone()),
            );
        }
    }
}

fn dependency_origins(
    syntax: &SyntaxTrees,
    declarations: &[Declaration],
    dependencies: &[(SourceSpan, usize)],
    authored: bool,
) -> Result<Vec<ConstArgumentOrigin>, Vec<Diagnostic>> {
    let mut origins = Vec::new();
    for (reference, ordinal) in dependencies {
        let dependency = &declarations[*ordinal];
        let origin = ConstArgumentOrigin {
            reference: *reference,
            declaration: dependency.definition.name.source_span(),
            initializer: syntax.expressions.source_span(dependency.definition.value),
            evaluated_value: dependency
                .evaluated_value
                .clone()
                .ok_or_else(|| failure(*reference, "custody dependency has no completed value"))?,
        };
        if !origins.contains(&origin) {
            origins.push(origin);
        }
        if let Some(receipt) = &dependency.definition.normalization {
            let custody = if authored {
                &receipt.authored_custody
            } else {
                &receipt.declaration_custody
            };
            for origin in &custody.selections {
                if !origins.contains(origin) {
                    origins.push(origin.clone());
                }
            }
        }
    }
    Ok(origins)
}

fn failure(reference: SourceSpan, reason: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("constant initializer: {reason}")).with_source_span(reference)]
}

fn materialize_value(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    replacements: &HashMap<ExpressionHandle, ExpressionHandle>,
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    if let Some(replacement) = replacements.get(&expression) {
        return Ok(*replacement);
    }
    let source_span = syntax.expressions.source_span(expression);
    let value = match syntax.expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = syntax.expressions.expression_handles(elements).to_vec();
            let materialized = elements
                .into_iter()
                .map(|element| materialize_value(syntax, element, replacements))
                .collect::<Result<Vec<_>, _>>()?;
            ExpressionNode::ArrayLiteral(syntax.expressions.insert_expression_handles(materialized))
        }
        ExpressionNode::StructLiteral(mut constructor) => {
            let mut fields = syntax
                .expressions
                .struct_fields(constructor.fields)
                .to_vec();
            for field in &mut fields {
                field.value = materialize_value(syntax, field.value, replacements)?;
            }
            constructor.fields = syntax.expressions.insert_struct_fields(fields);
            ExpressionNode::StructLiteral(constructor)
        }
        _ => return Ok(expression),
    };
    let handle = syntax.expressions.insert(value);
    syntax.expressions.set_source_span(handle, source_span);
    Ok(handle)
}

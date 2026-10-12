//! Public requirement signature instances own their formals and fact graphs.
//!
//! The original declaring trait and its applied scopes remain the source of
//! every substitution. An instance copies the entire public header before
//! applying simultaneous telescopes from inner to outer; copied arguments are
//! excluded from that telescope's occurrence roster. No executable provider
//! body, selected implementation, or arithmetic equality supplies a fact.

use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::signature::{AuthoredInvocationTarget, StateParameter, StateSignature};
use typed_trees::static_applications::StaticMachineBinding;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use super::body_cloning::{
    copy_cloned_expression_type_payloads, copy_proof_facts, copy_signature_contract,
    copy_type_reference,
};
use super::const_values;

/// The complete generated lexical roster is allocated once. Dispatch-only
/// leading parameters share that parent with the public formals, but remain
/// outside the copied public requirement signature.
pub(super) struct AppliedSignatureCopy {
    pub(super) signature: StateSignature,
    pub(super) leading_parameter_symbols: HandleSpan<symbols::Symbol>,
    pub(super) contract_fact_copies: HandleSpan<typed_trees::signature::SignatureContractFactCopy>,
}

/// Copy a requirement's public interface under its original applied trait
/// scope. Retained binding rows also substitute selected machine and evidence
/// operands in the copied facts. Callers preserve or close the invocation
/// telescope according to their original declaration application; selector and
/// non-selector consumers share this complete header owner. Leading lexical
/// parameters participate in the one child allocation, independently of the
/// original public formals and their source-to-instance substitution map.
pub(super) fn copy_applied_signature(
    source: &dyn validation::CheckingInput,
    program: &mut TypedTrees,
    signature: &StateSignature,
    scope: &AppliedTraitScope<'_>,
    bindings: &[StaticMachineBinding],
    leading_parameters: &[StateParameter],
    display_name: &str,
) -> Result<AppliedSignatureCopy, Diagnostic> {
    validate_scope(source, signature, scope)?;
    let symbol = program.symbols.insert_generated_root_from(
        signature.symbol,
        program.symbols.get(signature.symbol).kind,
        display_name,
    );
    let formals = source.state_signature_parameters(signature);
    let children = program.symbols.insert_generated_children_from(
        symbol,
        leading_parameters.iter().chain(formals).map(|parameter| {
            (
                parameter.symbol,
                SymbolKind::Parameter,
                parameter.name.as_str(),
            )
        }),
    );
    let leading_parameter_symbols = HandleSpan::from_parts_or_empty(
        children.start(),
        u32::try_from(leading_parameters.len())
            .expect("generated parameter span exceeds arena capacity"),
    );
    let mut symbols = vec![(signature.symbol, symbol)];
    symbols.extend(
        formals
            .iter()
            .zip(children.handles().skip(leading_parameters.len()))
            .map(|(parameter, symbol)| (parameter.symbol, symbol)),
    );
    let authored_reach = source
        .authored_service_reach_rows_for(signature.symbol)
        .map(|row| {
            let mut row = row.clone();
            row.owner = symbol;
            for target in &mut row.targets {
                target.service = super::remapped_symbol(target.service, &symbols);
            }
            row
        })
        .collect::<Vec<_>>();
    program.authored_service_reach_rows.extend(authored_reach);
    copy_signature_payload(source, program, signature, scope, bindings, &symbols).map(
        |mut copied| {
            copied.leading_parameter_symbols = leading_parameter_symbols;
            copied
        },
    )
}

/// A caller-scoped public contract keeps its original declaration/formal
/// identities. It copies the same complete payload as an executable header,
/// but allocates no executable lexical owner or service-reach declaration.
pub(super) fn copy_public_contract(
    source: &dyn validation::CheckingInput,
    program: &mut TypedTrees,
    signature: &StateSignature,
    scope: &AppliedTraitScope<'_>,
    bindings: &[StaticMachineBinding],
) -> Result<AppliedSignatureCopy, Diagnostic> {
    validate_scope(source, signature, scope)?;
    copy_signature_payload(source, program, signature, scope, bindings, &[]).map(|mut copied| {
        // This view consumed every invocation operand. The original source
        // declaration still owns the universal family and admission roster.
        copied.signature.type_parameters = HandleSpan::empty();
        copied
    })
}

fn validate_scope(
    source: &dyn validation::CheckingInput,
    signature: &StateSignature,
    scope: &AppliedTraitScope<'_>,
) -> Result<(), Diagnostic> {
    if source
        .trait_machine_signature_by_symbol(scope.declaration, signature.symbol)
        .is_none_or(|original| original != signature)
    {
        return Err(Diagnostic::error(
            "applied signature lost its original declaring requirement",
        ));
    }
    if scope
        .substitution_scope_starts
        .first()
        .is_some_and(|start| *start != 0)
        || scope
            .substitution_scope_starts
            .windows(2)
            .any(|pair| pair[0] > pair[1])
        || scope
            .substitution_scope_starts
            .last()
            .is_some_and(|start| *start > scope.substitutions.len())
    {
        return Err(Diagnostic::error(
            "applied signature has invalid simultaneous scope boundaries",
        ));
    }
    Ok(())
}

fn copy_signature_payload(
    source: &dyn validation::CheckingInput,
    program: &mut TypedTrees,
    signature: &StateSignature,
    scope: &AppliedTraitScope<'_>,
    bindings: &[StaticMachineBinding],
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> Result<AppliedSignatureCopy, Diagnostic> {
    let formals = source.state_signature_parameters(signature);
    let binding_expression_start = program.expression_table.expression_count() + 1;
    let mut application_bindings = bindings.to_vec();
    super::body_cloning::copy_binding_type_payloads(
        Some(source),
        program,
        &mut application_bindings,
        &[],
    );
    copy_cloned_expression_type_payloads(Some(source), program, binding_expression_start, &[])?;
    let calls = super::application_calls::ApplicationCallRewrites::prepare(
        program,
        &application_bindings,
        &scope.declaration.conformance_bounds,
    )?;
    let type_start = program.type_reference_table.type_reference_count() + 1;
    let expression_start = program.expression_table.expression_count() + 1;
    let mut copied = signature.clone();
    copied.symbol = super::remapped_symbol(signature.symbol, symbols);
    copied.parameters = HandleSpan::empty();
    copied.contracts = HandleSpan::empty();
    copied.invokes = HandleSpan::empty();
    for parameter in formals {
        let mut parameter = parameter.clone();
        parameter.symbol = super::remapped_symbol(parameter.symbol, symbols);
        parameter.type_reference =
            copy_type_reference(Some(source), program, parameter.type_reference, symbols);
        program.push_state_signature_parameter(&mut copied, parameter);
    }
    copied.return_type = copy_type_reference(Some(source), program, signature.return_type, symbols);
    let mut contract_fact_copies = Vec::new();
    for declared_contract in source.state_signature_contracts(signature) {
        let contract =
            copy_signature_contract(Some(source), program, declared_contract.clone(), symbols);
        // copy_proof_facts publishes one fresh graph per exact declared row,
        // preserving this clause's fact order before applying its telescopes.
        // Retain that producing association rather than rediscovering it by
        // expression equality or a consumer's guessed clause ordinal. Source
        // is the unchanged clone of this program's original arenas: declared
        // handles retain that prefix lineage, while applied graphs append here.
        for (declared, applied) in declared_contract
            .facts
            .handles()
            .zip(contract.facts.handles())
        {
            contract_fact_copies.push(typed_trees::signature::SignatureContractFactCopy {
                declared_signature: signature.symbol,
                applied_signature: copied.symbol,
                kind: declared_contract.kind.clone(),
                declared,
                applied,
            });
        }
        program.push_state_signature_contract(&mut copied, contract);
    }
    let contract_fact_copies = program
        .signature_contract_fact_copies
        .insert_many(contract_fact_copies);
    copied.where_facts = copy_proof_facts(Some(source), program, signature.where_facts, symbols);
    for invocation in source.state_signature_invokes(signature) {
        let mut invocation = invocation.clone();
        match &mut invocation.target {
            AuthoredInvocationTarget::Parameter { symbol, .. }
            | AuthoredInvocationTarget::Service(symbol) => {
                *symbol = super::remapped_symbol(*symbol, symbols);
            }
            AuthoredInvocationTarget::Unresolved => {}
        }
        program.push_state_signature_invoke(&mut copied, invocation);
    }
    if let language_semantics::TerminationGuarantee::Terminates { premises } =
        &mut copied.termination_guarantee
    {
        for premise in premises {
            premise.subject.root = super::remapped_symbol(premise.subject.root, symbols);
        }
    }
    copy_cloned_expression_type_payloads(Some(source), program, expression_start, symbols)?;
    // Select only copied header occurrences. Replacement operand graphs belong
    // to their original caller and must not be rebound by the same telescope.
    let call_occurrences = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    let mut end = scope.substitutions.len();
    for start in scope.substitution_scope_starts.iter().rev() {
        substitute_telescope(
            source,
            program,
            &scope.substitutions[*start..end],
            symbols,
            type_start,
            expression_start,
        )?;
        end = *start;
    }
    if end != 0 {
        return Err(Diagnostic::error(
            "applied requirement signature lost its complete substitution scopes",
        ));
    }
    super::application_arguments::substitute_machine_parameter_type_references(
        program,
        &application_bindings,
        Some(type_start),
    );
    for occurrence in call_occurrences {
        calls.rewrite_expression(program, occurrence, &[]);
    }
    for handle in copied.invokes.handles() {
        let invocation = program.signature_invokes.get_mut(handle);
        let target = match &mut invocation.target {
            AuthoredInvocationTarget::Parameter { symbol, .. }
            | AuthoredInvocationTarget::Service(symbol) => symbol,
            AuthoredInvocationTarget::Unresolved => continue,
        };
        if let Some((_, selected, _)) = calls
            .target_rewrites
            .iter()
            .find(|(binder, _, _)| *binder == *target)
        {
            *target = *selected;
        }
    }
    Ok(AppliedSignatureCopy {
        signature: copied,
        leading_parameter_symbols: HandleSpan::empty(),
        contract_fact_copies,
    })
}

/// Materialize one public type graph under the same complete simultaneous
/// scopes used by signature cloning. Consumers retain their original scoped
/// application separately; this operation produces no selection or proof.
pub(super) fn copy_scoped_type_reference(
    source: &dyn validation::CheckingInput,
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    scope: &AppliedTraitScope<'_>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let type_start = program.type_reference_table.type_reference_count() + 1;
    let expression_start = program.expression_table.expression_count() + 1;
    let copied = copy_type_reference(Some(source), program, reference, &[]);
    copy_cloned_expression_type_payloads(Some(source), program, expression_start, &[])?;
    let mut end = scope.substitutions.len();
    for start in scope.substitution_scope_starts.iter().rev() {
        let substitutions = scope.substitutions.get(*start..end).ok_or_else(|| {
            Diagnostic::error("applied type lost its simultaneous telescope boundaries")
        })?;
        substitute_telescope(
            source,
            program,
            substitutions,
            &[],
            type_start,
            expression_start,
        )?;
        end = *start;
    }
    if end != 0 {
        return Err(Diagnostic::error(
            "applied type lost a complete original substitution scope",
        ));
    }
    Ok(copied)
}

/// Capture source-owned occurrences before copying any actual. Sequentially
/// scanning fresh arguments would substitute a simultaneous telescope into its
/// own caller operands and could capture same-spelled or recursive binders.
fn substitute_telescope(
    source: &dyn validation::CheckingInput,
    program: &mut TypedTrees,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    symbols: &[(SymbolHandle, SymbolHandle)],
    type_start: usize,
    expression_start: usize,
) -> Result<(), Diagnostic> {
    if substitutions
        .iter()
        .enumerate()
        .any(|(ordinal, (binder, _))| {
            substitutions[..ordinal]
                .iter()
                .any(|(previous, _)| previous == binder)
        })
    {
        return Err(Diagnostic::error(
            "applied signature repeats a binder in one simultaneous telescope",
        ));
    }
    let carrier_expression_start = program.expression_table.expression_count() + 1;
    let mut carriers = Vec::new();
    for (binder, _) in substitutions {
        let parameter = source
            .data_type_parameters
            .iter()
            .find_map(|(_, parameter)| (parameter.symbol == *binder).then_some(parameter))
            .ok_or_else(|| {
                Diagnostic::error("applied requirement signature names no original static binder")
            })?;
        if let TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference } = &parameter.kind
        {
            carriers.push((
                *binder,
                copy_type_reference(Some(source), program, *type_reference, symbols),
            ));
        }
    }
    copy_cloned_expression_type_payloads(Some(source), program, carrier_expression_start, symbols)?;
    let occurrences = program
        .type_reference_table
        .named_references()
        .filter(|(handle, _, _)| handle.arena_index() as usize >= type_start)
        .filter_map(|(handle, binder, _)| {
            substitutions
                .iter()
                .find(|(parameter, _)| *parameter == binder)
                .map(|(_, actual)| (handle, *actual))
        })
        .collect::<Vec<_>>();
    let arrays = program
        .type_reference_table
        .fixed_array_lengths()
        .filter(|(handle, _)| handle.arena_index() as usize >= type_start)
        .filter_map(|(handle, length)| match length {
            FixedArrayLength::ConstParameter { symbol, .. } => substitutions
                .iter()
                .find(|(parameter, _)| parameter == symbol)
                .map(|(_, actual)| (handle, *actual)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let values = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Name(path) = expression else {
                return None;
            };
            if path.symbol != path.head_symbol
                || program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
            {
                return None;
            }
            let (_, carrier) = carriers.iter().find(|(binder, _)| *binder == path.symbol)?;
            let (_, actual) = substitutions
                .iter()
                .find(|(binder, _)| *binder == path.symbol)?;
            Some((handle, *actual, *carrier))
        })
        .collect::<Vec<_>>();
    // Actuals live in the original scope's owner. Each use copies that graph
    // without reapplying the current telescope to the actual's own children.
    for (occurrence, actual) in occurrences {
        let replacement =
            super::body_cloning::copy_type_reference_payloads(Some(source), program, actual, &[])?;
        program
            .type_reference_table
            .substitute_from(occurrence, replacement)
            .map_err(Diagnostic::error)?;
    }
    for (array, actual) in arrays {
        let TypeReferenceNode::Named { symbol, name } =
            source.type_reference_table.type_reference(actual)
        else {
            return Err(Diagnostic::error(
                "applied array extent has no canonical const value",
            ));
        };
        if symbolic_const_binder(source, *symbol) {
            let TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(array)
            else {
                unreachable!("collected array changed kind")
            };
            let replacement = TypeReferenceNode::FixedArray {
                element_type: *element_type,
                length: FixedArrayLength::ConstParameter {
                    symbol: *symbol,
                    name: name.clone(),
                },
            };
            program
                .type_reference_table
                .substitute_node(array, replacement);
            continue;
        }
        let value = name.as_str().parse().map_err(|_| {
            Diagnostic::error(
                "applied array extent does not fit its declared static length carrier",
            )
        })?;
        program
            .type_reference_table
            .set_fixed_array_length(array, value);
    }
    for (occurrence, actual, carrier) in values {
        let value = match source.type_reference_table.type_reference(actual) {
            TypeReferenceNode::Named { symbol, name } if symbolic_const_binder(source, *symbol) => {
                let expression = super::body_rewriting::insert_subject_name(
                    &mut program.expression_table,
                    name.clone(),
                    *symbol,
                );
                program.expression_table.expression(expression).clone()
            }
            TypeReferenceNode::ConstExpression(_) => {
                let actual = super::body_cloning::copy_type_reference_payloads(
                    Some(source),
                    program,
                    actual,
                    &[],
                )?;
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(actual)
                else {
                    unreachable!("copied const expression changed kind");
                };
                program.expression_table.expression(*expression).clone()
            }
            _ => {
                let actual = super::body_cloning::copy_type_reference_payloads(
                    Some(source),
                    program,
                    actual,
                    &[],
                )?;
                const_values::value(program, actual, carrier).ok_or_else(|| Diagnostic::error(
                    "applied requirement cannot materialize a const binder as its declared public carrier",
                ))?
            }
        };
        *program.expression_table.expression_mut(occurrence) = value;
    }
    Ok(())
}

fn symbolic_const_binder(source: &dyn validation::CheckingInput, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && source
            .data_type_parameters
            .iter()
            .any(|(_, parameter)| parameter.symbol == symbol && parameter.kind.is_const_or_value())
}

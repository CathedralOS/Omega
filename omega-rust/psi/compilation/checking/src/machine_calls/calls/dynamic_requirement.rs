//! Requirement calls through a runtime selector:
//! `Trait::member(selector, data...)`.
//!
//! The selector's `dyn Trait<Args>` type supplies the trait application the
//! same way an evidence binder's bound does, so the requirement is
//! instantiated and its arguments are checked by the evidence-call path. The
//! selector itself is the dispatch operand: it is never a requirement
//! argument, and only the operands after it are matched against the
//! requirement's parameters
//! ([dynamic dispatch](../../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)).
//! A requirement that a dynamic caller cannot reach (a boundary requirement,
//! open requirement-local generics, a private contract identity or an
//! unfittable envelope) rejects here, at the call that would need it.

use super::{
    Diagnostic, ExpressionHandle, ExpressionNode, Machine, State, TypeReferenceHandle,
    ValueEnvironment, WritableRoots,
};
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::AppliedTraitScope;

/// One requirement selected through a `dyn Trait<Args>` selector, with the
/// requirement's declaring-trait arguments the selector's application implies.
pub(crate) struct DynamicSelectorCall<'program> {
    /// The selector operand's `dyn Trait<Args>` type.
    pub(crate) selector_type: TypeReferenceHandle,
    pub(crate) signature: &'program StateSignature,
    pub(crate) application: AppliedTraitScope<'program>,
}

/// Recognize `Trait::member(selector, ...)`: `receiver` names a trait and the
/// first operand is a selector. `Ok(None)` leaves every other call to its own
/// route; a recognized call that cannot dispatch is an error.
pub(crate) fn dynamic_selector_call<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    receiver: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
) -> Result<Option<DynamicSelectorCall<'program>>, String> {
    let Some(&selector) = arguments.first() else {
        return Ok(None);
    };
    // Stage 04 retargets a selector call to its instantiated slot; the slot
    // names the requirement the call was checked against.
    let target = program
        .dynamic_slot_by_symbol(target)
        .map_or(target, |slot| slot.requirement);
    if !receiver.is_valid() || program.symbols.get(receiver).kind != symbols::SymbolKind::Trait {
        return Ok(None);
    }
    let Some(selector_type) = selector_type(program, machine, state, selector) else {
        return Ok(None);
    };
    let Some((selector_trait, selector_arguments)) =
        crate::declarations::traits::dynamic_selector_application(program, selector_type)
    else {
        return Ok(None);
    };
    let trait_definition = |symbol| program.trait_definition_by_symbol(symbol);
    let (Some(selector_definition), Some(declaring)) =
        (trait_definition(selector_trait), trait_definition(receiver))
    else {
        return Err("dynamic call names an unresolved trait".to_owned());
    };
    let Some(signature) = program.trait_machine_signature_by_symbol(declaring, target) else {
        return Err(format!(
            "`{}` declares no requirement named `{}`",
            declaring.name,
            program.symbols.name(target)
        ));
    };
    let application = AppliedTraitScope::new(
        program,
        selector_definition,
        typed_trees::trait_definition::TraitApplicationArguments {
            lexical_owner: machine.symbol,
            enclosing_data_applications: &[],
            arguments: selector_arguments,
            lifetimes: &[],
        },
    )
    .and_then(|scope| scope.declaring_scope(program, declaring.symbol));
    let Some(application) = application else {
        return Err(format!(
            "a `dyn {}` selector cannot call `{}::{}`: `{}` does not inherit `{}`",
            selector_definition.name,
            declaring.name,
            signature.name,
            selector_definition.name,
            declaring.name
        ));
    };
    if let Some(reason) = crate::declarations::traits::dynamic_requirement_ineligibility(
        program, declaring, signature,
    ) {
        return Err(format!(
            "requirement `{}::{}` is absent from `dyn {}`: {reason}",
            declaring.name, signature.name, selector_definition.name
        ));
    }
    Ok(Some(DynamicSelectorCall {
        selector_type,
        signature,
        application,
    }))
}

/// Validate the operands after the selector against the requirement
/// instantiated under the selector's trait application, and a finite-family
/// requirement's static tuple against its declared roster.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_dynamic_selector_arguments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    call: &DynamicSelectorCall<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_family_tuple(program, call, machine_arguments, diagnostics);
    let requirement = crate::declarations::traits::GenericBoundRequirement {
        signature: call.signature,
        application: call.application.clone(),
    };
    super::generic_requirement::validate_named_conformance_arguments(
        program,
        machine,
        state,
        value_environment,
        &arguments[1..],
        machine_arguments,
        &requirement,
        writable_roots,
        diagnostics,
    );
}

/// A selected table holds one row per roster tuple of a finite generic
/// requirement, so the call must name exactly one complete tuple. A closed
/// static tuple selects its row here; an argument that is an ordinary runtime
/// subject (the same shape RUNTIME-VALUE-GENERICS' `Value` binders admit)
/// defers membership to the call's runtime discharge, which must prove the
/// argument belongs to the roster -- there is no JIT or fallback. A
/// requirement without a finite family takes no static machine arguments
/// through a selector.
fn validate_family_tuple(
    program: &dyn crate::CheckingInput,
    call: &DynamicSelectorCall<'_>,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let declaring = &call.application.declaration.name;
    let requirement = &call.signature.name;
    match program.finite_signature_family(call.signature) {
        typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } => {
            let roster = || {
                tuples
                    .iter()
                    .map(|tuple| format!("({})", tuple.display.join(", ")))
                    .collect::<Vec<_>>()
                    .join(" or ")
            };
            let tuple_identities: Option<Vec<String>> = machine_arguments
                .iter()
                .map(|argument| program.static_const_argument_identity(argument))
                .collect();
            let selects_exact_tuple = tuple_identities.as_deref().is_some_and(|identities| {
                identities.len() == arity
                    && tuples
                        .iter()
                        .any(|tuple| tuple.identities.as_ref() == identities)
            });
            if selects_exact_tuple {
                return;
            }
            // One runtime subject (or several) makes the call runtime-capable:
            // every position must then be a closed const or an ordinary
            // runtime subject, and the complete tuple membership is proved at
            // the call's runtime discharge instead of here.
            let runtime_capable = machine_arguments.len() == arity
                && machine_arguments.iter().any(|argument| {
                    crate::machine_calls::static_arguments::is_runtime_value_subject(
                        program, argument,
                    )
                })
                && machine_arguments.iter().all(|argument| {
                    crate::machine_calls::static_arguments::is_runtime_value_subject(
                        program, argument,
                    ) || program.static_const_argument_identity(argument).is_some()
                });
            if !runtime_capable {
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic family call `{declaring}::{requirement}` requires exactly one closed roster tuple ({}); a runtime value cannot select a family row without proved membership -- ordinary runtime subjects of that arity discharge it at the call's own proof",
                    roster()
                )));
            }
        }
        typed_trees::finite_family::FamilyProbe::NotFinite(_) => {
            if !machine_arguments.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic requirement call `{declaring}::{requirement}` supplies static machine arguments, but the requirement declares no finite family"
                )));
            }
        }
    }
}

/// The trait an expression call's receiver names (`Trait` in
/// `Trait::member(...)`), or the invalid symbol.
pub(crate) fn expression_trait_receiver(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
) -> symbols::SymbolHandle {
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(name)
            if name.symbol.is_valid()
                && program.symbols.get(name.symbol).kind == symbols::SymbolKind::Trait =>
        {
            name.symbol
        }
        _ => symbols::SymbolHandle::invalid(),
    }
}

/// The selector type an operand carries: a conversion's stated `dyn` type, or
/// the declared type of the place that holds it.
fn selector_type(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        _ => crate::value_custody::places::declared_place_type_raw(
            program, machine, state, expression,
        ),
    }
}

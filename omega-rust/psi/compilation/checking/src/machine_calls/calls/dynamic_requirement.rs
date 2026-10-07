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
    TypedTrees, ValueEnvironment, WritableRoots,
};
use typed_trees::machine::GenericConformanceBound;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;

/// One requirement selected through a `dyn Trait<Args>` selector, with the
/// requirement's declaring-trait arguments the selector's application implies.
pub(super) struct DynamicSelectorCall<'program> {
    pub(super) signature: &'program StateSignature,
    pub(super) trait_definition: &'program TraitDefinition,
    pub(super) bound: GenericConformanceBound,
}

/// Recognize `Trait::member(selector, ...)`: `receiver` names a trait and the
/// first operand is a selector. `Ok(None)` leaves every other call to its own
/// route; a recognized call that cannot dispatch is an error.
pub(super) fn dynamic_selector_call<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    receiver: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
) -> Result<Option<DynamicSelectorCall<'program>>, String> {
    let Some(&selector) = arguments.first() else {
        return Ok(None);
    };
    if !receiver.is_valid() || program.symbols.get(receiver).kind != symbols::SymbolKind::Trait {
        return Ok(None);
    }
    let Some((selector_trait, selector_arguments)) = selector_type(program, machine, state, selector)
        .and_then(|selector_type| {
            crate::declarations::traits::dynamic_selector_application(program, selector_type)
        })
    else {
        return Ok(None);
    };
    let trait_definition = |symbol| {
        program
            .traits()
            .iter()
            .find(|definition| definition.symbol == symbol)
    };
    let (Some(selector_definition), Some(declaring)) =
        (trait_definition(selector_trait), trait_definition(receiver))
    else {
        return Err("dynamic call names an unresolved trait".to_owned());
    };
    let Some(signature) = program
        .trait_machine_signatures(declaring)
        .iter()
        .find(|signature| signature.symbol == target)
    else {
        return Err(format!(
            "`{}` declares no requirement named `{}`",
            declaring.name,
            program.symbols.name(target)
        ));
    };
    let Some(arguments) = crate::declarations::traits::arguments_for_declaring_trait(
        program,
        selector_definition,
        selector_arguments,
        declaring.symbol,
        &mut Vec::new(),
    ) else {
        return Err(format!(
            "a `dyn {}` selector cannot call `{}::{}`: `{}` does not inherit `{}`",
            selector_definition.name,
            declaring.name,
            signature.name,
            selector_definition.name,
            declaring.name
        ));
    };
    if let Some(reason) =
        crate::declarations::traits::dynamic_requirement_ineligibility(program, declaring, signature)
    {
        return Err(format!(
            "requirement `{}::{}` is absent from `dyn {}`: {reason}",
            declaring.name, signature.name, selector_definition.name
        ));
    }
    Ok(Some(DynamicSelectorCall {
        signature,
        trait_definition: declaring,
        bound: GenericConformanceBound {
            binder: symbols::SymbolHandle::invalid(),
            binder_name: typed_trees::name::Identifier::generated_static("dyn"),
            trait_symbol: declaring.symbol,
            trait_name: declaring.name.clone(),
            arguments,
        },
    }))
}

/// Validate the operands after the selector against the requirement
/// instantiated under the selector's trait application, and a finite-family
/// requirement's static tuple against its declared roster.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_dynamic_selector_arguments(
    program: &TypedTrees,
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
        trait_definition: call.trait_definition,
        bound: &call.bound,
    };
    super::generic_requirement::validate_named_conformance_arguments(
        program,
        machine,
        state,
        value_environment,
        &arguments[1..],
        &requirement,
        writable_roots,
        diagnostics,
    );
}

/// A selected table holds one row per roster tuple of a finite generic
/// requirement, so the call must name exactly one complete closed tuple: a
/// runtime value cannot select a family row. A requirement without a finite
/// family takes no static machine arguments through a selector.
fn validate_family_tuple(
    program: &TypedTrees,
    call: &DynamicSelectorCall<'_>,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let declaring = &call.trait_definition.name;
    let requirement = &call.signature.name;
    match program.finite_signature_family(call.signature) {
        typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } => {
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
            if !selects_exact_tuple {
                let roster = tuples
                    .iter()
                    .map(|tuple| format!("({})", tuple.display.join(", ")))
                    .collect::<Vec<_>>()
                    .join(" or ");
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic family call `{declaring}::{requirement}` requires exactly one closed roster tuple ({roster}); a runtime value cannot select a family row"
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
pub(super) fn expression_trait_receiver(
    program: &TypedTrees,
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
    program: &TypedTrees,
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

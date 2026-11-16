//! The structural callable judgment used by specialization and record inspection.
//! Both use the published contract even when a particular generated call could
//! meet a stronger requirement. A comparison needing a proof stays deferred.

use super::{Served, Stop, unserved};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{
    TypeParameter, TypeParameterKind, type_parameter_demands_stronger_properties,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// What a binder-side declaration stands for while the callback is compared
/// with the binder.
#[derive(Clone, Copy)]
pub(super) enum Bound {
    /// An enclosing type parameter, bound at this call.
    Type(TypeReferenceHandle),
    /// The binder's type parameter, matched by the callback's own one.
    Parameter(SymbolHandle),
}

/// The callback must refine the published binder (generics.md). This checks
/// the parts that need no proof: the same static parameter, the same
/// parameter and result types once the call's arguments are substituted, no
/// precondition the binder lacks, and no effect the binder rules out. A
/// binder contract that only a refinement proof could discharge is unserved.
pub(super) fn satisfies_binder(
    program: &TypedTrees,
    binder: &StateSignature,
    machine: &Machine,
    binder_description: &str,
    bound: &[(SymbolHandle, Bound)],
    applications: Option<&[TypeReferenceHandle]>,
) -> Served<()> {
    let callback = program.symbols.name(machine.symbol);
    let refuse = |reason: &str| {
        Stop::Rejected(format!(
            "`{callback}` does not satisfy the callback binder of {binder_description}: {reason}"
        ))
    };
    let required_parameters = program.state_signature_type_parameters(binder);
    let supplied_parameters = program.machine_callable_parameters(machine);
    if !binder.lifetime_parameters.is_empty()
        || !binder.conformance_bounds.is_empty()
        || !binder.invokes.is_empty()
        || !binder.native_callback_parameters.is_empty()
        || required_parameters
            .iter()
            .any(|parameter| parameter.kind != TypeParameterKind::Type)
    {
        return Err(unserved(
            "a callback binder needs a refinement proof for its full interface",
        ));
    }
    if supplied_parameters.len() != required_parameters.len()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !supplied_parameters
            .iter()
            .zip(required_parameters)
            .all(|(supplied, required)| {
                supplied.kind == required.kind
                    && properties_match(
                        program,
                        required,
                        supplied,
                        applications.filter(|_| required_parameters.len() == 1),
                    )
            })
    {
        return Err(refuse("its static parameters do not match the binder's"));
    }
    let mut map = bound.to_vec();
    map.extend(
        required_parameters
            .iter()
            .zip(supplied_parameters)
            .map(|(required, supplied)| (required.symbol, Bound::Parameter(supplied.symbol))),
    );
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or(unserved("the callback has no entry"))?;
    let required = program.state_signature_parameters(binder);
    let supplied = program.state_parameters(entry);
    if required.len() != supplied.len() {
        return Err(refuse(&format!(
            "the binder takes {} parameters and the callback takes {}",
            required.len(),
            supplied.len()
        )));
    }
    for (position, (required, supplied)) in required.iter().zip(supplied).enumerate() {
        if !same_type(
            program,
            required.type_reference,
            supplied.type_reference,
            &map,
        )? {
            return Err(refuse(&format!(
                "parameter {} `{}` does not have the type of the binder's `{}`",
                position + 1,
                supplied.name.as_str(),
                required.name.as_str(),
            )));
        }
    }
    if !same_type(program, binder.return_type, entry.return_type, &map)? {
        return Err(refuse("its result type is not the binder's"));
    }
    let binder_contracts = program.state_signature_contracts(binder);
    if binder_contracts
        .iter()
        .any(|contract| !matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    {
        return Err(unserved(
            "a callback binder with its own contract needs a refinement proof",
        ));
    }
    let contracts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .collect::<Vec<_>>();
    if contracts.iter().any(|contract| {
        contract.kind == SignatureContractKind::Requires
            && (required_parameters.is_empty() || !admits_types_only(program, contract))
    }) {
        return Err(refuse(
            "it declares a precondition on its inputs that the binder does not publish, so a caller of the binder could not establish it",
        ));
    }
    if contracts.iter().any(|contract| contract.is_crashes()) && binder.declares_no_crash_route {
        return Err(refuse(
            "it may crash, and the binder declares `crashes none`",
        ));
    }
    if binder_contracts
        .iter()
        .any(|contract| contract.is_crashes())
    {
        return Err(unserved(
            "a callback binder with explicit crash routes needs a refinement proof",
        ));
    }
    if (machine.suspends && !binder.suspends) || (machine.blocks && !binder.blocks) {
        return Err(refuse("it may suspend or block, and the binder does not"));
    }
    if binder.termination_guarantee != language_semantics::TerminationGuarantee::NoGuarantee {
        return Err(unserved(
            "a callback binder with a termination promise needs a refinement proof",
        ));
    }
    Ok(())
}

/// An ordinary callback must admit the binder's whole type family. A finite
/// per-member expansion instead knows every application: extra multiplicity
/// bounds must hold for every included field. This does not weaken any runtime
/// precondition or permit a qualification change. Carry refinement still needs
/// its ordinary evidence and is not inferred from field multiplicity.
fn properties_match(
    program: &TypedTrees,
    required: &TypeParameter,
    supplied: &TypeParameter,
    applications: Option<&[TypeReferenceHandle]>,
) -> bool {
    if !type_parameter_demands_stronger_properties(required, supplied) {
        return true;
    }
    if supplied.bounds.carry.is_some() && supplied.bounds.carry != required.bounds.carry {
        return false;
    }
    applications.is_some_and(|types| {
        types.iter().all(|reference| {
            program
                .type_reference_table
                .contains_type_reference(*reference)
                && program.stored_value_multiplicity(*reference) == supplied.bounds.multiplicity
        })
    })
}

/// Whether a `requires` clause only restricts which types the callback
/// admits, such as `Kind == u32 || Kind == i32`. It names no runtime value,
/// so a caller of the binder has nothing to establish: it is a fact about
/// each field's application, and the specialization of every generated call
/// rejects an application outside what the callback admits (reflection.md:
/// "An unsupported included field rejects with its path and failed
/// obligation").
fn admits_types_only(
    program: &TypedTrees,
    contract: &typed_trees::signature::SignatureContract,
) -> bool {
    use typed_trees::domain::ProofFact;
    let facts = program.proof_facts.span_or_empty(contract.facts);
    !facts.is_empty()
        && facts.iter().all(|fact| {
            let ProofFact::Expression(root) = fact else {
                return false;
            };
            let mut types_only = root.is_valid();
            program.subtree_for_each(*root, typed_trees::SubtreeEdges::default(), |_, node| {
                types_only &= matches!(
                    node,
                    ExpressionNode::TypeExpression(_)
                        | ExpressionNode::Boolean(_)
                        | ExpressionNode::Binary(_)
                        | ExpressionNode::Unary(_)
                );
            });
            types_only
        })
}

/// Whether the binder's `required` type is the callback's `supplied` type,
/// reading each binder-side parameter through `map`. Types that mention no
/// mapped parameter compare by their normalized identity.
fn same_type(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    supplied: TypeReferenceHandle,
    map: &[(SymbolHandle, Bound)],
) -> Served<bool> {
    let table = &program.type_reference_table;
    match (
        table.type_reference(required),
        table.type_reference(supplied),
    ) {
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => Ok(true),
        (
            TypeReferenceNode::Reference {
                referee: required,
                access: required_access,
                lifetime: required_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: supplied,
                access: supplied_access,
                lifetime: supplied_lifetime,
            },
        ) => {
            if required_lifetime.is_some() || supplied_lifetime.is_some() {
                return Err(unserved(
                    "a callback parameter with a named lifetime is not compared yet",
                ));
            }
            Ok(
                required_access == supplied_access
                    && same_type(program, *required, *supplied, map)?,
            )
        }
        (TypeReferenceNode::Named { symbol, .. }, _)
            if let Some((_, bound)) = map.iter().find(|(binder, _)| binder == symbol) =>
        {
            Ok(match *bound {
                Bound::Parameter(parameter) => matches!(
                    table.type_reference(supplied),
                    TypeReferenceNode::Named { symbol, .. } if *symbol == parameter
                ),
                Bound::Type(actual) => {
                    program.normalized_type_identity(actual)
                        == program.normalized_type_identity(supplied)
                }
            })
        }
        (
            TypeReferenceNode::Named { symbol: left, .. },
            TypeReferenceNode::Named { symbol: right, .. },
        ) => Ok(left == right),
        _ => {
            if mentions(program, required, map) || mentions(program, supplied, map) {
                return Err(unserved(
                    "a callback parameter of this shape is not compared yet",
                ));
            }
            Ok(program.normalized_type_identity(required)
                == program.normalized_type_identity(supplied))
        }
    }
}

/// Whether `reference` names a mapped binder-side parameter or the
/// callback's own type parameter anywhere inside it.
fn mentions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    map: &[(SymbolHandle, Bound)],
) -> bool {
    let table = &program.type_reference_table;
    let named = |symbol: SymbolHandle| {
        map.iter().any(|(binder, bound)| {
            *binder == symbol || matches!(bound, Bound::Parameter(own) if *own == symbol)
        })
    };
    match table.type_reference(reference) {
        // Callable clauses and signature types can name the mapped binder.
        TypeReferenceNode::Callable { .. } => {
            typed_trees::references::type_reference_inputs(program, reference)
                .symbols
                .into_iter()
                .any(named)
        }
        TypeReferenceNode::Named { symbol, .. } => named(*symbol),
        TypeReferenceNode::Reference { referee, .. } => mentions(program, *referee, map),
        TypeReferenceNode::Constrained { base_type, .. } => mentions(program, *base_type, map),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => mentions(program, *element_type, map),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            named(*base_symbol)
                || table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| mentions(program, *argument, map))
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_callback_copy_bounds_keep_exclusive_references_affine() {
        let mut program = TypedTrees::default();
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut reference = |access| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: unit,
                    access,
                    lifetime: None,
                })
        };
        let shared = reference(language_core::ReferenceAccess::Shared);
        let exclusive = reference(language_core::ReferenceAccess::Mutable);
        let required = TypeParameter::default();
        let supplied = TypeParameter {
            bounds: typed_trees::data::DataProperties {
                multiplicity: language_semantics::Multiplicity::Unrestricted,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!properties_match(&program, &required, &supplied, None));
        assert!(properties_match(
            &program,
            &required,
            &supplied,
            Some(&[shared])
        ));
        assert!(!properties_match(
            &program,
            &required,
            &supplied,
            Some(&[shared, exclusive])
        ));
        assert!(properties_match(&program, &required, &supplied, Some(&[])));
    }
}

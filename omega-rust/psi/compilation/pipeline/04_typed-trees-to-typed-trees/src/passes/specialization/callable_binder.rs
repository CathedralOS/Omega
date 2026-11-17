//! The structural callable judgment used by specialization and record inspection.
//! Both use the published contract even when a particular generated call could
//! meet a stronger requirement. A comparison needing a proof stays deferred.

use super::{Served, Stop, unserved};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::callable_signature_shapes::CallableShapeMismatch;
use typed_trees::data::{
    TypeParameter, TypeParameterKind, type_parameter_demands_stronger_properties,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::types::TypeReferenceHandle;

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
    if !binder.conformance_bounds.is_empty()
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
    let substitutions = map
        .iter()
        .filter_map(|(parameter, bound)| match bound {
            Bound::Type(actual) => Some((*parameter, *actual)),
            Bound::Parameter(_) => None,
        })
        .collect::<Vec<_>>();
    let corresponding_parameters = map
        .iter()
        .filter_map(|(parameter, bound)| match bound {
            Bound::Parameter(supplied) => Some((*parameter, *supplied)),
            Bound::Type(_) => None,
        })
        .collect::<Vec<_>>();
    let mismatches = program
        .compare_callback_signature_types(
            binder,
            machine,
            &substitutions,
            &corresponding_parameters,
        )
        .map_err(unserved)?;
    if let Some(mismatch) = mismatches.first() {
        return Err(refuse(&match *mismatch {
            CallableShapeMismatch::ParameterType { position } => format!(
                "parameter {} `{}` does not have the type of the binder's `{}`",
                position + 1,
                supplied[position].name.as_str(),
                required[position].name.as_str(),
            ),
            CallableShapeMismatch::ParameterMode { position } => format!(
                "parameter {} `{}` does not have the passing mode of the binder's `{}`",
                position + 1,
                supplied[position].name.as_str(),
                required[position].name.as_str(),
            ),
            CallableShapeMismatch::ReturnType => "its result type is not the binder's".to_owned(),
            CallableShapeMismatch::ParameterArity { .. }
            | CallableShapeMismatch::GenericArity { .. } => {
                "its parameter count is not the binder's".to_owned()
            }
        }));
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
            && (!admits_types_only(program, contract)
                || required_parameters.is_empty()
                    && !program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .all(|fact| {
                            let typed_trees::domain::ProofFact::Expression(expression) = fact
                            else {
                                return false;
                            };
                            program.applied_static_type_predicate_value(*expression, &substitutions)
                                == Some(true)
                        }))
    }) {
        return Err(refuse(
            "it declares a precondition on its inputs that the binder does not publish, so a caller of the binder could not establish it",
        ));
    }
    // Only authored routes are known here. A private callback's inferred
    // routes exist after lowering, so Terminal Psi closure compares them with
    // the binder's `crashes none` (`ClosedReachMachineBinding`).
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

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn callback_binder_matches_named_outer_lifetimes_and_nested_type_arguments() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::name::Identifier;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;

        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
        let callback = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("callback"));
        let parameter = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut outer = Machine {
            symbol: owner,
            name: Identifier::from("owner"),
            lifetime_parameters: vec![Identifier::from("input")],
            ..Default::default()
        };
        program.push_machine_state(
            &mut outer,
            State {
                symbol: owner,
                return_type: unit,
                ..Default::default()
            },
        );
        program.push_machine(outer);
        let named = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: unit,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from("input")),
            });
        let elided = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: unit,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let generic = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: Identifier::from("T"),
            });
        let required_result = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: generic,
            });
        let actual_result = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: unit });
        let binder = StateSignature {
            symbol: owner,
            parameters: program.state_parameters.insert_many([StateParameter {
                type_reference: named,
                ..Default::default()
            }]),
            return_type: required_result,
            ..Default::default()
        };
        let parameters = program.state_parameters.insert_many([StateParameter {
            type_reference: elided,
            ..Default::default()
        }]);
        let mut machine = Machine {
            symbol: callback,
            name: Identifier::from("callback"),
            ..Default::default()
        };
        program.push_machine_state(
            &mut machine,
            State {
                symbol: callback,
                parameters,
                return_type: actual_result,
                ..Default::default()
            },
        );
        program.push_machine(machine.clone());
        assert!(
            satisfies_binder(
                &program,
                &binder,
                &machine,
                "test",
                &[(parameter, Bound::Type(unit))],
                None
            )
            .is_ok()
        );
        let mut wrong = machine.clone();
        let wrong_parameters = program.state_parameters.insert_many([StateParameter {
            type_reference: elided,
            is_mutable: true,
            ..Default::default()
        }]);
        wrong.states = Default::default();
        program.push_machine_state(
            &mut wrong,
            State {
                symbol: callback,
                parameters: wrong_parameters,
                return_type: actual_result,
                ..Default::default()
            },
        );
        assert!(
            matches!(satisfies_binder(&program, &binder, &wrong, "test", &[(parameter, Bound::Type(unit))], None), Err(Stop::Rejected(message)) if message.contains("passing mode"))
        );
    }

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

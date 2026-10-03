//! Borrowed aggregates retain their declared owner and exact call occurrence.
//! Fixed arrays loan the same established storage as records; their formal
//! shape retains the length and element type without a nominal declaration.
//! Records may retain supported field projections; scalar sums are observed
//! whole at their established home, including ordinary returned case values.
//! The implicit receiver precedes explicit actuals but has no row in their
//! observation roster. Validate it separately against actuals whose storage
//! remains live during the invocation; scalar evaluation has already finished.
//! A projected operand has two identities: the caller's storage root and the
//! authored endpoint captured by the call. Reconstruct both from the source;
//! an equal endpoint type does not permit substituting a sibling or another root.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::machine::Machine;
use checked_trees::signature::StateParameter;
use checked_trees::state::State;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

/// Receiver overlap uses the complete source-replayed roster partitioned by
/// already admitted explicit formals. A scalar value carries no continuing loan.
pub(super) enum ArgumentOccurrence<'a> {
    Receiver {
        parameters: &'a [StateParameter],
        access_positions: &'a [super::mixed_arguments::access_occurrences::ArgumentAccessExtent],
    },
    Explicit(usize),
}

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target_owner: &Machine,
    target: &StateParameter,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    occurrence: ArgumentOccurrence<'_>,
) -> Result<(), LoweringError> {
    let shell = validation::reference_shell(&checked.typed, target.type_reference).ok_or(
        LoweringError::Unsupported("record operand requires a readable reference formal"),
    )?;
    let target_access = shell.access;
    let referee = validation::peel_declared_constraints(&checked.typed, shell.referee);
    let (expected_access, expected_kind) = match target_access {
        ReferenceAccess::Shared => (
            CheckedStructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Read,
        ),
        ReferenceAccess::Mutable => (
            CheckedStructuralAccess::MutableBorrow,
            checked_trees::BorrowAccessKind::Mutable,
        ),
        ReferenceAccess::WriteOnly => {
            return unsupported("record operand requires readable access");
        }
    };
    if argument.access != expected_access {
        return unsupported("record operand changed its formal borrow access");
    }
    let table = &checked.expression_table;
    let named = match table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == target_access => borrow.target,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            expression
        }
        _ => return unsupported("record operand has no named or projected source"),
    };
    let (caller, _) = super::authored_state(checked, state.symbol)?;
    let source = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => {
            let source = crate::emission::call_source_custody::projected_receivers::source(
                checked,
                caller.symbol,
                state.symbol,
                statement as usize,
                named,
            )?;
            if source.path != argument.path {
                return unsupported("record operand substituted its projected path");
            }
            source
        }
        _ => return unsupported("record operand has no supported source owner"),
    };
    let root = source.root;
    let endpoint = source.endpoint();
    let parameters = checked.state_parameters(state);
    let reference = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            if symbol != root
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("record operand substituted its local owner");
            }
            let local = super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "record operand lost its local declaration",
                "record operand has no exact prior establishment",
            )?;
            // The loan rejoins the current original backing, not an immutable
            // snapshot of the initializer's scalar fields. An established
            // shared-borrow local carries that loan itself, so the carrier the
            // callee observes is the referent it dereferences to — the same
            // rule a shared-borrow parameter follows.
            match checked.type_reference_table.type_reference(
                validation::peel_declared_constraints(&checked.typed, local.type_reference),
            ) {
                TypeReferenceNode::Reference {
                    access, referee, ..
                } if !local.is_mutable
                    && (*access == ReferenceAccess::Mutable
                        || (*access == ReferenceAccess::Shared
                            && target_access == ReferenceAccess::Shared)) =>
                {
                    *referee
                }
                // Lending exclusive receiver custody follows the ordinary
                // receiver lane's mutable-local requirement.
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
                    if target_access == ReferenceAccess::Shared || local.is_mutable =>
                {
                    local.type_reference
                }
                _ => return unsupported("record operand widens source access"),
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let source = super::structural_parameter_at(
                checked,
                parameters,
                parameter_index,
                "record operand lost its structural parameter",
            )?;
            if source.is_const
                || source.symbol != root
                || !super::parameter_binding_is_unique(checked, state, root)
            {
                return unsupported("record operand substituted its source parameter");
            }
            let reference = match checked.type_reference_table.type_reference(
                validation::peel_declared_constraints(&checked.typed, source.type_reference),
            ) {
                TypeReferenceNode::Reference {
                    access, referee, ..
                } if *access == ReferenceAccess::Mutable
                    || (*access == ReferenceAccess::Shared
                        && target_access == ReferenceAccess::Shared) =>
                {
                    *referee
                }
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
                    if target_access == ReferenceAccess::Shared
                        || source.is_mutable
                        || source.is_self =>
                {
                    source.type_reference
                }
                _ => return unsupported("record operand widens source access"),
            };
            if source.is_self {
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("record source self lost its nominal attachment");
                };
                if *symbol != caller.symbol && *symbol != caller.attached_data_symbol {
                    return unsupported("record source self changed its attachment");
                }
                // Projected loans reconstruct the endpoint from the authored
                // place below. Their enclosing attachment need not have an
                // incidentally interned named type; its identity was checked
                // above independently of the producer's argument plan.
                if argument.path.is_empty() {
                    checked
                        .type_reference_table
                        .find_named_type_reference(caller.attached_data_symbol)
                        .ok_or(LoweringError::Unsupported(
                            "record source attachment has no declared type",
                        ))?
                } else {
                    reference
                }
            } else {
                reference
            }
        }
        _ => return unsupported("record operand has no supported source owner"),
    };
    let reference = if argument.path.is_empty() {
        reference
    } else {
        validation::declared_place_type_raw(&checked.typed, caller, Some(state), named).ok_or(
            LoweringError::Unsupported("record projection lost its declared type"),
        )?
    };
    let carrier = validation::peel_declared_constraints(&checked.typed, reference);
    if carrier != reference
        && !validation::has_plain_owned_contents_with_represented_constraints(
            &checked.typed,
            reference,
        )
    {
        return unsupported("borrowed operand has unrepresented source constraints");
    }
    match checked.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { .. } => {}
        TypeReferenceNode::Named { symbol, .. } => {
            let declaration = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *symbol)
                .ok_or(LoweringError::Unsupported(
                    "borrowed nominal operand has no declaration",
                ))?;
            let members = checked.data_members(declaration);
            if members
                .iter()
                .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
                && (checked.type_multiplicity(reference)
                    == language_semantics::Multiplicity::Linear
                    || !members.iter().all(|member| {
                        let checked_trees::data::DataMember::Variant(case) = member else {
                            return false;
                        };
                        checked.data_payload_fields(case).iter().all(|field| {
                            !field.relevance.is_erased()
                                && matches!(
                                    checked
                                        .type_reference_table
                                        .type_reference(field.type_reference),
                                    TypeReferenceNode::Named { .. }
                                )
                        })
                    }))
            {
                return unsupported("borrowed case operand requires a non-erased nominal sum");
            }
        }
        _ => return unsupported("borrowed operand lost its exact aggregate source type"),
    }
    // Both explicit context arguments and receivers loan existing storage.
    // Neither copies the referent or establishes its provider fields. Fixed
    // byte-array references present a bounded byte view over the same backing;
    // its carrier retains the reference identity. Reconstruct that presentation
    // from the exact authored formal, not the producer's requested identity.
    // The source/formal referee equality and occurrence checks below still own
    // the storage identity and loan authority; signature replay owns its shape.
    let presentation = if target.is_self {
        carrier
    } else {
        match validation::byte_sequence_carrier(&checked.typed, target.type_reference, &[]) {
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }) => {
                target.type_reference
            }
            // A qualified bounded owner retains its live extent. Borrowing it
            // presents the same storage carrier, not the reference's spelling;
            // signature and Terminal argument checks retain its predicates.
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }) | None => carrier,
            _ => return unsupported("aggregate operand changed its byte carrier"),
        }
    };
    if checked.normalized_type_identity(presentation).as_str() != argument.type_identity {
        return unsupported("record operand changed its declared referent type");
    }
    if target.is_self {
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(reference)
        else {
            return unsupported("record receiver lost its nominal source type");
        };
        let ArgumentOccurrence::Receiver {
            parameters,
            access_positions,
        } = occurrence
        else {
            return unsupported("record receiver lost its explicit argument partition");
        };
        let TypeReferenceNode::Named {
            symbol: self_symbol,
            ..
        } = checked.type_reference_table.type_reference(referee)
        else {
            return unsupported("record receiver lost its self referent");
        };
        if (*self_symbol != target_owner.symbol
            && *self_symbol != target_owner.attached_data_symbol)
            || *symbol != target_owner.attached_data_symbol
            || !call.has_receiver
            // An indexed receiver has no terminal member symbol. Its call
            // occurrence retains the absent stamp; the reconstructed root,
            // full path and endpoint type above establish its storage identity.
            || call.receiver_symbol != source.stamp
        {
            return unsupported("record receiver disagrees with its resolved attachment");
        }
        let accesses = checked
            .facts
            .borrow
            .argument_accesses
            .span(call.accesses)
            .ok_or(LoweringError::Unsupported(
                "record receiver has stale explicit accesses",
            ))?;
        if parameters.len() != access_positions.len() {
            return unsupported("record receiver lost its explicit argument partition");
        }
        for (position, parameter) in parameters.iter().enumerate() {
            if !checked
                .type_reference_table
                .contains_type_reference(parameter.type_reference)
            {
                return unsupported("record receiver has a stale explicit formal");
            }
            // Completed copies do not retain custody. Exclusive loans during
            // argument evaluation still conflict with the retained receiver,
            // even when a nested call ends them before producing a scalar.
            let start = access_positions[position].start;
            let end = access_positions[position].end;
            let retained = accesses.get(start..end).ok_or(LoweringError::Unsupported(
                "record receiver has an invalid explicit argument partition",
            ))?;
            if retained.iter().any(|access| {
                (access.end_before_access.is_none() || access.kind.is_exclusive())
                    && (access.root_symbol == root || access.root_symbol == endpoint)
                    && (expected_access == CheckedStructuralAccess::MutableBorrow
                        || access.kind.is_exclusive())
            }) {
                return unsupported("record receiver overlaps an incompatible actual");
            }
        }
    } else {
        let ArgumentOccurrence::Explicit(position) = occurrence else {
            return unsupported("record argument lost its observation position");
        };
        if checked.normalized_type_identity(referee) != checked.normalized_type_identity(carrier) {
            return unsupported("record argument differs from its exact formal referent");
        }
        super::borrow_rows::validate_place_argument_at(
            checked,
            call,
            position,
            source.captured_place(),
            expected_kind,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};

    #[test]
    fn qualified_byte_field_computation_replays_exact_path_access_and_carrier() {
        let checked = crate::front_end::checked_program(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/domains/mutable_receiver_declared_field_exit/main.omg"
        )));
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("main"))
            .expect("qualified field computations lower through their real caller");

        let observe = checked
            .machines()
            .iter()
            .find(|owner| owner.name.as_str() == "observe")
            .unwrap()
            .symbol;
        let mut changed = checked.clone();
        let effects = &mut changed.facts.flow.terminal_unit_effects;
        let parameter = effects
            .machines
            .iter_mut()
            .filter(|plan| plan.machine == observe)
            .flat_map(|plan| &mut plan.structural_parameters)
            .chain(
                effects
                    .composed_machines
                    .iter_mut()
                    .filter(|plan| plan.machine == observe)
                    .flat_map(|plan| &mut plan.states)
                    .flat_map(|state| &mut state.structural_parameters),
            )
            .find(|parameter| !parameter.qualifications.is_empty())
            .expect("observe retains its declared byte qualification");
        parameter.qualifications.clear();
        assert!(
            crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("main")).is_err(),
            "borrow binding cannot erase the callee's authored qualification"
        );
        for name in ["replace", "Packet::replace"] {
            let owner = checked
                .machines()
                .iter()
                .find(|owner| owner.name.as_str() == name)
                .unwrap();
            let state = &checked.machine_states(owner)[0];
            let plans = &checked.facts.values.scalar_computations;
            let root = plans
                .root_at(state.symbol, 2, CheckedScalarExpressionRole::Return)
                .expect("qualified field call keeps its return computation")
                .clone();
            let expression = plans.nodes.get(root.root).authored_root;
            let replay = |candidate: &checked_trees::CheckedTrees| {
                super::super::validate_computation_calls(
                    candidate,
                    root.machine,
                    root.state,
                    root.statement_ordinal,
                    root.root,
                    expression,
                )
            };
            replay(&checked).expect("qualified field borrow replays independently");
            let CheckedScalarComputationKind::Select { when_true, .. } =
                plans.nodes.get(root.root).kind
            else {
                panic!("short-circuit return");
            };
            let CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } = plans.nodes.get(when_true).kind
            else {
                panic!("selected observe call");
            };
            for mutation in 0..3 {
                let mut candidate = checked.clone();
                let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
                    candidate
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .get_mut(structural_arguments.start())
                else {
                    panic!("projected field operand");
                };
                match mutation {
                    0 => argument.path.clear(),
                    1 => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
                    2 => argument.type_identity.push_str("-substituted"),
                    _ => unreachable!(),
                }
                assert!(replay(&candidate).is_err(), "{name}, mutation {mutation}");
            }
        }
    }
}

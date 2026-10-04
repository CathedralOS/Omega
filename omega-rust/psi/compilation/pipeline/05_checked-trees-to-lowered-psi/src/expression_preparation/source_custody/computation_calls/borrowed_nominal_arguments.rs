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
use checked_trees::expression::ExpressionHandle;
use checked_trees::machine::Machine;
use checked_trees::signature::StateParameter;
use checked_trees::state::State;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
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
    let borrowed = super::borrowed_storage::source(
        checked,
        state,
        statement,
        target_access,
        expression,
        argument,
    )?;
    let source = borrowed.place;
    let root = source.root;
    let endpoint = source.endpoint();
    let reference = borrowed.declared_endpoint;
    let carrier = borrowed.carrier;
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
                    || !members.iter().all(|member| match member {
                        checked_trees::data::DataMember::Variant(case) => {
                            checked.data_payload_fields(case).iter().all(|field| {
                                !field.relevance.is_erased()
                                    && matches!(
                                        checked
                                            .type_reference_table
                                            .type_reference(field.type_reference),
                                        TypeReferenceNode::Named { .. }
                                    )
                            })
                        }
                        // An erased member stores nothing the borrow loans;
                        // the sum's storage stays its discriminated variants.
                        checked_trees::data::DataMember::Field(field) => {
                            field.relevance.is_erased()
                        }
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
    let presentation =
        match validation::byte_sequence_carrier(&checked.typed, target.type_reference, &[]) {
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }) => {
                target.type_reference
            }
            // A qualified bounded owner retains its live extent. Borrowing it
            // presents the same storage carrier, not the reference's spelling;
            // signature and Terminal argument checks retain its predicates.
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }) | None => carrier,
            _ => return unsupported("aggregate operand changed its byte carrier"),
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

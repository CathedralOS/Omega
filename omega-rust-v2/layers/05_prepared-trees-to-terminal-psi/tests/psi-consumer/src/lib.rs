//! A downstream client of the owned Psi package. Its manifest is the dependency boundary.
use omega_psi::{
    DeclarationKind, FoldedLayout, FoldedSubject, FoldedTargetClosure, FoldedTargetObservation, LayoutExtent, LayoutGeometry, LoweredPackage, ReceivingTarget,
    TargetIdentity, TypeShape,
};

mod assembly;
pub mod zero_count_clauses;
mod domain_completions;
mod floats;
mod ieee_predicates;
pub use ieee_predicates::{PredicateValue, evaluate_ieee_clause, float_formal, record_predicate_application};
mod trapping;
pub use assembly::{LocationSupply, assembly_branch, assembly_changed_registers, assembly_location, assembly_location_supply, assembly_state_writes};
pub use domain_completions::{cast_completions, returned_domain_cleanup};
pub use floats::{FloatScalar, evaluate_float_body};
pub use trapping::{DivisionOutcome, observe_byte_division};

/// Every terminal external exit of a body, read from published terminators alone.
pub fn external_exits(body: &omega_psi::MachineBody) -> Vec<&omega_psi::ExternalExit> {
    body.blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            omega_psi::BlockTerminator::ExternalExit(exit) => Some(exit),
            _ => None,
        })
        .collect()
}

/// Replay a package on a separately selected receiver, including the plan that supplied its geometry.
pub fn replay_layout<'a>(
    closure: &'a FoldedTargetClosure,
    profile: &str,
    plan: u8,
    size: u64,
    subject: FoldedSubject,
) -> Result<(), &'a FoldedTargetObservation> {
    let identity = TargetIdentity {
        profile: profile.to_string(),
        semantics_version: 1,
        realization: vec![7].into(),
    };
    let layout = FoldedLayout {
        target: identity.clone(),
        subject,
        schema: vec![1].into(),
        plan: vec![plan].into(),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(size),
            alignment: Some(8),
        },
    };
    let receiver = ReceivingTarget::new(identity, [layout], []).expect("valid independently selected layout");
    closure.replay(&receiver)
}

/// Follow the immediate nominal parent to its package declaration, retaining the
/// parent's applied arguments. A trait without a parent has no inherited slots.
pub fn declared_trait_parent<'a>(
    package: &'a LoweredPackage,
    child: &str,
) -> Option<Option<(&'a omega_psi::LoweredTrait, &'a [omega_psi::TypeExpression])>> {
    let child = package.traits.iter().find(|t| t.declaration.path == child)?;
    let Some(parent) = &child.parent else { return Some(None) };
    let declaration = package.traits.iter().find(|t| t.declaration == parent.declaration)?;
    Some(Some((declaration, &parent.arguments)))
}

/// Read the nominal declaration and complete whole-value memberships of a
/// structural entry formal. The signature, body place and domain catalog must agree.
pub fn record_formal<'a>(
    package: &'a LoweredPackage, owner: &str, position: usize,
) -> Option<(&'a omega_psi::LoweredData, Vec<omega_psi::DomainAtom>)> {
    let m = machine(package, owner)?;
    let omega_psi::Parameter::Structural(input) = m.body.parameters.get(position)? else { return None };
    let place = m.body.places.get(input.index())?;
    if place.origin != omega_psi::PlaceOrigin::Parameter || m.parameter_types.get(position) != Some(&place.ty) { return None; }
    let (id, set) = match package.types.get(place.ty)? {
        TypeShape::Nominal(id) => (*id, None),
        TypeShape::QualifiedNominal { data, domains } => (*data, Some(*domains)),
        _ => return None,
    };
    let record = package.data.iter().find(|d| d.id == id)?;
    let mut atoms = Vec::new();
    if let Some(set) = set {
        for (declaration, arguments) in package.types.domain_atoms(set)? {
            let domain = package.domains.iter().find(|d| &d.declaration == declaration)?;
            match package.types.get(domain.carrier)? {
                TypeShape::Nominal(owner) | TypeShape::QualifiedNominal { data: owner, .. } if *owner == id => {}
                _ => return None,
            }
            atoms.push(omega_psi::DomainAtom { declaration: declaration.clone(), arguments: arguments.to_vec() });
        }
    }
    Some((record, atoms))
}

pub fn array_parameter_length(
    package: &LoweredPackage,
    machine_name: &str,
) -> Option<u64> {
    let machine = machine(package, machine_name)?;
    match package.types.get(*machine.parameter_types.first()?)? {
        TypeShape::FixedArray { length, .. } => Some(*length),
        _ => None,
    }
}

pub fn machine_contract<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> Option<&'a omega_psi::MachineContract> {
    Some(&machine(package, name)?.contract)
}
fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> Option<&'a omega_psi::LoweredMachine> {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.kind == DeclarationKind::Machine && machine.declaration.path == name)
}

/// Read the published target and operand sequence of an executable member call.
pub fn member_call<'a>(
    package: &'a LoweredPackage,
    operation: &'a omega_psi::OperationKind,
) -> Option<(&'a omega_psi::MemberCallSelection, &'a [omega_psi::CallArgument])> {
    let omega_psi::OperationKind::Call {
        callee: omega_psi::CallTarget::Member(id),
        arguments,
        ..
    } = operation
    else {
        return None;
    };
    Some((package.member_calls.get(*id)?, arguments))
}

/// Read the parent place of a single-block machine's explicitly returned reborrow.
pub fn returned_reborrow(body: &omega_psi::MachineBody) -> Option<&omega_psi::ProjectedPlace> {
    let [block] = body.blocks.as_slice() else { return None };
    let omega_psi::BlockTerminator::Return {
        result: omega_psi::ReturnEdge::Structural { source, .. },
        ..
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::PlaceOrigin::Operation(id) = body.places.get(source.index())?.origin else {
        return None;
    };
    let op = body.operations.get(id.index())?;
    if op.result != omega_psi::OperationResult::Structural(*source) {
        return None;
    }
    match &op.kind {
        omega_psi::OperationKind::BorrowPlace { source, .. } => Some(source),
        _ => None,
    }
}

/// Pairs `(earlier, later)` of transfers out of one root whose paths are equal
/// or one is a prefix of the other, read from a straight-line body's ordered
/// operations alone. This shows that the published checker input locates a
/// repeated transfer. It is not the checker's verdict: it ignores branches,
/// refills and every other ownership rule.
pub fn overlapping_transfers(body: &omega_psi::MachineBody) -> Vec<(omega_psi::OperationId, omega_psi::OperationId)> {
    let [block] = body.blocks.as_slice() else {
        panic!("the reader replays one straight-line block")
    };
    let mut moved: Vec<(&omega_psi::ProjectedPlace, omega_psi::OperationId)> = Vec::new();
    let mut pairs = Vec::new();
    for op in &body.operations[block.operations.clone()] {
        let omega_psi::OperationKind::MoveStructural { source } = &op.kind else {
            continue;
        };
        if let Some((_, earlier)) = moved
            .iter()
            .find(|(path, _)| path.root == source.root && (path.path.starts_with(&source.path) || source.path.starts_with(&path.path)))
        {
            pairs.push((*earlier, op.id));
        }
        moved.push((source, op.id));
    }
    pairs
}

/// Follow the returned place to its defining transfer and read precisely what survives on that edge.
pub fn returned_move<'a>(
    body: &'a omega_psi::MachineBody,
    block: &'a omega_psi::BranchBlock,
) -> Option<(&'a omega_psi::ProjectedPlace, &'a [omega_psi::CleanupAction])> {
    let omega_psi::BlockTerminator::Return {
        result: omega_psi::ReturnEdge::Structural { source, .. },
        cleanup,
        ..
    } = &block.terminator
    else {
        return None;
    };
    let omega_psi::PlaceOrigin::Operation(id) = body.places.get(source.index())?.origin else {
        return None;
    };
    let operation = body.operations.get(id.index())?;
    if operation.result != omega_psi::OperationResult::Structural(*source) {
        return None;
    }
    match &operation.kind {
        omega_psi::OperationKind::MoveStructural { source } => Some((source, cleanup)),
        _ => None,
    }
}

/// Reconstruct the local crossing roster from selected machine envelopes after dropping all source trees.
pub fn suspending_local_calls(
    package: &LoweredPackage,
    body: &omega_psi::MachineBody,
) -> Vec<omega_psi::CallSite> {
    let may_park = |target: omega_psi::CallTarget| matches!(target,omega_psi::CallTarget::Machine(id) if package.machines[(id.get()-1) as usize].contract.realized_pause.suspend);
    body.blocks
        .iter()
        .flat_map(|block| {
            body.operations[block.operations.clone()]
                .iter()
                .filter_map(|op| match op.kind {
                    omega_psi::OperationKind::Call { callee, .. } if may_park(callee) => Some(omega_psi::CallSite::Operation(op.id)),
                    _ => None,
                })
                .chain(match block.terminator {
                    omega_psi::BlockTerminator::TerminalCall { id, callee, .. } if may_park(callee) => Some(omega_psi::CallSite::Edge(id)),
                    _ => None,
                })
        })
        .collect()
}

mod parked_byte_views;
pub use parked_byte_views::check_parked_byte_views;

mod stored_exclusive;
pub use stored_exclusive::{ExclusiveOrigin, ReturnedExclusive, returned_exclusive};

mod joined_loans;
pub use joined_loans::check_joined_loan;

mod exclusive_joins;
pub use exclusive_joins::{ExclusiveJoin, JoinedUse, exclusive_join, local_descriptor_move};

mod returned_view;
pub use returned_view::{recursive_view_backings, returned_view_backing};

mod element_writes;
pub use element_writes::{ElementWrite, element_writes};

mod field_stores;
pub use field_stores::{StoredDescriptor, parked_field_stores};

mod parked_arrivals;
pub use parked_arrivals::{Arrival, ArrivalSource, ParkedDescriptor, parked_descriptor_arrivals};

mod write_only;
pub use write_only::{WriteOnlyStore, closed_copy_storage, write_only_stores};

mod parked_windows;
pub use parked_windows::{ParkedWindow, StoreKind, parked_windows, window_stores};

mod parked_view_sources;
pub use parked_view_sources::{ProviderInvocation, ViewSources, parked_view_sources};

mod parked_handles;
pub use parked_handles::check_parked_handles;

mod embedded_fields;
pub use embedded_fields::{check_constructed_field, check_incoming_fields, discarded_loan_fields, parked_replacement_origin};

mod parked_owner_borrows;
pub use parked_owner_borrows::{ParkedFieldLoan, ParkedLoanBacking, check_parked_owner_borrow, parked_field_loan_carry, parked_field_loans};

mod parked_field_reborrows;
pub use parked_field_reborrows::{ParkedFieldReborrow, parked_field_reborrow};

mod returned_field_loans;
pub use returned_field_loans::{ReturnedStoredLoan, returned_stored_loan};

pub mod product_witness;

mod dependency_call;
pub use dependency_call::{ReturnedCall, returned_call};

mod theorem_citations;
pub use theorem_citations::{NestedCitation, byte_subject_addition, nested_theorem_citations};
pub use theorem_citations::{CitationObligations, ProofCitation, theorem_citations, theorem_proof};

mod hooked_transfers;
pub use hooked_transfers::{HookedFieldTransfer, hooked_field_transfers};

mod ranking;
pub use ranking::{claimed_cleanup, entry_rank, successor_from_call};

mod primitive_homes;
pub use primitive_homes::borrowed_scalar_formal;
mod copyable_homes;
pub use copyable_homes::{CopyableHome, parked_copyable_homes, plain_carry};
pub use primitive_homes::{HomeEvent, ParkedHome, PrimitiveHome, parked_homes, primitive_homes};

mod scalar_terms;
pub use scalar_terms::{Operand, Term, call_arguments, integer_comparisons};

mod callback_premises;
pub use callback_premises::{CallbackPremises, callback_premises, predicate_callback};

/// Open templates evaluated or resolved from Psi alone.
pub mod machine_templates;

mod template_operands;
pub use template_operands::{moved_formal_fields, template_field_argument};

mod view_bounds;
pub use view_bounds::{ViewElementBound, returned_element_count_formal, returned_nominal_view_count, view_element_bounds};

mod member_templates;
pub use member_templates::{
    ReturnedTemplateCall, TemplateCallee, TemplateValue, formal_borrow_interval, member_template, member_template_call, returned_template_call, template_result,
};

pub mod callable_loans;
pub mod sum_cleanup;
pub mod selector_dispatch;

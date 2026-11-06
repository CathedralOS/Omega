//! Sealed primitive result relations join the same ordinary argument and state
//! substitution as checked bodies. Authentication permits describing an
//! operation's exact loan relation; it supplies neither a grant, a profile,
//! access admission nor independent permission to use the returned view.

use super::{
    AggregateOrigins, FramePathPrecision, FramePlaceOrigin, Machine, ReferenceLeaf,
    ReferenceSource, State, TypeReferenceHandle,
};
use crate::machine_calls::calls::write_frames::stored_origins::{
    StoredOriginKind, declared_origins_for_query, place_suffix, projected_storage_type,
};
use crate::machine_calls::calls::write_frames::{
    FrameSourcePlace, RecastSource, ReferenceViewFormation,
};
use symbols::SymbolHandle;
use typed_trees::typed_trees::{AccessExecutionDeclaration, AccessLifecycleOperation};

pub(super) fn result_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expected: TypeReferenceHandle,
    include_shared: bool,
) -> Option<AggregateOrigins> {
    let mut result = installed_context_result_origins(program, machine, state, expected)
        .or_else(|| activation_borrow_result_origins(program, machine, state, expected))
        .or_else(|| access_view_result_origins(program, machine, state, expected))?;
    if !include_shared {
        result.references.retain(|leaf| {
            !matches!(leaf.kind, StoredOriginKind::Reference(access) if !access.is_exclusive())
        });
    }
    Some(result)
}

/// Recover both ends of the same sealed result relation for coordinate
/// consumers. This is type correspondence only, never access admission.
pub(in crate::machine_calls::calls::write_frames) fn extent_view_types(
    program: &dyn crate::CheckingInput,
    declaration: AccessExecutionDeclaration,
) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
    let machine = program.machine_by_symbol(declaration.machine())?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == declaration.state())?;
    let relation = access_view_result_origins(program, machine, state, state.return_type)?;
    let [leaf] = relation.references.as_slice() else {
        return None;
    };
    let ReferenceSource::Symbolic(origin) = leaf.origin.as_ref()? else {
        return None;
    };
    let [formation] = origin.source.recasts.as_slice() else {
        return None;
    };
    if formation.formation != ReferenceViewFormation::ExtentView(declaration) {
        return None;
    }
    Some((
        program.state_parameters(state).first()?.type_reference,
        formation.target_type,
    ))
}

/// The authenticated View result's exact reference leaf borrows the first
/// formal's content region. The result type supplies only the declared case
/// and field roster; the sealed operation supplies this origin relationship.
/// No lifetime, spelling or coincident numeric address supplies that relation.
fn access_view_result_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expected: TypeReferenceHandle,
) -> Option<AggregateOrigins> {
    let declaration = AccessExecutionDeclaration::authenticate(program, machine.symbol).ok()??;
    if declaration.operation() != AccessLifecycleOperation::View
        || declaration.state() != state.symbol
        || !super::super::isolation::aggregate_storage_types_match(
            program,
            state.return_type,
            expected,
        )
    {
        return None;
    }
    let [storage, _profile] = program.state_parameters(state) else {
        return None;
    };
    let storage_shell = crate::reference_shell(program, storage.type_reference)?;
    let frontier = declared_origins_for_query(
        program,
        SymbolHandle::invalid(),
        "result",
        state.return_type,
        true,
    )?;
    let [leaf] = frontier.references.as_slice() else {
        return None;
    };
    if leaf.may_select || leaf.kind != StoredOriginKind::Reference(storage_shell.access) {
        return None;
    }
    let result_type = projected_storage_type(program, state.return_type, &leaf.local_segments)?;
    let view = crate::reference_shell(program, result_type)?;
    // The authenticated primitive's declaration supplies this loan relation
    // even while its policy/schema binders are symbolic. Requiring a closed
    // physical plan here loses the relation in generic helpers. Geometry and
    // access admission belong to the concrete operation, not origin discovery.
    Some(AggregateOrigins {
        references: vec![ReferenceLeaf {
            kind: leaf.kind,
            local_suffix: place_suffix("result", &leaf.local_path)?.to_owned(),
            local_segments: leaf.local_segments.clone(),
            may_select: false,
            lifetime_dependencies: Some(Vec::new()),
            origin: Some(ReferenceSource::Symbolic(FramePlaceOrigin {
                path: storage.name.as_str().to_owned(),
                precision: FramePathPrecision::Exact,
                source: FrameSourcePlace {
                    root: storage.symbol,
                    builtin_coordinates: true,
                    representation: Some((view.referee, Vec::new())),
                    recasts: vec![RecastSource {
                        formation: ReferenceViewFormation::ExtentView(declaration),
                        root: storage.symbol,
                        segments: Vec::new(),
                        representation: None,
                        target_type: view.referee,
                    }],
                    ..Default::default()
                },
            })),
        }],
        cases: frontier.into_owned().cases,
        ..Default::default()
    })
}

// The sealed accessor's result borrows the exact input grant's region. This
// intrinsic relation substitutes the actual normally; a leaf spelling, return
// type, address, or bodyless declaration supplies no such storage authority.
// The extent/initialization and activation-local grant are checked separately
// by establishment. A readable view never manufactures an independent grant.
fn activation_borrow_result_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expected: TypeReferenceHandle,
) -> Option<AggregateOrigins> {
    let contract = crate::exact_toolchain_activation_storage_contract(program, machine)?;
    if contract.operation
        != language_core::activation_storage::ActivationStorageOperation::BorrowBytes
        || !super::super::isolation::aggregate_storage_types_match(
            program,
            state.return_type,
            expected,
        )
    {
        return None;
    }
    let [entry] = program.machine_states(machine) else {
        return None;
    };
    if entry.symbol != state.symbol {
        return None;
    }
    let [parameter] = program.state_parameters(state) else {
        return None;
    };
    Some(AggregateOrigins {
        references: vec![ReferenceLeaf {
            kind: super::super::stored_origins::StoredOriginKind::Reference(
                language_semantics::ReferenceAccess::Mutable,
            ),
            local_suffix: String::new(),
            local_segments: Vec::new(),
            may_select: false,
            lifetime_dependencies: Some(Vec::new()),
            origin: Some(ReferenceSource::Symbolic(FramePlaceOrigin {
                path: parameter.name.as_str().to_owned(),
                precision: FramePathPrecision::Exact,
                source: FrameSourcePlace::parameter_root(parameter),
            })),
        }],
        ..Default::default()
    })
}

/// The installation supplies the pointer; the acknowledgement supplies only the
/// lifetime dependency. Both survive ordinary call and stored-value transport.
fn installed_context_result_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expected: TypeReferenceHandle,
) -> Option<AggregateOrigins> {
    let context =
        crate::compiler_operations::static_interrupt_context(program, machine.symbol).ok()??;
    if context.entry != state.symbol
        || !super::super::isolation::aggregate_storage_types_match(
            program,
            state.return_type,
            expected,
        )
    {
        return None;
    }
    let [acknowledgement] = program.state_parameters(state) else {
        return None;
    };
    let frontier = declared_origins_for_query(
        program,
        SymbolHandle::invalid(),
        "result",
        state.return_type,
        true,
    )?;
    let [leaf] = frontier.references.as_slice() else {
        return None;
    };
    if leaf.may_select
        || leaf.kind != StoredOriginKind::Reference(language_semantics::ReferenceAccess::Shared)
    {
        return None;
    }
    if std::env::var_os("OMEGA_COMPILER_TRACE_INSTALLED_CONTEXT").is_some() {
        eprintln!(
            "INSTALLED_CONTEXT primitive relation machine={:?} state={:?} referent={:?} acknowledgement={:?}",
            machine.symbol, state.symbol, context.referent, acknowledgement.symbol
        );
    }
    let dependency = super::super::stored_calls::SlotSource::from_place(
        program,
        FrameSourcePlace::parameter_root(acknowledgement),
        FramePathPrecision::Exact,
        Some(language_semantics::ReferenceAccess::Shared),
    );
    Some(AggregateOrigins {
        references: vec![ReferenceLeaf {
            kind: leaf.kind,
            local_suffix: place_suffix("result", &leaf.local_path)?.to_owned(),
            local_segments: leaf.local_segments.clone(),
            may_select: false,
            lifetime_dependencies: Some(vec![dependency]),
            origin: Some(ReferenceSource::Symbolic(FramePlaceOrigin {
                path: "installed interrupt context".to_owned(),
                precision: FramePathPrecision::Exact,
                source: FrameSourcePlace::invocation_result(
                    typed_trees::expression::ExpressionHandle::invalid(),
                    context.referent,
                    Vec::new(),
                ),
            })),
        }],
        cases: frontier.into_owned().cases,
        ..Default::default()
    })
}

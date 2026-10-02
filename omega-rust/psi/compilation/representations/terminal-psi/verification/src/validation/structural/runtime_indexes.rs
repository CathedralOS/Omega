//! Runtime-index segments inside an operation's structural projections.
//!
//! A `RuntimeIndex { index, obligation }` segment is both an operand and an
//! obligation of the operation whose projection carries it (the inventory is
//! `terminal_psi::OperationKind::runtime_indexes`). This owner checks the
//! operand side and resolves, for each segment, the fixed array it selects in:
//!
//! - only operations whose own validation resolves projections through
//!   `resolve_runtime_projection` may carry one; every other consumer resolves
//!   static paths and would read a runtime segment as no place at all;
//! - the selector is an integer scalar (not an address) defined before the
//!   operation, like any other operand;
//! - each segment's prefix resolves to a fixed array or bounded byte view,
//!   whose declared extent the reconstructed obligation bounds the selector
//!   by; a segment on an element view instead names the dominating
//!   `ElementViewLength` observation of the view's place, its bound the same
//!   `index < length` relation an element-view write carries as an operand.
//!
//! The obligation itself is reconstructed in
//! `verification::reconstruction::operation_facts::runtime_index` from the
//! facts that hold before the operation, and a certificate must discharge it.
//! Nothing in the segment states a bound.

use crate::validation::{
    BTreeMap, BTreeSet, ModuleError, OperationKind, ScalarType, StructuralTypeShape,
    TerminalMachine, TerminalModule,
};
use semantic_vocabulary::{ObligationId, ValueId};

/// One runtime-selected element step of an operation's projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeIndexSite {
    pub(crate) index: ValueId,
    pub(crate) obligation: ObligationId,
    /// The extent the segment's reconstructed obligation bounds `index` by.
    pub(crate) extent: RuntimeIndexExtent,
}

/// What a `RuntimeIndex` segment's bound names: a declared element count for
/// a fixed array or bounded byte view, or the result of the dominating
/// `ElementViewLength` observation for an element view — the extent a view
/// carries is live, never a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeIndexExtent {
    /// The declared element count of the array the segment selects in.
    Literal(u64),
    /// The scalar result of the operation's dominating `ElementViewLength`
    /// observation of the projection's root view.
    ObservedLength(ValueId),
}

/// Operations whose validation resolves their projections through a runtime
/// projection resolver: calls through their structural arguments, leaf copies
/// through their source path, primitive-leaf reads and stores through
/// `terminal_semantics::primitive_projection_type`, scalar field stores
/// through their carrier path, and case observations through the path to
/// their sum. Extending this set is a claim that the operation's own checks,
/// interpretation and lowering treat a runtime segment as "some element of
/// this array"; a case observation through one therefore keys no case fact
/// (`canonical_field_path` has no spelling for it).
fn admits_runtime_indexes(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::CallUnit { .. }
            | OperationKind::CallStructuralScalar { .. }
            | OperationKind::CallStructural { .. }
            | OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::StructuralLeafCopy { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::WriteOnlyPrimitiveStore { .. }
            | OperationKind::StructuralScalarFieldStore { .. }
            | OperationKind::StructuralByteViewFieldStore { .. }
            | OperationKind::StructuralByteSequenceFieldLength { .. }
            | OperationKind::StructuralByteSequenceFieldRead { .. }
            | OperationKind::MoveStructuralField { .. }
            | OperationKind::StoreStructuralField { .. }
    )
}

/// A dynamic view never proves a literal selector from its type. Such a
/// selector still needs a RuntimeIndex operation obligation; downgrading it
/// to FixedIndex must not erase its bound while preserving the same geometry.
pub(super) fn checked_projection_type(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    for (position, segment) in path.iter().enumerate() {
        if matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_)) {
            let container = crate::validation::foundation::resolve_runtime_projection(
                module,
                root,
                &path[..position],
            )?;
            if module.structural_types.iter().any(|row| {
                row.id == container && matches!(row.shape, StructuralTypeShape::ElementView { .. })
            }) {
                return None;
            }
        }
    }
    crate::validation::foundation::resolve_runtime_projection(module, root, path)
}

/// The operand side of every runtime index the operation carries.
pub(in crate::validation) fn validate_operands(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let indexes = operation.kind.runtime_indexes();
    if indexes.is_empty() {
        return Ok(());
    }
    for (index, _) in indexes {
        let invalid = ModuleError::InvalidRuntimeIndex {
            operation: operation.id,
            index,
        };
        if !admits_runtime_indexes(&operation.kind) {
            return Err(invalid);
        }
        if !defined.contains(&index) {
            return Err(ModuleError::ValueUsedBeforeDefinition(index));
        }
        match value_types.get(&index) {
            Some(ScalarType::Integer(integer_type)) if !integer_type.is_address() => {}
            Some(_) => return Err(invalid),
            None => return Err(ModuleError::UnknownValue(index)),
        }
    }
    Ok(())
}

/// Each runtime segment of the operation's projections, in operand order,
/// with the extent of the fixed array its prefix resolves to.
pub(crate) fn runtime_index_sites(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<Vec<RuntimeIndexSite>, ModuleError> {
    let mut sites = Vec::new();
    for projection in operation.kind.structural_projections() {
        if terminal_psi::is_static_structural_path(projection.path) {
            continue;
        }
        let root_type = crate::validation::structural::operations::caller_structural_root_type(
            machine,
            projection.root,
        )
        .or_else(|| {
            crate::validation::structural::result_contracts::source_signature(
                machine,
                projection.root,
            )
            .map(|signature| signature.structural_type)
        });
        for (position, segment) in projection.path.iter().enumerate() {
            let Some((index, obligation)) = segment.runtime_index() else {
                continue;
            };
            let invalid = ModuleError::InvalidRuntimeIndex {
                operation: operation.id,
                index,
            };
            let array = root_type
                .and_then(|root_type| {
                    crate::validation::foundation::resolve_runtime_projection(
                        module,
                        root_type,
                        &projection.path[..position],
                    )
                })
                .ok_or(invalid.clone())?;
            let extent = module
                .structural_types
                .iter()
                .find_map(|declaration| match declaration.shape {
                    StructuralTypeShape::FixedArray { length, .. } if declaration.id == array => {
                        Some(RuntimeIndexExtent::Literal(length))
                    }
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
                    ) if declaration.id == array => {
                        Some(RuntimeIndexExtent::Literal(capacity))
                    }
                    // A view's extent is its stored runtime length: the
                    // segment's bound names the operation's dominating
                    // `ElementViewLength` observation of the projection's
                    // root place — only a path position of zero resolves a
                    // view at the root, where that observation is well-formed.
                    StructuralTypeShape::ElementView { .. }
                        if declaration.id == array && position == 0 =>
                    {
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|candidate| {
                                candidate.result.scalar().is_some()
                                    && matches!(
                                        candidate.kind,
                                        OperationKind::ElementViewLength { source }
                                            if source == projection.root
                                    )
                                    && crate::validation::structural::byte_sequence_fields::view_length_is_current(
                                        module,
                                        machine,
                                        candidate.id,
                                        operation.id,
                                        projection.root,
                                    )
                            })
                            .and_then(|candidate| {
                                candidate.result.scalar().map(|result| result.id)
                            })
                            .map(RuntimeIndexExtent::ObservedLength)
                    }
                    _ => None,
                })
                .ok_or(invalid)?;
            sites.push(RuntimeIndexSite {
                index,
                obligation,
                extent,
            });
        }
    }
    Ok(sites)
}

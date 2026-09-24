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
//! - each segment's prefix resolves to a fixed array, whose declared extent
//!   the reconstructed obligation bounds the selector by.
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
    /// The declared element count of the fixed array the segment selects in.
    pub(crate) extent: u64,
}

/// Operations whose validation resolves their projections through a runtime
/// projection resolver: calls through their structural arguments, leaf copies
/// through their source path, and primitive-leaf reads and stores through
/// `terminal_semantics::primitive_projection_type`. Extending this set is a
/// claim that the operation's own checks, interpretation and lowering treat a
/// runtime segment as "some element of this array".
fn admits_runtime_indexes(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::CallUnit { .. }
            | OperationKind::CallStructuralScalar { .. }
            | OperationKind::CallStructural { .. }
            | OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::StructuralLeafCopy { .. }
            | OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::WriteOnlyPrimitiveStore { .. }
    )
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
                        Some(length)
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

use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::lowering_error::LoweringError;
use checked_trees::CheckedUnitStructuralPathSegment;
use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::StructuralPathSegment;
use terminal_psi::ValueDeclaration;

/// Lower a checked static projection. A `RuntimeIndex` has no context-free
/// lowering: its Terminal segment names the evaluated index value and an
/// obligation the carrying operation owns, so the emitter that evaluates the
/// index lowers it. Claims, discards, qualifications and residuals are static
/// by construction; a runtime segment reaching this function is refused.
pub(crate) fn lower_structural_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Referent => Ok(StructuralPathSegment::Referent),
            CheckedUnitStructuralPathSegment::Field(identity) => {
                Ok(StructuralPathSegment::Field(identity.clone()))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                Ok(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex { .. } => {
                Err(LoweringError::Unsupported(
                    "a runtime-index projection is lowered by the emitter that evaluates its index",
                ))
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                Ok(StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                })
            }
        })
        .collect()
}

/// A checked argument path can carry a runtime selector only when the
/// operand lowering owns the index's evaluation and obligation: a dense
/// scalar parameter of the calling state is already in the caller's operand
/// frame, and a `StructuralField` selector consumes the field read
/// `mint_runtime_index_selectors` emitted in argument order ahead of this
/// lowering. Either way the call allocates the selector's bound obligation
/// from the emission context (the same counter `allocate_requirement`
/// serves). An `AssignmentIndex` segment belongs to a store destination, not
/// a call argument, so it is refused here.
pub(crate) fn lower_argument_path(
    path: &[CheckedUnitStructuralPathSegment],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
    if !path
        .iter()
        .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
    {
        return lower_structural_path(path);
    }
    let mut calls = calls;
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Parameter { position },
            ) => {
                let (Some(values), Some(calls)) = (caller_scalar_values, calls.as_deref_mut())
                else {
                    return Err(LoweringError::Unsupported(
                        "runtime index argument path has no caller scalar frame",
                    ));
                };
                let index = values
                    .get(usize::try_from(*position).map_err(|_| {
                        LoweringError::Unsupported("runtime index position exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "runtime index selector position has no caller scalar",
                    ))?;
                if !matches!(
                    index.scalar_type,
                    ScalarType::Integer(integer) if !integer.is_address()
                ) {
                    return Err(LoweringError::Unsupported(
                        "runtime index selector is not an integer operand",
                    ));
                }
                calls.allocate_requirement().map(|obligation| {
                    terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: index.id,
                        obligation,
                    }
                })
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StructuralField(_),
            ) => {
                let Some(calls) = calls.as_deref_mut() else {
                    return Err(LoweringError::Unsupported(
                        "runtime index argument path has no caller scalar frame",
                    ));
                };
                let index = selector_values.next().ok_or(LoweringError::Unsupported(
                    "field-selector runtime index has no evaluated operand",
                ))?;
                calls.allocate_requirement().map(|obligation| {
                    terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation }
                })
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(_) => Err(LoweringError::Unsupported(
                "an assignment-index selector is owned by its store destination",
            )),
            segment => Ok(lower_structural_path(std::slice::from_ref(segment))?.remove(0)),
        })
        .collect()
}

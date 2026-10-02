use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::lowering_error::LoweringError;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::ValueDeclaration;
use terminal_psi::{StructuralAccess, StructuralPathSegment};

/// The Terminal access a checked structural access lowers to: the two
/// vocabularies correspond one to one. A lane that admits only some
/// accesses matches the checked access itself.
pub(crate) fn terminal_access(access: CheckedStructuralAccess) -> StructuralAccess {
    match access {
        CheckedStructuralAccess::Owned => StructuralAccess::Owned,
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        CheckedStructuralAccess::WriteOnlyBorrow => StructuralAccess::WriteOnlyBorrow,
    }
}

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
            CheckedUnitStructuralPathSegment::Case(_) => Err(LoweringError::Unsupported(
                "a case projection requires its exact source type during preparation",
            )),
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

/// A static projection is resolved against its original source declaration
/// during preparation. Only runtime selector coordinates wait for the emitting
/// call's scalar frame and bound-obligation namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreparedStructuralPathSegment {
    Static(StructuralPathSegment),
    RuntimeIndex(checked_trees::CheckedRuntimeIndex),
}

pub(crate) fn prepare_argument_path(
    path: &[CheckedUnitStructuralPathSegment],
    root: semantic_vocabulary::StructuralTypeId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    if !path
        .iter()
        .any(|step| matches!(step, CheckedUnitStructuralPathSegment::Case(_)))
    {
        return prepare_unscoped_path(path);
    }
    let declaration = |id| {
        let mut matches = types.iter().filter(|declaration| declaration.id == id);
        let found = matches.next().ok_or(LoweringError::Unsupported(
            "argument path lost its source type",
        ))?;
        if matches.next().is_some() {
            return Err(LoweringError::Unsupported(
                "argument path source type is ambiguous",
            ));
        }
        Ok(found)
    };
    let mut current = root;
    let mut selected_case = None;
    let mut prepared = Vec::with_capacity(path.len());
    for step in path {
        let shape = &declaration(current)?.shape;
        let lowered = match step {
            CheckedUnitStructuralPathSegment::Case(identity) => {
                if selected_case.is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case must select its payload field",
                    ));
                }
                let cases = match shape {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => {
                        return Err(LoweringError::Unsupported(
                            "argument case requires its exact source sum",
                        ));
                    }
                };
                let mut matches = cases.iter().filter(|case| case.identity == *identity);
                let case = matches.next().ok_or(LoweringError::Unsupported(
                    "argument case belongs to another source type",
                ))?;
                if matches.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case identity is ambiguous",
                    ));
                }
                selected_case = Some(case);
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Case(case.id))
            }
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let fields = match selected_case.take() {
                    Some(case) => &case.fields,
                    None => match shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => fields,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "argument field requires its source record or selected case",
                            ));
                        }
                    },
                };
                let mut matches = fields.iter().filter(|field| field.identity == *identity);
                let field = matches.next().ok_or(LoweringError::Unsupported(
                    "argument field belongs to another declaration",
                ))?;
                if matches.next().is_some() || field.relevance.is_erased() {
                    return Err(LoweringError::Unsupported(
                        "argument field is ambiguous or erased",
                    ));
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return Err(LoweringError::Unsupported(
                        "borrowed argument path requires a structural payload",
                    ));
                };
                current = child;
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Field(
                    identity.clone(),
                ))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                if selected_case.is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case must select its payload field",
                    ));
                }
                let StructuralTypeShape::FixedArray { element, length } = shape else {
                    return Err(LoweringError::Unsupported(
                        "argument index requires fixed array storage",
                    ));
                };
                if index >= length {
                    return Err(LoweringError::Unsupported(
                        "argument index exceeds its array",
                    ));
                }
                current = *element;
                PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                if selected_case.is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case must select its payload field",
                    ));
                }
                let StructuralTypeShape::FixedArray { element, .. } = shape else {
                    return Err(LoweringError::Unsupported(
                        "argument selector requires fixed array storage",
                    ));
                };
                current = *element;
                PreparedStructuralPathSegment::RuntimeIndex(index.clone())
            }
            CheckedUnitStructuralPathSegment::Referent => {
                if selected_case.is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case must select its payload field",
                    ));
                }
                let StructuralTypeShape::Reference { referent, .. } = shape else {
                    return Err(LoweringError::Unsupported(
                        "argument referent requires a reference carrier",
                    ));
                };
                current = *referent;
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Referent)
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { .. } => {
                return Err(LoweringError::Unsupported(
                    "case argument byte windows require their view carrier",
                ));
            }
        };
        prepared.push(lowered);
    }
    if selected_case.is_some() {
        return Err(LoweringError::Unsupported(
            "argument case has no payload field",
        ));
    }
    Ok(prepared)
}

pub(crate) fn prepare_unscoped_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                Ok(PreparedStructuralPathSegment::RuntimeIndex(index.clone()))
            }
            segment => Ok(PreparedStructuralPathSegment::Static(
                lower_structural_path(std::slice::from_ref(segment))?.remove(0),
            )),
        })
        .collect()
}

pub(crate) fn lower_typed_argument_path(
    path: &[CheckedUnitStructuralPathSegment],
    root: semantic_vocabulary::StructuralTypeId,
    types: &[terminal_psi::StructuralTypeDeclaration],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    emit_argument_path(
        &prepare_argument_path(path, root, types)?,
        caller_scalar_values,
        selector_values,
        calls,
    )
}

/// A checked argument path can carry a runtime selector only when the
/// operand lowering owns the index's evaluation and obligation: a dense
/// scalar parameter or value-binding local of the calling state is already
/// in the caller's operand frame, and a `StructuralField` selector consumes
/// a field snapshot already evaluated at its authored operand position.
/// Unit calls mint these reads in
/// their operation frame; scalar computations carry them through their ordered
/// operand blocks. The call allocates each selector's bound obligation
/// from the emission context (the same counter `allocate_requirement`
/// serves). An `AssignmentIndex` segment belongs to a store destination, not
/// a call argument, so it is refused here.
pub(crate) fn lower_argument_path(
    path: &[CheckedUnitStructuralPathSegment],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
    emit_argument_path(
        &prepare_unscoped_path(path)?,
        caller_scalar_values,
        selector_values,
        calls,
    )
}

pub(crate) fn emit_argument_path(
    path: &[PreparedStructuralPathSegment],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    let mut calls = calls;
    path.iter()
        .map(|segment| match segment {
            PreparedStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Parameter { position }
                | checked_trees::CheckedRuntimeIndex::Local { position },
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
            PreparedStructuralPathSegment::RuntimeIndex(
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
            PreparedStructuralPathSegment::RuntimeIndex(_) => Err(LoweringError::Unsupported(
                "an assignment-index selector is owned by its store destination",
            )),
            PreparedStructuralPathSegment::Static(segment) => Ok(segment.clone()),
        })
        .collect()
}

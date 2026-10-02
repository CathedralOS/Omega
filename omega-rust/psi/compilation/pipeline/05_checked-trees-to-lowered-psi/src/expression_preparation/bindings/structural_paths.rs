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
    use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeShape};
    // A structural field may end in an inline scalar or byte carrier rather
    // than another declared type. Retain that real distinction while walking;
    // no synthetic declaration is needed to prepare its final coordinate.
    enum Tip<'a> {
        Declaration(&'a terminal_psi::StructuralTypeDeclaration),
        Inline(&'a StructuralFieldType),
        Scalar,
        Window,
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
    let mut tip = Tip::Declaration(declaration(root)?);
    let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
    let mut prepared = Vec::with_capacity(path.len());
    for step in path {
        if selected_case.is_some() && !matches!(step, CheckedUnitStructuralPathSegment::Field(_)) {
            return Err(LoweringError::Unsupported(
                "argument case must select its payload field",
            ));
        }
        let shape = match tip {
            Tip::Declaration(declaration) => Some(&declaration.shape),
            _ => None,
        };
        let bytes = match tip {
            Tip::Declaration(declaration) => match declaration.shape {
                StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                _ => None,
            },
            Tip::Inline(field) => field.byte_sequence_carrier(),
            Tip::Scalar | Tip::Window => None,
        };
        let lowered = match step {
            CheckedUnitStructuralPathSegment::Case(identity) => {
                let cases = match shape {
                    Some(
                        StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. },
                    ) => cases,
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
                        Some(
                            StructuralTypeShape::Record { fields }
                            | StructuralTypeShape::Mixed { fields, .. },
                        ) => fields,
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
                tip = match field.field_type {
                    StructuralFieldType::Structural(child) => Tip::Declaration(declaration(child)?),
                    _ => Tip::Inline(&field.field_type),
                };
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Field(
                    identity.clone(),
                ))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                tip = match shape {
                    Some(StructuralTypeShape::FixedArray { element, length }) => {
                        if index >= length {
                            return Err(LoweringError::Unsupported(
                                "argument index exceeds its array",
                            ));
                        }
                        Tip::Declaration(declaration(*element)?)
                    }
                    Some(StructuralTypeShape::ElementView { element }) => {
                        Tip::Declaration(declaration(*element)?)
                    }
                    _ => match bytes {
                        Some(
                            ByteSequenceCarrier::BoundedOwned { capacity }
                            | ByteSequenceCarrier::BoundedView { capacity, .. },
                        ) if *index < capacity => Tip::Scalar,
                        Some(ByteSequenceCarrier::BorrowedView { .. }) => Tip::Scalar,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "argument index has no element storage",
                            ));
                        }
                    },
                };
                PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                tip = match shape {
                    Some(
                        StructuralTypeShape::FixedArray { element, .. }
                        | StructuralTypeShape::ElementView { element },
                    ) => Tip::Declaration(declaration(*element)?),
                    _ if bytes.is_some() => Tip::Scalar,
                    _ => {
                        return Err(LoweringError::Unsupported(
                            "argument selector has no element storage",
                        ));
                    }
                };
                PreparedStructuralPathSegment::RuntimeIndex(index.clone())
            }
            CheckedUnitStructuralPathSegment::Referent => {
                let Some(StructuralTypeShape::Reference { referent, .. }) = shape else {
                    return Err(LoweringError::Unsupported(
                        "argument referent requires a reference carrier",
                    ));
                };
                tip = Tip::Declaration(declaration(*referent)?);
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Referent)
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                let capacity = match shape {
                    Some(StructuralTypeShape::FixedArray { element, length })
                        if declaration(*element)?.shape
                            == StructuralTypeShape::PrimitiveScalar(
                                semantic_vocabulary::ScalarType::Integer(
                                    semantic_vocabulary::IntegerType::new(
                                        semantic_vocabulary::IntegerSign::Unsigned,
                                        8,
                                    )
                                    .expect("u8 is a supported integer carrier"),
                                ),
                            ) =>
                    {
                        Some(*length)
                    }
                    _ => match bytes {
                        Some(
                            ByteSequenceCarrier::BoundedOwned { capacity }
                            | ByteSequenceCarrier::BoundedView { capacity, .. },
                        ) => Some(capacity),
                        Some(ByteSequenceCarrier::BorrowedView { .. }) => None,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "argument window has no byte storage",
                            ));
                        }
                    },
                };
                if start > end || capacity.is_some_and(|capacity| *end > capacity) {
                    return Err(LoweringError::Unsupported(
                        "argument window exceeds its byte storage",
                    ));
                }
                tip = Tip::Window;
                PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                })
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

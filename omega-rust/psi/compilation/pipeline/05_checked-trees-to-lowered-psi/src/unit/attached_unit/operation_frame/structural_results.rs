//! Where one emitting body keeps its completed structural results: the
//! ordinary machine's dense per-binding roster with return-discard custody,
//! or a composed state's operation-buffer registration against the
//! machine-wide place catalog. `StructuralResults::earlier` gives both the
//! dense `(declaration, discard)` view call operands resolve against, and
//! `validate_completed_arrival` proves a completed binding rejoins the
//! exact block parameter roster that owns it.

use super::super::argument_evaluation::Evaluation;
use crate::emission::borrowed_window::BorrowedWindowRepairValue;
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
use crate::unit::{
    LoweringError, OperationKind, OperationResult, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind, unsupported,
};
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedUnitStructuralArgumentPlan};
use std::borrow::Cow;

/// Where the emitting body keeps its completed structural results.
pub(in crate::unit::attached_unit) enum StructuralResults<'f> {
    /// The ordinary machine's roster: one row per authored result binding in
    /// ordinal order, with whether the machine's return discards it. A row
    /// holds no declaration when its binding is a projected owned alias:
    /// that binding names a subtree of an already established place, so it
    /// declares no place of its own and the composed alias lives in the
    /// operation buffer's `structural_values` registry instead. Keeping the
    /// ordinal slot preserves the density every later ordinal is checked
    /// against, and makes every reader that needs a produced place refuse
    /// the alias explicitly rather than silently drop its path.
    Dense(&'f mut Vec<(Option<StructuralPlaceDeclaration>, bool)>),
    /// A composed state: each binding is registered in the operation buffer,
    /// and every result place joins the machine-wide place catalog.
    StateGraph {
        state: &'f CheckedComposedUnitControlStatePlan,
        places: &'f mut Vec<StructuralPlaceDeclaration>,
    },
}

fn validate_completed_arrival(
    declaration: &StructuralPlaceDeclaration,
    evaluation: &Evaluation,
) -> Result<(), LoweringError> {
    let StructuralPlaceKind::BlockParameter { block, .. } = declaration.kind else {
        return unsupported("completed arrival lost its block declaration");
    };
    // A completed binding can outlive later private blocks. Its declaration
    // rejoins the actual owning block's parameter roster, not just the current
    // roster. Receiving replay still proves use dominance and live ownership.
    let mut emitted = evaluation
        .blocks
        .iter()
        .filter(|candidate| candidate.id == block);
    let parameters = if block == evaluation.current {
        if emitted.next().is_some() {
            return unsupported("completed arrival repeats its current block");
        }
        evaluation.block_structural_parameters.as_slice()
    } else {
        let owner = emitted.next().ok_or(LoweringError::Unsupported(
            "completed arrival has no established block",
        ))?;
        if emitted.next().is_some() {
            return unsupported("completed arrival repeats its established block");
        }
        owner.structural_parameters.as_slice()
    };
    if !parameters
        .iter()
        .any(|parameter| parameter.place == declaration.id)
    {
        return unsupported("completed arrival has no declared parameter");
    }
    Ok(())
}

impl StructuralResults<'_> {
    /// The place a call continuation discards for `binding_ordinal`.
    pub(super) fn continuation_place(
        &self,
        binding_ordinal: u32,
        operations: &OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        match self {
            Self::Dense(roster) => {
                let (place, discard_on_return) =
                    roster
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "call continuation result has not been produced",
                        ))?;
                let place = place.as_ref().ok_or(LoweringError::Unsupported(
                    "call continuation result is a projected alias",
                ))?;
                if *discard_on_return {
                    return unsupported("call continuation cleanup has conflicting custody");
                }
                if !matches!(place.kind, StructuralPlaceKind::OperationResult { .. }) {
                    return unsupported("call continuation cleanup has no operation result");
                }
                Ok(place.id)
            }
            Self::StateGraph { state, .. } => {
                let binding = super::super::composed_control::state_graph_result(
                    state,
                    binding_ordinal,
                    operations,
                )?;
                Ok(binding.operation_result()?.place)
            }
        }
    }

    /// The completed whole value a borrowed-window repair stores back.
    pub(super) fn repair_value(
        &self,
        binding_ordinal: u32,
        operations: &OperationBuffer,
    ) -> Result<BorrowedWindowRepairValue, LoweringError> {
        match self {
            Self::Dense(roster) => {
                let (declaration, _) =
                    roster
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "borrowed-window repair binding is absent",
                        ))?;
                let declaration = declaration.as_ref().ok_or(LoweringError::Unsupported(
                    "borrowed-window repair binding is a projected alias",
                ))?;
                let structural_type = match declaration.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => structural_type,
                    StructuralPlaceKind::BlockParameter { .. } => {
                        // A construction or selection can finish at a join,
                        // not at its last operation. That completed binding
                        // already owns the join's type and custody; rejoin it
                        // to the exact declared place rather than requiring a
                        // second operation to manufacture the same value.
                        let mut bindings = operations
                            .structural_values
                            .iter()
                            .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                        let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                            "borrowed-window repair join has no completed binding",
                        ))?;
                        let result = binding.operation_result()?;
                        if bindings.next().is_some() || result.place != declaration.id {
                            return unsupported(
                                "borrowed-window repair join substituted its completed place",
                            );
                        }
                        result.structural_type
                    }
                    _ => {
                        return unsupported(
                            "borrowed-window repair binding is not a completed value",
                        );
                    }
                };
                Ok(BorrowedWindowRepairValue {
                    place: declaration.id,
                    structural_type,
                })
            }
            Self::StateGraph { state, .. } => {
                let binding = super::super::composed_control::state_graph_result(
                    state,
                    binding_ordinal,
                    operations,
                )?;
                let produced = binding.operation_result()?;
                Ok(BorrowedWindowRepairValue {
                    place: produced.place,
                    structural_type: produced.structural_type,
                })
            }
        }
    }

    /// A dense roster admits only the next ordinal; a composed state rejects
    /// a repeated binding when it registers the result.
    pub(super) fn require_next(
        &self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        refusal: &'static str,
    ) -> Result<(), LoweringError> {
        match self {
            Self::Dense(roster) if result.binding_ordinal as usize != roster.len() => {
                unsupported(refusal)
            }
            _ => Ok(()),
        }
    }

    /// The completed results a call's operands resolve by binding ordinal,
    /// as the dense `(declaration, discard)` roster transfer validation and
    /// argument lowering index.
    ///
    /// A composed state's binding ordinals belong to the source state, while
    /// its place catalog spans the whole emitted machine, so the view rejoins
    /// the operation buffer's registry (one row per ordinal, in order) to the
    /// one catalog declaration its producing operation established. Graph
    /// states own each result's cleanup per edge (`result_custody`), so no
    /// row is marked for a return discard here. A call naming no completed
    /// result reads no row, and the view stays empty rather than failing on
    /// a registry it never consults.
    pub(super) fn earlier(
        &self,
        arguments: &[CheckedUnitStructuralArgumentPlan],
        operations: &OperationBuffer,
        evaluation: &Evaluation,
    ) -> Result<Cow<'_, [(Option<StructuralPlaceDeclaration>, bool)]>, LoweringError> {
        let places = match self {
            Self::Dense(roster) => {
                for (ordinal, (declaration, _)) in roster.iter().enumerate() {
                    let Some(declaration) = declaration else {
                        continue;
                    };
                    if matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. })
                        && arguments.iter().any(|argument| {
                            argument
                                .source_structural_result_binding_ordinal()
                                .is_some_and(|binding| binding as usize == ordinal)
                        })
                    {
                        let mut bindings = operations
                            .structural_values
                            .iter()
                            .filter(|(binding, _)| *binding as usize == ordinal);
                        let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                            "completed arrival has no exact binding",
                        ))?;
                        if bindings.next().is_some() {
                            return unsupported("completed arrival repeats its binding");
                        }
                        binding.operation_result()?;
                        validate_completed_arrival(declaration, evaluation)?;
                    }
                }
                return Ok(Cow::Borrowed(roster.as_slice()));
            }
            Self::StateGraph { places, .. } => places,
        };
        if !arguments.iter().any(|argument| {
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        }) {
            return Ok(Cow::Owned(Vec::new()));
        }
        operations
            .structural_values
            .iter()
            .enumerate()
            .map(|(binding_position, (ordinal, result))| {
                if *ordinal as usize != binding_position {
                    return unsupported(
                        "composed call result binding namespace is stale or duplicated",
                    );
                }
                match result {
                    StructuralBinding::ByteSequenceLiteral { place, structural_type, declaration_ordinal } => {
                        let declaration = StructuralPlaceDeclaration { id: *place, kind: StructuralPlaceKind::ByteSequenceLiteral { structural_type: *structural_type, declaration_ordinal: *declaration_ordinal } };
                        let mut producers = operations.iter().filter(|operation| matches!(operation.kind,
                            OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == declaration.id));
                        let producer = producers.next().ok_or(LoweringError::Unsupported("literal binding has no establishment"))?;
                        if producers.next().is_some() || producer.result != OperationResult::Unit {
                            return unsupported("literal binding repeats or changes its establishment");
                        }
                        Ok((Some(declaration), false))
                    }
                    StructuralBinding::ProjectedAlias { .. } => unsupported(
                        "composed call result binding is a projected alias",
                    ),
                    StructuralBinding::OperationResult(result) => {
                        let mut declarations = places.iter().filter(|place| place.id == result.place);
                        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                            "composed call completed result has no place declaration",
                        ))?;
                        if declarations.next().is_some() {
                            return unsupported("composed call result repeats its declaration");
                        }
                        if let StructuralPlaceKind::BlockParameter { .. } = declaration.kind
                            && arguments.iter().any(|argument| {
                                argument.source_structural_result_binding_ordinal()
                                    == Some(*ordinal)
                            })
                        {
                            validate_completed_arrival(declaration, evaluation)?;
                        }
                        Ok((Some(*declaration), false))
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Cow::Owned)
    }

    /// Add one completed result's declaration: the dense row with its
    /// return-discard custody, or the machine-wide composed catalog.
    pub(super) fn push(
        &mut self,
        declaration: StructuralPlaceDeclaration,
        discard_on_return: bool,
    ) {
        match self {
            Self::Dense(roster) => roster.push((Some(declaration), discard_on_return)),
            Self::StateGraph { places, .. } => places.push(declaration),
        }
    }

    /// Reserve one ordinal slot for a projected owned alias. The binding
    /// declares no place and emits no operation, so there is nothing to
    /// catalog or discard; the slot keeps later ordinals dense.
    pub(super) fn push_projected_alias(&mut self) -> Result<(), LoweringError> {
        match self {
            Self::Dense(roster) => {
                roster.push((None, false));
                Ok(())
            }
            // A composed state resolves operands through the operation
            // buffer's registry alone, and its catalog has no slotless row.
            Self::StateGraph { .. } => {
                unsupported("composed state cannot bind a projected owned alias")
            }
        }
    }
}

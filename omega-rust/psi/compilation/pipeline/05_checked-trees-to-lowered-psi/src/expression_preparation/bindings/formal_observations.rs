//! Bind one source telescope to already completed invocation values.
//!
//! These construction-only aliases own no storage or transfer permission. A
//! projected actual keeps its original backing home and complete prefix, while
//! scalar formals name the exact input slots that captured their values. Shared
//! observation uses the ordinary home/type/path resolver; the source operands
//! that produced those values are never evaluated again.
use super::{
    AuthoredScalarBinding, LoweringError, ScalarBindings, ScalarType, StructuralArgument,
    StructuralTypeId, unsupported,
};
use symbols::SymbolHandle;

/// One structural formal, in dense structural telescope order. Source position
/// and exact declaration symbol remain independent of that dense position and
/// the actual's physical home. Retained receiver/claim aliases are full rows too.
#[derive(Clone)]
pub(crate) struct CapturedStructuralFormal {
    pub(crate) source_position: u32,
    pub(crate) symbol: SymbolHandle,
    pub(crate) argument: StructuralArgument,
    pub(crate) structural_type: StructuralTypeId,
}

impl ScalarBindings {
    /// Observe a target telescope over completed scalar prefix slots and
    /// structural actuals. The receiver owns the original backing declarations;
    /// no formal's selected subtree is relabeled as a physical whole-root type.
    pub(crate) fn capture_formal_observations(
        &self,
        scalar_formals: &[(usize, ScalarType)],
        structural_formals: &[CapturedStructuralFormal],
    ) -> Result<Self, LoweringError> {
        for (position, formal) in structural_formals.iter().enumerate() {
            if !formal.symbol.is_valid()
                || structural_formals[..position].iter().any(|earlier| {
                    earlier.symbol == formal.symbol
                        || earlier.source_position == formal.source_position
                })
                || self.structural_argument_type(&formal.argument) != Some(formal.structural_type)
            {
                return unsupported("captured formal observation lost its exact typed actual");
            }
        }
        let mut frame = self.clone();
        frame.authored = scalar_formals
            .iter()
            .map(|(value_position, scalar_type)| {
                Some(AuthoredScalarBinding {
                    value_position: *value_position,
                    scalar_type: *scalar_type,
                    storage_symbol: SymbolHandle::invalid(),
                })
            })
            .collect();
        frame.immutable = frame
            .authored
            .iter()
            .map(|binding| binding.map(|row| row.value_position))
            .collect();
        frame.parameter_count = scalar_formals.len();
        // A captured scalar denotes its incoming value, not the mutable formal
        // home an executable body may establish after entry observation.
        frame.primitive_storage.clear();
        frame.scalar_parameter_homes.clear();
        frame.captured_structural_formals = structural_formals.to_vec();
        Ok(frame)
    }

    /// Pure storage observations name authored positions, whereas mixed
    /// structural arguments name dense positions. Keep both coordinates joined
    /// to one exact captured formal instead of indexing the caller's telescope.
    pub(crate) fn captured_storage_home(
        &self,
        root: checked_trees::CheckedStorageRoot,
    ) -> Result<Option<(StructuralArgument, StructuralTypeId)>, LoweringError> {
        let mut observations = self
            .captured_storage_observations
            .iter()
            .filter(|(candidate, _, _)| *candidate == root);
        if let Some((_, argument, structural_type)) = observations.next() {
            if observations.next().is_some() {
                return unsupported("storage observation has conflicting captured homes");
            }
            return Ok(Some((argument.clone(), *structural_type)));
        }
        use checked_trees::CheckedStorageRoot as Root;
        if self.captured_structural_formals.is_empty() {
            return Ok(None);
        }
        let formal = match root {
            Root::Parameter { index } => Some(
                self.captured_structural_formals
                    .iter()
                    .find(|formal| formal.source_position == index)
                    .ok_or(LoweringError::Unsupported(
                        "captured storage observation lost its authored formal coordinate",
                    ))?,
            ),
            Root::OwnedLocal { symbol } | Root::ViewLocal { symbol } => self
                .captured_structural_formals
                .iter()
                .find(|formal| formal.symbol == symbol),
        };
        Ok(formal.map(|formal| (formal.argument.clone(), formal.structural_type)))
    }

    pub(super) fn captured_formal_home(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<StructuralArgument>, LoweringError> {
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
        let parameter_index = match argument.source {
            Source::Parameter { parameter_index }
            | Source::ByteSequenceSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            }
            | Source::ElementViewSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            } => Some(parameter_index),
            _ => None,
        };
        if let Some(index) = parameter_index {
            if self.captured_structural_formals.is_empty() {
                return Ok(None);
            }
            return self
                .captured_structural_formals
                .get(index as usize)
                .map(|formal| Some(formal.argument.clone()))
                .ok_or(LoweringError::Unsupported(
                    "captured formal observation lost its dense structural coordinate",
                ));
        }
        let symbol = argument
            .structural_storage_root()
            .and_then(|root| root.as_symbol());
        Ok(symbol.and_then(|symbol| {
            self.captured_structural_formals
                .iter()
                .find(|formal| formal.symbol == symbol)
                .map(|formal| formal.argument.clone())
        }))
    }
}

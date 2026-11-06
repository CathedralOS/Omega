//! A declared home supplies storage, while its current claim frontier proves
//! the owned value installed there. Observation can use an enclosing nominal
//! owner; moving a subtree must transfer that subtree's exact identities.
use super::{CurrentOwnership, LiveClaim};
use crate::structural_types::StructuralTypes;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::{OperationSite, OptimizationUnitValidationError};
use abstract_operations::AbstractOperation as O;
use abstract_operations::StructuralWindowRoot;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

pub(super) struct ValueUse<'a> {
    pub function: &'a PsiOptimizationFunction,
    pub types: &'a [StructuralTypeDeclaration],
    pub block: BlockId,
    pub node: u32,
    pub admitted_case: &'a dyn Fn(PlaceId, &[StructuralPathSegment], StructuralCaseId) -> bool,
}

pub(super) fn uninstalled_home(
    function: &PsiOptimizationFunction,
    current: &CurrentOwnership,
    place: PlaceId,
) -> bool {
    current.owned_places.get(&place) == Some(&StructuralMultiplicity::Linear)
        && current
            .partial_custody_paths
            .get(&place)
            .is_some_and(|missing| missing.len() == 1 && missing.first().is_some_and(Vec::is_empty))
        && function.block_nodes().any(|producer| {
            matches!(&producer.operation,
            O::EstablishStructuralLocal { result, .. } if result.place == place)
        })
}

impl ValueUse<'_> {
    fn refusal(&self, place: PlaceId, reason: &'static str) -> OptimizationUnitValidationError {
        self.refusal_at(place, &[], reason)
    }

    fn refusal_at(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        reason: &'static str,
    ) -> OptimizationUnitValidationError {
        OptimizationUnitValidationError::CurrentOwnedValueUnavailable {
            site: OperationSite::new(self.function.machine, self.block, self.node),
            place,
            path_length: path.len(),
            path_prefix: path.iter().take(4).cloned().collect(),
            reason,
        }
    }

    fn frontier(
        &self,
        current: &CurrentOwnership,
        place: PlaceId,
        selected: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<(), OptimizationUnitValidationError> {
        let claims = current.claims.values().filter(|claim| {
            claim.input == Some(place)
                && (claim.path.starts_with(path) || path.starts_with(&claim.path))
        });
        if claims
            .clone()
            .any(|claim| claim.multiplicity != Some(StructuralMultiplicity::Linear))
        {
            return Err(self.refusal(place, "owned value claim has no exact linear multiplicity"));
        }
        terminal_semantics::owned_structure::validate_owned_claim_frontier(
            &|identity| self.types.declaration(identity),
            selected,
            path,
            claims.map(|claim| claim.path.clone()),
            &|path, case| (self.admitted_case)(place, path, case),
        )
        .map_err(|reason| self.refusal_at(place, path, reason))
    }

    pub(super) fn moving(
        &self,
        current: &CurrentOwnership,
        argument: &StructuralArgument,
    ) -> Result<(), OptimizationUnitValidationError> {
        let root = structural_source_contract(self.function, argument.place).ok_or_else(|| {
            self.refusal(argument.place, "owned operand has no exact source contract")
        })?;
        if root.access != StructuralAccess::Owned
            || !terminal_semantics::projection_stays_in_inline_storage(
                self.types.iter(),
                root.structural_type,
                &argument.path,
            )
        {
            // A borrowed-window extraction retains its parent's namespace.
            // Its existing loan/transfer owner authenticates the operation;
            // this frame cannot demand or invent the caller's claim names.
            return Ok(());
        }
        if current
            .partial_custody_paths
            .get(&argument.place)
            .is_some_and(|missing| {
                missing.iter().any(|path| {
                    terminal_semantics::structural_paths_may_overlap(path, &argument.path)
                })
            })
        {
            return Err(self.refusal_at(
                argument.place,
                &argument.path,
                "owned operand is unestablished or partially unavailable",
            ));
        }
        let selected = terminal_semantics::runtime_structural_path_tip(
            self.types.iter(),
            root.structural_type,
            &argument.path,
        )
        .ok_or_else(|| {
            self.refusal(argument.place, "owned operand path has no selected carrier")
        })?;
        self.frontier(current, argument.place, selected.id, &argument.path)
    }

    /// All ordinary borrows, including &write, name an existing valid value.
    /// The internal store destination is storage authority and never calls this.
    pub(super) fn observing(
        &self,
        current: &CurrentOwnership,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), OptimizationUnitValidationError> {
        let root = structural_source_contract(self.function, place)
            .ok_or_else(|| self.refusal(place, "observed value has no exact source contract"))?;
        if root.access != StructuralAccess::Owned {
            // The existing live-reference owner authenticates borrowed ingress;
            // it does not import the caller's owned claims into this frame.
            return Ok(());
        }
        if (root.multiplicity != StructuralMultiplicity::Unrestricted
            && current.owned_places.get(&place) != Some(&root.multiplicity)
            && !super::structural::projected_root_is_fully_consumed(
                self.function,
                self.types,
                current,
                place,
            ))
            || current
                .partial_custody_paths
                .get(&place)
                .is_some_and(|absent| {
                    absent.iter().any(|absent| {
                        terminal_semantics::structural_paths_may_overlap(absent, path)
                    })
                })
        {
            return Err(self.refusal_at(
                place,
                path,
                "observed owned value is moved or partially unavailable",
            ));
        }
        for length in 0..=path.len() {
            let prefix = &path[..length];
            if matches!(prefix.last(), Some(StructuralPathSegment::Case(_))) {
                // A payload namespace is not a value or pointer crossing.
                // The following field resolves its exact structural child.
                continue;
            }
            if !terminal_semantics::projection_stays_in_inline_storage(
                self.types.iter(),
                root.structural_type,
                prefix,
            ) {
                // A descriptor crossing is governed by the captured loan,
                // not by claims on a merely equal pointer or pointee type.
                return Ok(());
            }
            let Some(selected) = terminal_semantics::runtime_structural_path_tip(
                self.types.iter(),
                root.structural_type,
                prefix,
            ) else {
                // A primitive endpoint has no nominal obligation of its own;
                // any enclosing owner was checked at an earlier prefix.
                return Ok(());
            };
            if selected.owns_linear_claim || length == path.len() {
                return self.frontier(current, place, selected.id, prefix);
            }
        }
        Ok(())
    }

    pub(super) fn observations(
        &self,
        current: &CurrentOwnership,
        operation: &O,
    ) -> Result<(), OptimizationUnitValidationError> {
        match operation {
            O::EstablishReference { source, .. }
            | O::EstablishAccessReference { source, .. }
            | O::EstablishElementView { source, .. }
            | O::ElementViewLength { source, .. }
            | O::ElementViewSubslice { source, .. }
            | O::ByteSequenceLength { source, .. }
            | O::ByteSequenceSubslice { source, .. }
            | O::GlobalDescriptorTable { source, .. } => {
                self.observing(current, source.place, &source.path)
            }
            abstract_operations::primitive_storage_observation_pattern!(source, path, ..) => {
                let region = super::storage_regions::projected(
                    self.function,
                    self.types,
                    *source,
                    path,
                    None,
                );
                self.observing(current, *source, region.direct_path())
            }
            O::StructuralScalarFieldRead {
                source,
                path,
                field,
                ..
            } => {
                let region = super::storage_regions::canonical_field(
                    self.function,
                    self.types,
                    *source,
                    path,
                    *field,
                );
                self.observing(current, *source, &region)
            }
            O::StructuralCase { source, path, .. } => self.observing(current, *source, path),
            O::StructuralLeafCopy {
                source,
                path: abstract_operations::StructuralCopyPath::Ordinary(path),
                ..
            } => self.observing(current, *source, path),
            O::MoveStructuralField {
                source,
                path,
                field,
                ..
            } => {
                let selected = super::references::window_hole(
                    self.function,
                    self.types,
                    source.place(),
                    source.structural_type(),
                    path,
                    *field,
                    StructuralAccess::MutableBorrow,
                )
                .ok_or_else(|| {
                    self.refusal(source.place(), "move has no selected storage window")
                })?;
                self.observing(current, source.place(), &selected)
            }
            abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                let region = super::storage_regions::projected(
                    self.function,
                    self.types,
                    *source,
                    path,
                    Some(*field),
                );
                self.observing(current, *source, region.direct_path())
            }
            O::ElementViewRead { source, .. } | O::ByteSequenceRead { source, .. } => {
                self.observing(current, *source, &[])
            }
            O::EstablishByteSequenceConcatenation { left, right, .. }
            | O::ByteSequenceEqual { left, right, .. } => {
                self.byte_operand(current, left)?;
                self.byte_operand(current, right)
            }
            O::ByteSequencePredicate { operand, .. } => self.byte_operand(current, operand),
            O::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                self.observing(current, gdt.place, &gdt.path)?;
                self.observing(current, scratch.place, &scratch.path)
            }
            O::ActivateTaskRegister {
                gdt,
                task_state,
                scratch,
                ..
            } => {
                self.observing(current, gdt.place, &gdt.path)?;
                self.observing(current, task_state.place, &task_state.path)?;
                self.observing(current, scratch.place, &scratch.path)
            }
            _ => Ok(()),
        }
    }

    fn byte_operand(
        &self,
        current: &CurrentOwnership,
        operand: &terminal_psi::control_flow::ByteSequenceOperand,
    ) -> Result<(), OptimizationUnitValidationError> {
        use terminal_psi::control_flow::ByteSequenceOperand;
        let (source, field) = match operand {
            ByteSequenceOperand::Whole { source, .. } => (source, None),
            ByteSequenceOperand::Field { source, field, .. } => (source, Some(*field)),
        };
        let region = super::storage_regions::projected(
            self.function,
            self.types,
            source.place,
            &source.path,
            field,
        );
        self.observing(current, source.place, region.direct_path())
    }

    /// A performed store relocates existing identities into inline owned
    /// storage. Across a borrowed window the parent keeps its own namespace;
    /// the detached value's frame-local claims end at its checked repair.
    pub(super) fn install_store(
        &self,
        current: &mut CurrentOwnership,
        destination: &StructuralWindowRoot,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
        value: &StructuralArgument,
        access: StructuralAccess,
    ) -> Result<(), OptimizationUnitValidationError> {
        let hole = super::references::window_hole(
            self.function,
            self.types,
            destination.place(),
            destination.structural_type(),
            path,
            field,
            access,
        )
        .ok_or_else(|| self.refusal(destination.place(), "store has no exact receiving window"))?;
        let owned = destination.parameter().is_none()
            && terminal_semantics::projection_stays_in_inline_storage(
                self.types.iter(),
                destination.structural_type(),
                &hole,
            );
        let moved = current
            .claims
            .iter()
            .filter(|(_, claim)| claim.input == Some(value.place))
            .map(|(identity, claim)| (*identity, claim.clone()))
            .collect::<Vec<_>>();
        if owned
            && current.claims.values().any(|claim| {
                claim.input == Some(destination.place())
                    && terminal_semantics::structural_paths_may_overlap(&claim.path, &hole)
            })
        {
            return Err(self.refusal(
                destination.place(),
                "store would overwrite a live owned claim",
            ));
        }
        for (identity, claim) in moved {
            current.claims.remove(&identity);
            if owned {
                let mut restored = hole.clone();
                restored.extend_from_slice(&claim.path);
                let multiplicity = if restored.is_empty() {
                    destination.multiplicity()
                } else {
                    StructuralMultiplicity::Linear
                };
                current.claims.insert(
                    identity,
                    LiveClaim {
                        input: Some(destination.place()),
                        path: restored,
                        multiplicity: Some(multiplicity),
                    },
                );
            }
        }
        if owned && hole.is_empty() {
            // Only the whole establishing store makes this home a value.
            // Installing a claimed child says nothing about unfilled siblings,
            // including fields that carry no claim of their own.
            if let Some(missing) = current.partial_custody_paths.get_mut(&destination.place()) {
                missing.remove(&Vec::new());
                if missing.is_empty() {
                    current.partial_custody_paths.remove(&destination.place());
                }
            }
        }
        Ok(())
    }
}

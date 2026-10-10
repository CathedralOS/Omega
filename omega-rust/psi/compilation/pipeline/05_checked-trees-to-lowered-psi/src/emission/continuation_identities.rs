//! Rename one privately emitted continuation together with its evidence.
//!
//! Control specialization happens before source retention publishes reach and
//! suspension certificates. A copied operation is a new static occurrence:
//! its definitions, obligations, claim establishments and source companions
//! must move together. Types, callee signatures and authored source coordinates
//! are shared declarations and are never renamed by this namespace.
//!
//! The control worklist supplies the correspondence, including inherited
//! dominating definitions. Absence means that the original definition still
//! dominates this continuation, not permission to manufacture another value.

use semantic_vocabulary::{BlockId, ClaimId, EdgeId, ObligationId, OperationId, PlaceId, ValueId};
use terminal_psi::{Block, Operation, OperationResult, StructuralPathSegment};

use crate::lowering_error::{LoweringError, unsupported};

mod companions;
mod operations;
mod proofs;
mod termination;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IdentityMap {
    pub(crate) observed: Option<IdentityUses>,
    pub(crate) values: Vec<(ValueId, ValueId)>,
    pub(crate) places: Vec<(PlaceId, PlaceId)>,
    pub(crate) blocks: Vec<(BlockId, BlockId)>,
    pub(crate) edges: Vec<(EdgeId, EdgeId)>,
    pub(crate) operations: Vec<(OperationId, OperationId)>,
    pub(crate) inherited_operations: Vec<(OperationId, OperationId)>,
    pub(crate) obligations: Vec<(ObligationId, ObligationId)>,
    pub(crate) claims: Vec<(ClaimId, ClaimId)>,
    /// Two edges originally targeting the same block can select different
    /// ownership continuations. Target choice belongs to the original edge.
    pub(crate) edge_targets: Vec<(EdgeId, BlockId)>,
}

/// Dependency collection uses the very same traversal as substitution, so a
/// proof-only reference cannot disappear from continuation liveness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IdentityUses {
    pub(crate) values: Vec<ValueId>,
    pub(crate) places: Vec<PlaceId>,
    pub(crate) operations: Vec<OperationId>,
    pub(crate) claims: Vec<ClaimId>,
    pub(crate) obligations: Vec<ObligationId>,
}

fn remember<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn mapped<T: Copy + PartialEq>(rows: &[(T, T)], original: T) -> T {
    rows.iter()
        .rev()
        .find_map(|(source, target)| (*source == original).then_some(*target))
        .unwrap_or(original)
}

impl IdentityMap {
    pub(crate) fn value(&mut self, original: ValueId) -> ValueId {
        if let Some(observed) = &mut self.observed {
            remember(&mut observed.values, original);
        }
        mapped(&self.values, original)
    }
    pub(crate) fn place(&mut self, original: PlaceId) -> PlaceId {
        if let Some(observed) = &mut self.observed {
            remember(&mut observed.places, original);
        }
        mapped(&self.places, original)
    }
    pub(crate) fn block_id(&mut self, original: BlockId) -> BlockId {
        mapped(&self.blocks, original)
    }
    pub(crate) fn edge(&mut self, original: EdgeId) -> EdgeId {
        mapped(&self.edges, original)
    }
    pub(crate) fn operation(&mut self, original: OperationId) -> OperationId {
        if let Some(observed) = &mut self.observed {
            remember(&mut observed.operations, original);
        }
        self.operations
            .iter()
            .rev()
            .find_map(|(source, target)| (*source == original).then_some(*target))
            .unwrap_or_else(|| mapped(&self.inherited_operations, original))
    }
    pub(crate) fn obligation(&mut self, original: ObligationId) -> ObligationId {
        if let Some(observed) = &mut self.observed {
            remember(&mut observed.obligations, original);
        }
        mapped(&self.obligations, original)
    }
    pub(crate) fn claim(&mut self, original: ClaimId) -> ClaimId {
        if let Some(observed) = &mut self.observed {
            remember(&mut observed.claims, original);
        }
        mapped(&self.claims, original)
    }

    pub(crate) fn block(&mut self, block: &mut Block) -> Result<(), LoweringError> {
        block.id = self.block_id(block.id);
        for value in block
            .parameters
            .iter_mut()
            .chain(&mut block.erased_scalar_formals)
        {
            value.id = self.value(value.id);
        }
        for parameter in &mut block.structural_parameters {
            parameter.place = self.place(parameter.place);
            for qualification in &mut parameter.projected_qualifications {
                self.path(&mut qualification.path);
            }
        }
        for operation in &mut block.operations {
            self.operation_definition(operation)?;
        }
        self.terminator(&mut block.terminator)
    }

    pub(crate) fn operation_definition(
        &mut self,
        operation: &mut Operation,
    ) -> Result<(), LoweringError> {
        // These certificates are attached by source retention after the whole
        // private control graph is complete. Cloning a published certificate
        // would detach its identity from the occurrence it authorizes.
        if operation.static_reach_binding.is_some() || operation.suspension_crossing.is_some() {
            return unsupported(
                "continuation specialization must precede source certificate retention",
            );
        }
        operation.id = self.operation(operation.id);
        match &mut operation.result {
            OperationResult::Unit => {}
            OperationResult::Scalar(value) => value.id = self.value(value.id),
            OperationResult::Structural(result) => {
                result.place = self.place(result.place);
                for row in &mut result.projected_qualifications {
                    self.path(&mut row.path);
                }
                for row in &mut result.claims {
                    row.claim = self.claim(row.claim);
                    self.path(&mut row.path);
                }
                result.claims.sort_by_key(|row| row.claim);
            }
        }
        self.operation_kind(&mut operation.kind)
    }

    pub(crate) fn path(&mut self, path: &mut [StructuralPathSegment]) {
        for segment in path {
            if let StructuralPathSegment::RuntimeIndex { index, obligation } = segment {
                *index = self.value(*index);
                *obligation = self.obligation(*obligation);
            }
        }
    }

    pub(crate) fn argument(&mut self, argument: &mut terminal_psi::StructuralArgument) {
        argument.place = self.place(argument.place);
        self.path(&mut argument.path);
    }

    pub(crate) fn place_declaration(
        &mut self,
        declaration: &mut terminal_psi::StructuralPlaceDeclaration,
    ) {
        declaration.id = self.place(declaration.id);
        match &mut declaration.kind {
            semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                *producer = self.operation(*producer)
            }
            semantic_vocabulary::StructuralPlaceKind::BlockParameter { block, .. } => {
                *block = self.block_id(*block)
            }
            semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
            | semantic_vocabulary::StructuralPlaceKind::Result
            | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
            | semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { .. }
            | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal { .. } => {}
        }
    }
}

//! Namespace definitions and live captured inputs of a continuation instance.
//!
//! Dependency collection uses the identity remapper itself, including proof and
//! source companions. This keeps the worklist key sensitive to historical loan
//! captures even after their runtime carrier has moved. Block parameters and
//! actual claim introductions bind new identities; forwarding does not.

use super::{CompanionRows, IdentityMap, IdentityUses, LoweringError, Specialization};
use lowered_psi::LoweredSourceCallSite;
use semantic_vocabulary::{ClaimId, EdgeId, ObligationId, OperationId, PlaceId, ValueId};
use terminal_psi::{Block, OperationKind, OperationResult, TerminalMachine};

pub(super) struct Definitions {
    pub(super) values: Vec<ValueId>,
    pub(super) places: Vec<PlaceId>,
    pub(super) operations: Vec<OperationId>,
    pub(super) edges: Vec<EdgeId>,
    pub(super) obligations: Vec<ObligationId>,
    pub(super) claims: Vec<ClaimId>,
}

impl Definitions {
    pub(super) fn for_block(
        machine: &TerminalMachine,
        block: &Block,
        rows: &CompanionRows<'_>,
    ) -> Result<Self, LoweringError> {
        let mut definitions = Self {
            values: block
                .parameters
                .iter()
                .chain(&block.erased_scalar_formals)
                .map(|parameter| parameter.id)
                .collect(),
            places: block
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect(),
            operations: block
                .operations
                .iter()
                .map(|operation| operation.id)
                .collect(),
            edges: block.terminator.edges().collect(),
            obligations: Vec::new(),
            claims: machine
                .block_entry_claims
                .iter()
                .filter(|claim| {
                    block
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == claim.input)
                })
                .map(|claim| claim.claim)
                .collect(),
        };
        for operation in &block.operations {
            match &operation.result {
                OperationResult::Unit => {}
                OperationResult::Scalar(value) => definitions.values.push(value.id),
                OperationResult::Structural(result) => definitions.places.push(result.place),
            }
            match &operation.kind {
                OperationKind::EstablishByteSequenceLiteral { destination, .. }
                | OperationKind::EstablishElementView { destination, .. }
                | OperationKind::EstablishMutableElementView { destination, .. }
                | OperationKind::EstablishTrivialAffineLocal { destination } => {
                    definitions.places.push(*destination);
                }
                _ => {}
            }
        }
        definitions.claims.extend(
            rows.claim_introductions
                .iter()
                .filter(|introduction| definitions.operations.contains(&introduction.operation))
                .map(|introduction| introduction.claim),
        );
        let mut namespace = observer();
        namespace.block(&mut block.clone())?;
        definitions.obligations = namespace
            .observed
            .take()
            .expect("dependency observer")
            .obligations;
        // Arrival obligations belong to the predecessor's edge, not the
        // header whose predicate consumes their independently checked result.
        definitions.obligations.extend(
            rows.scalar_block_invariants
                .iter()
                .flat_map(|invariant| &invariant.arrivals)
                .filter(|arrival| definitions.edges.contains(&arrival.edge))
                .map(|arrival| arrival.obligation),
        );
        normalize(&mut definitions.values);
        normalize(&mut definitions.places);
        normalize(&mut definitions.operations);
        normalize(&mut definitions.edges);
        normalize(&mut definitions.obligations);
        normalize(&mut definitions.claims);
        Ok(definitions)
    }

    fn remove_from(&self, uses: &mut IdentityUses) {
        uses.values
            .retain(|identity| !self.values.contains(identity));
        uses.places
            .retain(|identity| !self.places.contains(identity));
        uses.operations
            .retain(|identity| !self.operations.contains(identity));
        uses.claims
            .retain(|identity| !self.claims.contains(identity));
        uses.obligations
            .retain(|identity| !self.obligations.contains(identity));
    }
}

pub(super) fn live_inputs(
    machine: &TerminalMachine,
    topology: &Specialization,
    definitions: &[Definitions],
    rows: &CompanionRows<'_>,
) -> Result<Vec<IdentityUses>, LoweringError> {
    let local = machine
        .blocks
        .iter()
        .zip(definitions)
        .map(|(block, definitions)| {
            let mut namespace = observer();
            namespace.block(&mut block.clone())?;
            for call in rows
                .source_calls
                .iter()
                .filter(|call| match call.terminal_site {
                    LoweredSourceCallSite::Operation(operation) => {
                        definitions.operations.contains(&operation)
                    }
                    LoweredSourceCallSite::InvokeNoNormal(edge)
                    | LoweredSourceCallSite::ExternalCompletion(edge) => {
                        definitions.edges.contains(&edge)
                    }
                })
            {
                namespace.source_call(&mut call.clone());
            }
            for completion in machine
                .loan_completions
                .iter()
                .filter(|completion| definitions.operations.contains(&completion.operation))
            {
                namespace.loan_completion(&mut completion.clone());
            }
            for invariant in rows
                .scalar_block_invariants
                .iter()
                .filter(|invariant| invariant.header == block.id)
            {
                let mut invariant = invariant.clone();
                invariant.arrivals.clear();
                namespace.block_invariant(&mut invariant)?;
            }
            for coercion in rows
                .qualification_coercions
                .iter()
                .filter(|coercion| definitions.edges.contains(&coercion.edge))
            {
                // Destination belongs to the successor telescope. Its own block
                // binds that value; retaining it here would invent a prior input.
                namespace.value(coercion.source);
            }
            for (_, value) in rows
                .continuation_value_uses
                .iter()
                .filter(|(owner, _)| *owner == block.id)
            {
                namespace.value(*value);
            }
            for claim in machine.block_entry_claims.iter().filter(|claim| {
                block
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == claim.input)
            }) {
                namespace.path(&mut claim.path.clone());
            }
            let mut uses = namespace.observed.take().expect("dependency observer");
            definitions.remove_from(&mut uses);
            normalize_uses(&mut uses);
            Ok(uses)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut live = topology
        .continuations
        .iter()
        .map(|continuation| local[continuation.original_block].clone())
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for position in (0..topology.continuations.len()).rev() {
            let continuation = &topology.continuations[position];
            let mut incoming = local[continuation.original_block].clone();
            for (edge, successor) in &continuation.successors {
                incoming.values.extend_from_slice(&live[*successor].values);
                incoming.places.extend_from_slice(&live[*successor].places);
                incoming
                    .operations
                    .extend_from_slice(&live[*successor].operations);
                for claim in &live[*successor].claims {
                    let mut bindings = rows
                        .claim_edge_bindings
                        .iter()
                        .filter(|binding| binding.edge == *edge && binding.destination == *claim);
                    let source = bindings.next().map_or(*claim, |binding| binding.source);
                    if bindings.next().is_some() {
                        return Err(LoweringError::Unsupported(
                            "continuation edge repeats a claim binding",
                        ));
                    }
                    incoming.claims.push(source);
                }
                incoming
                    .obligations
                    .extend_from_slice(&live[*successor].obligations);
            }
            definitions[continuation.original_block].remove_from(&mut incoming);
            normalize_uses(&mut incoming);
            if live[position] != incoming {
                live[position] = incoming;
                changed = true;
            }
        }
        if !changed {
            return Ok(live);
        }
    }
}

fn observer() -> IdentityMap {
    IdentityMap {
        observed: Some(IdentityUses::default()),
        ..IdentityMap::default()
    }
}

fn normalize<T: Ord>(identities: &mut Vec<T>) {
    identities.sort_unstable();
    identities.dedup();
}

fn normalize_uses(uses: &mut IdentityUses) {
    normalize(&mut uses.values);
    normalize(&mut uses.places);
    normalize(&mut uses.operations);
    normalize(&mut uses.claims);
    normalize(&mut uses.obligations);
}

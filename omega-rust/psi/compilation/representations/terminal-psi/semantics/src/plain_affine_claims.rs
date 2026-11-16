//! Claim names follow the existing availability walk. They do not authorize
//! backing, content issuance, a boundary implementation or opaque run cells.
//! The producer emits rows; receiving reconstructs transport from operations
//! and compares those rows, including empty and inactive-case catalogues.

use super::Error;
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, InvocationSite, OperationId, PlaceId, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, EntryClaim, MachineContract, Operation, OperationKind as Op,
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralPathSegment as Path,
    StructuralResultClaimBinding as Binding, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, TerminalMachine, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimLocation {
    pub identity: ClaimId,
    pub place: PlaceId,
    pub path: Vec<Path>,
}

// Slots come from the declared claim and place rosters, never from the largest
// supplied identity. Each place lists only its own live claim slots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ClaimsState {
    claim_ids: Arc<[ClaimId]>,
    place_ids: Arc<[PlaceId]>,
    claims: Vec<Option<ClaimLocation>>,
    at_place: Vec<Vec<usize>>,
    live_count: usize,
}

impl ClaimsState {
    fn new(machine: &TerminalMachine, mut claim_ids: Vec<ClaimId>) -> Result<Self, Error> {
        claim_ids.sort_unstable();
        claim_ids.dedup();
        let mut place_ids = machine
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect::<Vec<_>>();
        place_ids.sort_unstable();
        if place_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::malformed("claim storage repeats a declared place"));
        }
        Ok(Self {
            claims: vec![None; claim_ids.len()],
            at_place: vec![Vec::new(); place_ids.len()],
            claim_ids: claim_ids.into(),
            place_ids: place_ids.into(),
            live_count: 0,
        })
    }
    fn live(&self) -> impl Iterator<Item = &ClaimLocation> {
        self.claims.iter().flatten()
    }
    fn at(&self, place: PlaceId) -> impl Iterator<Item = &ClaimLocation> {
        self.place_ids
            .binary_search(&place)
            .ok()
            .into_iter()
            .flat_map(|position| &self.at_place[position])
            .map(|slot| {
                self.claims[*slot]
                    .as_ref()
                    .expect("place slots name live claims")
            })
    }
    pub(super) fn meet(&self, other: &Self) -> Result<Self, Error> {
        if self != other {
            return Err(Error::unsupported(
                "arrivals disagree on their exact logical claim frontier",
            ));
        }
        Ok(self.clone())
    }
    fn take_at(
        &mut self,
        place: PlaceId,
        selected: impl Fn(&ClaimLocation) -> bool,
    ) -> Vec<ClaimLocation> {
        let Ok(position) = self.place_ids.binary_search(&place) else {
            return Vec::new();
        };
        let mut moved = Vec::new();
        let claims = &mut self.claims;
        self.at_place[position].retain(|slot| {
            if selected(
                claims[*slot]
                    .as_ref()
                    .expect("place slots name live claims"),
            ) {
                moved.push(claims[*slot].take().expect("selected claim is live"));
                false
            } else {
                true
            }
        });
        self.live_count -= moved.len();
        moved
    }
    fn take(&mut self, argument: &StructuralArgument) -> Result<Vec<ClaimLocation>, Error> {
        if argument.access != StructuralAccess::Owned {
            return Ok(Vec::new());
        }
        if argument
            .path
            .iter()
            .any(|part| !matches!(part, Path::Field(_) | Path::Case(_) | Path::FixedIndex(_)))
        {
            return Err(Error::unsupported(
                "owned claim transport requires an exact inline path",
            ));
        }
        if self
            .at(argument.place)
            .any(|claim| argument.path.starts_with(&claim.path) && argument.path != claim.path)
        {
            return Err(Error::unsupported(
                "a projected move would split an intrinsic logical claim",
            ));
        }
        let mut moved = self.take_at(argument.place, |claim| {
            claim.path.starts_with(&argument.path)
        });
        for claim in &mut moved {
            claim.path.drain(..argument.path.len());
        }
        Ok(moved)
    }
    fn put(&mut self, claim: ClaimLocation) -> Result<(), Error> {
        let slot = self
            .claim_ids
            .binary_search(&claim.identity)
            .map_err(|_| Error::malformed("claim transport names an undeclared identity"))?;
        if self.claims[slot].is_some() {
            return Err(Error::malformed(
                "claim transport duplicates a live identity",
            ));
        }
        let place = self
            .place_ids
            .binary_search(&claim.place)
            .map_err(|_| Error::malformed("claim transport names an undeclared place"))?;
        if self
            .at(claim.place)
            .any(|held| held.path.starts_with(&claim.path) || claim.path.starts_with(&held.path))
        {
            return Err(Error::malformed("claim transport overlaps an owned path"));
        }
        let position = self.at_place[place]
            .binary_search(&slot)
            .expect_err("an absent claim has no live place slot");
        self.at_place[place].insert(position, slot);
        self.claims[slot] = Some(claim);
        self.live_count += 1;
        Ok(())
    }
    fn under(&self, place: PlaceId) -> Vec<Binding> {
        self.at(place)
            .map(|claim| Binding {
                claim: claim.identity,
                path: claim.path.clone(),
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
struct OperationClaims {
    operation: OperationId,
    bindings: Vec<Binding>,
}

#[derive(Clone, Debug)]
struct ReturnClaims {
    edge: EdgeId,
    bindings: Option<Vec<Binding>>,
}

#[derive(Clone, Debug, Default)]
pub struct ClaimsPlan {
    pub entry_claims: Vec<EntryClaim>,
    pub block_entry_claims: Vec<EntryClaim>,
    operation_results: Vec<OperationClaims>,
    pub call_transfers: BTreeMap<InvocationSite, Vec<ClaimTransfer>>,
    pub completion_receipts: BTreeMap<InvocationSite, Vec<CompletionReceipt>>,
    returned_claims: Vec<ReturnClaims>,
    frontiers: BTreeMap<InvocationSite, Vec<ClaimLocation>>,
    unreachable: BTreeSet<InvocationSite>,
}
impl ClaimsPlan {
    pub fn call_frontier(&self, site: InvocationSite) -> Option<&[ClaimLocation]> {
        self.frontiers.get(&site).map(Vec::as_slice)
    }
    pub fn call_is_unreachable(&self, site: InvocationSite) -> bool {
        self.unreachable.contains(&site)
    }
    pub fn install(&self, machine: &mut TerminalMachine) -> Result<(), Error> {
        machine.contract.entry_claims.clone_from(&self.entry_claims);
        machine
            .block_entry_claims
            .clone_from(&self.block_entry_claims);
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if let terminal_psi::OperationResult::Structural(result) = &mut operation.result {
                    result.claims = self
                        .operation_results
                        .binary_search_by_key(&operation.id, |row| row.operation)
                        .ok()
                        .map(|position| self.operation_results[position].bindings.clone())
                        .unwrap_or_default();
                }
                let site = InvocationSite::Operation(operation.id);
                match &mut operation.kind {
                    Op::CallUnit {
                        claim_transfers, ..
                    }
                    | Op::CallStructuralScalar {
                        claim_transfers, ..
                    }
                    | Op::CallStructural {
                        claim_transfers, ..
                    }
                    | Op::CallStructuralWithScalarArguments {
                        claim_transfers, ..
                    }
                    | Op::CallMachine {
                        claim_transfers, ..
                    }
                    | Op::CallDynamic {
                        claim_transfers, ..
                    } => {
                        *claim_transfers =
                            self.call_transfers.get(&site).cloned().unwrap_or_default()
                    }
                    Op::BoundaryCall {
                        completion_receipts,
                        ..
                    } => {
                        *completion_receipts = self
                            .completion_receipts
                            .get(&site)
                            .cloned()
                            .unwrap_or_default()
                    }
                    _ => {}
                }
            }
            if let Terminator::InvokeNoNormal {
                edge,
                claim_transfers,
                ..
            } = &mut block.terminator
            {
                *claim_transfers = self
                    .call_transfers
                    .get(&InvocationSite::InvokeNoNormal(*edge))
                    .cloned()
                    .unwrap_or_default();
            }
            if let Terminator::ReturnStructural {
                edge,
                returned_claims,
                ..
            } = &mut block.terminator
            {
                *returned_claims = self
                    .returned_claims
                    .binary_search_by_key(edge, |row| row.edge)
                    .ok()
                    .and_then(|position| self.returned_claims[position].bindings.clone())
                    .unwrap_or_default();
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Publish,
    Verify,
}

struct TypeClaims {
    structural_type: StructuralTypeId,
    paths: Option<Vec<Vec<Path>>>,
}
struct Catalogues(Vec<TypeClaims>);
impl Catalogues {
    fn new(types: &[StructuralTypeDeclaration]) -> Result<Self, Error> {
        let mut rows = types
            .iter()
            .map(|row| TypeClaims {
                structural_type: row.id,
                paths: None,
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|row| row.structural_type);
        if rows
            .windows(2)
            .any(|pair| pair[0].structural_type == pair[1].structural_type)
        {
            return Err(Error::malformed(
                "claim catalogue repeats a structural type",
            ));
        }
        Ok(Self(rows))
    }
    fn paths(
        &mut self,
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
    ) -> Result<&[Vec<Path>], Error> {
        let position = self
            .0
            .binary_search_by_key(&root, |row| row.structural_type)
            .map_err(|_| Error::malformed("claim catalogue names an absent structural type"))?;
        let paths = &mut self.0[position].paths;
        if paths.is_none() {
            *paths = Some(
                crate::owned_structure::claim_catalogue(
                    |id| terminal_psi::find_structural_type(types, id),
                    root,
                    usize::MAX,
                )
                .map_err(Error::malformed)?
                .into_iter()
                .collect(),
            );
        }
        Ok(paths.as_deref().expect("queried claim catalogue retained"))
    }
}
fn fresh(next: &mut u64) -> Result<ClaimId, Error> {
    let id = ClaimId::new(*next).ok_or_else(|| Error::malformed("claim identity overflows"))?;
    *next = next
        .checked_add(1)
        .ok_or_else(|| Error::malformed("claim identity overflows"))?;
    Ok(id)
}
fn validate_bindings(expected: &[Vec<Path>], supplied: &[Binding]) -> Result<(), Error> {
    let paths = supplied
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    if supplied
        .windows(2)
        .any(|rows| rows[0].claim >= rows[1].claim)
        || paths.len() != supplied.len()
        || paths.len() != expected.len()
        || expected.iter().any(|path| !paths.contains(path))
    {
        return Err(Error::malformed(
            "claim bindings do not cover the exact guarded type catalogue",
        ));
    }
    Ok(())
}

/// Header claims name parameter custody and possible returned paths, without
/// granting an implementation or saying that every outcome selects each path.
pub fn publish_contract_claims(
    contract: &mut MachineContract,
    types: &[StructuralTypeDeclaration],
) -> Result<(), Error> {
    let mut cache = Catalogues::new(types)?;
    let mut next = 1;
    let mut entries = Vec::new();
    for formal in &contract.structural_parameters {
        if formal.access != StructuralAccess::Owned {
            continue;
        }
        for path in cache.paths(types, formal.structural_type)? {
            entries.push(EntryClaim {
                claim: fresh(&mut next)?,
                input: formal.place,
                path: path.clone(),
            });
        }
    }
    contract.entry_claims = entries;

    Ok(())
}

pub(super) struct Flow {
    mode: Mode,
    catalogues: Catalogues,
    fresh_results: Vec<OperationClaims>,
    pub plan: ClaimsPlan,
}
impl Flow {
    pub fn new(
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
        mode: Mode,
    ) -> Result<(Self, ClaimsState), Error> {
        let mut contract = machine.contract.clone();
        if mode == Mode::Publish {
            publish_contract_claims(&mut contract, types)?;
        }
        let mut cache = Catalogues::new(types)?;
        let mut assigned = BTreeSet::new();
        let mut next = 1;
        let mut check_id = |id: ClaimId| -> Result<(), Error> {
            if !assigned.insert(id) {
                return Err(Error::malformed("claim declaration repeats an identity"));
            }
            next = next.max(
                id.get()
                    .checked_add(1)
                    .ok_or_else(|| Error::malformed("claim identity overflows"))?,
            );
            Ok(())
        };
        let mut entry_locations = Vec::new();
        for formal in &contract.structural_parameters {
            let rows = contract
                .entry_claims
                .iter()
                .filter(|row| row.input == formal.place)
                .map(|row| Binding {
                    claim: row.claim,
                    path: row.path.clone(),
                })
                .collect::<Vec<_>>();
            let expected = if formal.access == StructuralAccess::Owned {
                cache.paths(types, formal.structural_type)?
            } else {
                &[]
            };
            validate_bindings(expected, &rows)?;
            for row in rows {
                check_id(row.claim)?;
                entry_locations.push(ClaimLocation {
                    identity: row.claim,
                    place: formal.place,
                    path: row.path,
                });
            }
        }
        if contract.entry_claims.len() != entry_locations.len() {
            return Err(Error::malformed("entry claim names an absent formal"));
        }
        let mut block_claims = Vec::new();
        for block in &machine.blocks {
            for formal in &block.structural_parameters {
                let expected = if formal.access == StructuralAccess::Owned {
                    cache.paths(types, formal.structural_type)?
                } else {
                    &[]
                };
                let rows = if mode == Mode::Publish {
                    expected
                        .iter()
                        .map(|path| {
                            Ok(Binding {
                                claim: fresh(&mut next)?,
                                path: path.clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?
                } else {
                    machine
                        .block_entry_claims
                        .iter()
                        .filter(|row| row.input == formal.place)
                        .map(|row| Binding {
                            claim: row.claim,
                            path: row.path.clone(),
                        })
                        .collect()
                };
                validate_bindings(expected, &rows)?;
                for row in rows {
                    if !assigned.insert(row.claim) {
                        return Err(Error::malformed("block claim repeats an identity"));
                    }
                    block_claims.push(EntryClaim {
                        claim: row.claim,
                        input: formal.place,
                        path: row.path,
                    });
                }
            }
        }
        if mode == Mode::Verify && block_claims.len() != machine.block_entry_claims.len() {
            return Err(Error::malformed("block claim names an absent formal"));
        }
        let mut fresh_results = Vec::new();
        for operation in machine.operations() {
            let Some(result) = operation.result.as_structural() else {
                continue;
            };
            let expected = cache.paths(types, result.structural_type)?;
            let rows = if mode == Mode::Publish {
                expected
                    .iter()
                    .map(|path| {
                        Ok(Binding {
                            claim: fresh(&mut next)?,
                            path: path.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, Error>>()?
            } else {
                result.claims.clone()
            };
            if matches!(
                operation.kind,
                Op::CallStructural { .. }
                    | Op::CallStructuralWithScalarArguments { .. }
                    | Op::CallMachine { .. }
                    | Op::BoundaryCall { .. }
                    | Op::CallDynamic { .. }
            ) {
                validate_bindings(expected, &rows)?;
            }
            fresh_results.push(OperationClaims {
                operation: operation.id,
                bindings: rows,
            });
        }
        fresh_results.sort_unstable_by_key(|row| row.operation);
        if fresh_results
            .windows(2)
            .any(|pair| pair[0].operation == pair[1].operation)
        {
            return Err(Error::malformed("claim results repeat an operation"));
        }
        let claim_ids = contract
            .entry_claims
            .iter()
            .chain(&block_claims)
            .map(|row| row.claim)
            .chain(
                fresh_results
                    .iter()
                    .flat_map(|row| row.bindings.iter().map(|binding| binding.claim)),
            )
            .collect();
        let mut state = ClaimsState::new(machine, claim_ids)?;
        for claim in entry_locations {
            state.put(claim)?;
        }
        let operation_results = fresh_results
            .iter()
            .map(|row| OperationClaims {
                operation: row.operation,
                bindings: Vec::new(),
            })
            .collect();
        let mut returned_claims = machine
            .blocks
            .iter()
            .filter_map(|block| {
                if let Terminator::ReturnStructural { edge, .. } = &block.terminator {
                    Some(ReturnClaims {
                        edge: *edge,
                        bindings: None,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        returned_claims.sort_unstable_by_key(|row| row.edge);
        if returned_claims
            .windows(2)
            .any(|pair| pair[0].edge == pair[1].edge)
        {
            return Err(Error::malformed("claim returns repeat an edge"));
        }
        Ok((
            Self {
                mode,
                catalogues: cache,
                fresh_results,
                plan: ClaimsPlan {
                    entry_claims: contract.entry_claims,
                    block_entry_claims: block_claims,
                    operation_results,
                    returned_claims,
                    ..ClaimsPlan::default()
                },
            },
            state,
        ))
    }
    fn fresh_result(&self, operation: OperationId) -> Option<&[Binding]> {
        self.fresh_results
            .binary_search_by_key(&operation, |row| row.operation)
            .ok()
            .map(|position| self.fresh_results[position].bindings.as_slice())
    }
    fn record_result(&mut self, operation: &Operation, state: &ClaimsState) -> Result<(), Error> {
        if let Some(result) = operation.result.as_structural() {
            let bindings = state.under(result.place);
            if self.mode == Mode::Verify && result.claims != bindings {
                return Err(Error::malformed(
                    "operation result claims do not match transported custody",
                ));
            }
            let position = self
                .plan
                .operation_results
                .binary_search_by_key(&operation.id, |row| row.operation)
                .map_err(|_| Error::malformed("claim result names an undeclared operation"))?;
            self.plan.operation_results[position].bindings = bindings;
        }
        Ok(())
    }
    fn field_path(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[Path],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<Vec<Path>, Error> {
        let mut selected = path.to_vec();
        if let Some(field) = field {
            let member = crate::projected_field_declaration(types.iter(), root, path, field)
                .ok_or_else(|| {
                    Error::malformed("claim transport field is not declared on its exact carrier")
                })?;
            selected.push(Path::Field(member.identity));
        }
        Ok(selected)
    }
    fn relocate(
        state: &mut ClaimsState,
        argument: &StructuralArgument,
        destination: PlaceId,
        prefix: &[Path],
    ) -> Result<(), Error> {
        for mut claim in state.take(argument)? {
            let mut path = prefix.to_vec();
            path.extend(claim.path);
            claim.place = destination;
            claim.path = path;
            state.put(claim)?;
        }
        Ok(())
    }
    fn snapshot(&mut self, site: InvocationSite, state: &ClaimsState, settled: bool) {
        if settled {
            self.plan
                .frontiers
                .insert(site, state.live().cloned().collect::<Vec<_>>());
        }
    }
    pub fn operation(
        &mut self,
        state: &mut ClaimsState,
        operation: &Operation,
        types: &[StructuralTypeDeclaration],
        roots: &[Option<super::Root>],
        settled: bool,
    ) -> Result<(), Error> {
        let root_type = |place: PlaceId| {
            roots
                .get(super::index(place.get())?)
                .and_then(Option::as_ref)
                .map(|row| row.structural_type)
                .ok_or_else(|| Error::malformed("claim field has no exact owned carrier"))
        };
        let site = InvocationSite::Operation(operation.id);
        let is_call = matches!(
            operation.kind,
            Op::Call { .. }
                | Op::CallUnit { .. }
                | Op::CallStructuralScalar { .. }
                | Op::CallStructural { .. }
                | Op::CallStructuralWithScalarArguments { .. }
                | Op::CallMachine { .. }
                | Op::BoundaryCall { .. }
                | Op::CallDynamic { .. }
        );
        if is_call {
            self.snapshot(site, state, settled);
            let held = state.live().cloned().collect::<Vec<_>>();
            let mut transfers = Vec::new();
            for (ordinal, argument) in terminal_psi::call_structural_arguments(&operation.kind)
                .iter()
                .enumerate()
            {
                for claim in state.take(argument)? {
                    transfers.push(ClaimTransfer {
                        claim: claim.identity,
                        argument_index: u32::try_from(ordinal)
                            .map_err(|_| Error::malformed("claim argument ordinal overflows"))?,
                        path: claim.path,
                    });
                }
            }
            transfers.sort_by_key(|row| row.claim);
            crate::owned_structure::validate_claim_transfers(
                terminal_psi::call_structural_arguments(&operation.kind),
                held.iter()
                    .map(|claim| (claim.identity, claim.place, claim.path.as_slice())),
                transfers
                    .iter()
                    .map(|row| (row.claim, row.argument_index, row.path.as_slice())),
            )
            .map_err(Error::malformed)?;
            if matches!(operation.kind, Op::BoundaryCall { .. }) {
                let receipts = transfers
                    .iter()
                    .map(|row| CompletionReceipt {
                        claim: row.claim,
                        argument_index: row.argument_index,
                        path: row.path.clone(),
                    })
                    .collect::<Vec<_>>();
                if self.mode == Mode::Verify {
                    let Op::BoundaryCall {
                        completion_receipts,
                        ..
                    } = &operation.kind
                    else {
                        unreachable!()
                    };
                    if completion_receipts != &receipts {
                        return Err(Error::malformed(
                            "boundary completion receipts differ from exact transferred custody",
                        ));
                    }
                }
                if settled {
                    self.plan.completion_receipts.insert(site, receipts);
                }
            } else {
                if self.mode == Mode::Verify && operation.kind.claim_transfers() != transfers {
                    return Err(Error::malformed(
                        "call transfers differ from exact incoming custody",
                    ));
                }
                if settled {
                    self.plan.call_transfers.insert(site, transfers);
                }
            }
            if let Some(result) = operation.result.as_structural() {
                for binding in self.fresh_result(operation.id).into_iter().flatten() {
                    state.put(ClaimLocation {
                        identity: binding.claim,
                        place: result.place,
                        path: binding.path.clone(),
                    })?;
                }
            }
        } else {
            match &operation.kind {
                Op::MoveStructuralValue { source } | Op::TransformContent { source, .. } => {
                    if let Some(result) = operation.result.as_structural() {
                        Self::relocate(state, source, result.place, &[])?;
                    }
                }
                Op::MoveStructuralField {
                    source,
                    path,
                    field,
                } => {
                    let selected = if state.at(*source).next().is_some() {
                        Self::field_path(types, root_type(*source)?, path, *field)?
                    } else {
                        Vec::new()
                    };
                    if let Some(result) = operation.result.as_structural() {
                        Self::relocate(
                            state,
                            &StructuralArgument {
                                place: *source,
                                path: selected,
                                access: StructuralAccess::Owned,
                            },
                            result.place,
                            &[],
                        )?;
                    }
                }
                Op::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    if state.at(value.place).next().is_some() {
                        let selected =
                            Self::field_path(types, root_type(*destination)?, path, *field)?;
                        Self::relocate(state, value, *destination, &selected)?;
                    }
                }
                Op::EstablishRecord { fields } | Op::EstablishStructuralCase { fields, .. } => {
                    if let Some(result) = operation.result.as_structural() {
                        for field in fields {
                            let RecordFieldValue::Structural(argument) = &field.value else {
                                continue;
                            };
                            let mut prefix = Vec::new();
                            if let Op::EstablishStructuralCase { result_case, .. } = &operation.kind
                            {
                                prefix.push(Path::Case(*result_case));
                            }
                            let member=crate::projected_field_declaration(types.iter(),result.structural_type,&prefix,field.field).ok_or_else(||Error::malformed("claim constructor field does not belong to its declared carrier"))?;
                            prefix.push(Path::Field(member.identity));
                            Self::relocate(state, argument, result.place, &prefix)?;
                        }
                        if terminal_psi::find_structural_type(types, result.structural_type)
                            .is_some_and(|ty| ty.owns_linear_claim)
                        {
                            let row = self
                                .fresh_result(operation.id)
                                .and_then(|rows| rows.iter().find(|row| row.path.is_empty()))
                                .ok_or_else(|| {
                                    Error::malformed(
                                        "logical constructor has no exact intrinsic root binding",
                                    )
                                })?;
                            state.put(ClaimLocation {
                                identity: row.claim,
                                place: result.place,
                                path: Vec::new(),
                            })?;
                        }
                    }
                }
                Op::EstablishStructuralArray { elements } => {
                    if let Some(result) = operation.result.as_structural() {
                        for (index, argument) in elements.iter().enumerate() {
                            Self::relocate(
                                state,
                                argument,
                                result.place,
                                &[Path::FixedIndex(u64::try_from(index).map_err(|_| {
                                    Error::malformed("claim array index overflows")
                                })?)],
                            )?;
                        }
                    }
                }
                Op::EstablishZeroedStructural { .. } => {
                    if let Some(result) = operation.result.as_structural()
                        && !self
                            .catalogues
                            .paths(types, result.structural_type)?
                            .is_empty()
                    {
                        return Err(Error::unsupported(
                            "implicit zero-filled storage establishes no linear claim",
                        ));
                    }
                }
                Op::StructuralLeafCopy { source, path } => {
                    for claim in state.at(*source) {
                        let overlaps = crate::structural_claim_paths_may_overlap(
                            types.iter(),
                            root_type(*source)?,
                            &claim.path,
                            path,
                        )
                        .ok_or_else(|| {
                            Error::malformed("claim copy has an invalid storage path")
                        })?;
                        if overlaps {
                            return Err(Error::malformed(
                                "a copy duplicates logical claim custody",
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        if settled {
            self.record_result(operation, state)?;
        }
        Ok(())
    }
    pub fn select_case(
        &self,
        state: &mut ClaimsState,
        terminator: &Terminator,
        edge: EdgeId,
    ) -> Result<(), Error> {
        let Terminator::StructuralCase {
            source,
            path,
            cases,
            ..
        } = terminator
        else {
            return Ok(());
        };
        let Some(selected) = cases.iter().find(|case| case.edge == edge) else {
            return Ok(());
        };
        state.take_at(*source, |claim| {
            claim.path.starts_with(path)
                && matches!(claim.path.get(path.len()), Some(Path::Case(case)) if *case != selected.case)
        });
        Ok(())
    }
    pub fn successor(
        &mut self,
        state: &mut ClaimsState,
        machine: &TerminalMachine,
        target: BlockId,
        arguments: &[StructuralArgument],
    ) -> Result<(), Error> {
        let block = machine
            .block(&target)
            .ok_or_else(|| Error::malformed("claim successor block is absent"))?;
        let captured = arguments
            .iter()
            .zip(&block.structural_parameters)
            .map(|(argument, formal)| {
                // An owned argument may lend to a borrowed destination. That
                // attenuation preserves its owner's custody at the source.
                if formal.access == StructuralAccess::Owned {
                    state.take(argument)
                } else {
                    Ok(Vec::new())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (formal, claims) in block.structural_parameters.iter().zip(captured) {
            for mut claim in claims {
                let binding = self
                    .plan
                    .block_entry_claims
                    .iter()
                    .find(|binding| binding.input == formal.place && binding.path == claim.path)
                    .ok_or_else(|| {
                        Error::malformed("claim arrival has no exact destination binding")
                    })?;
                claim.identity = binding.claim;
                claim.place = formal.place;
                state.put(claim)?;
            }
        }
        Ok(())
    }
    pub fn terminal(
        &mut self,
        state: &mut ClaimsState,
        terminator: &Terminator,
        settled: bool,
    ) -> Result<(), Error> {
        if let Terminator::InvokeNoNormal {
            edge,
            structural_arguments,
            claim_transfers,
            ..
        } = terminator
        {
            let site = InvocationSite::InvokeNoNormal(*edge);
            self.snapshot(site, state, settled);
            let mut rows = Vec::new();
            for (ordinal, argument) in structural_arguments.iter().enumerate() {
                for claim in state.take(argument)? {
                    rows.push(ClaimTransfer {
                        claim: claim.identity,
                        argument_index: u32::try_from(ordinal)
                            .map_err(|_| Error::malformed("terminal claim ordinal overflows"))?,
                        path: claim.path,
                    });
                }
            }
            rows.sort_by_key(|row| row.claim);
            if self.mode == Mode::Verify && &rows != claim_transfers {
                return Err(Error::malformed(
                    "terminal call claim rows differ from actual custody",
                ));
            }
            if settled {
                self.plan.call_transfers.insert(site, rows);
            }
        }
        if let Terminator::ExternalCompletion {
            edge,
            structural_arguments,
            ..
        } = terminator
        {
            self.snapshot(InvocationSite::ExternalCompletion(*edge), state, settled);
            for argument in structural_arguments {
                if !state.take(argument)?.is_empty() {
                    return Err(Error::unsupported(
                        "external completion with owned claims needs its declared custody handoff",
                    ));
                }
            }
        }
        if let Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            ..
        } = terminator
        {
            let returned = state
                .take(&StructuralArgument::whole(*source, StructuralAccess::Owned))?
                .into_iter()
                .map(|claim| Binding {
                    claim: claim.identity,
                    path: claim.path,
                })
                .collect::<Vec<_>>();
            if self.mode == Mode::Verify && returned != *returned_claims {
                return Err(Error::malformed(
                    "return claims differ from actual returned custody",
                ));
            }
            if settled {
                let position = self
                    .plan
                    .returned_claims
                    .binary_search_by_key(edge, |row| row.edge)
                    .map_err(|_| Error::malformed("claim return names an undeclared edge"))?;
                self.plan.returned_claims[position].bindings = Some(returned);
            }
        }
        if matches!(
            terminator,
            Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
        ) && state.live_count != 0
        {
            return Err(Error::unsupported(
                "a normal return leaves an untransferred logical claim",
            ));
        }
        Ok(())
    }
    pub fn cleanup(
        &mut self,
        state: &ClaimsState,
        edge: EdgeId,
        actions: &[TerminalAffineCleanupAction],
        settled: bool,
    ) -> Result<(), Error> {
        for (ordinal, action) in actions.iter().enumerate() {
            let TerminalAffineCleanupAction::InvokeOwned(_) = action else {
                continue;
            };
            let site = InvocationSite::Cleanup {
                edge,
                action_ordinal: u32::try_from(ordinal)
                    .map_err(|_| Error::malformed("cleanup claim ordinal overflows"))?,
            };
            self.snapshot(site, state, settled);
            if state.at(action.place()).next().is_some() {
                return Err(Error::unsupported(
                    "claim-bearing cleanup needs its exact consuming disposition",
                ));
            }
        }
        Ok(())
    }
    pub fn unreachable_block(&mut self, block: &terminal_psi::Block) {
        for operation in &block.operations {
            if let Ok(position) = self
                .fresh_results
                .binary_search_by_key(&operation.id, |row| row.operation)
            {
                self.plan.operation_results[position]
                    .bindings
                    .clone_from(&self.fresh_results[position].bindings);
            }
            if matches!(
                operation.kind,
                Op::Call { .. }
                    | Op::CallUnit { .. }
                    | Op::CallStructuralScalar { .. }
                    | Op::CallStructural { .. }
                    | Op::CallStructuralWithScalarArguments { .. }
                    | Op::CallMachine { .. }
                    | Op::BoundaryCall { .. }
                    | Op::CallDynamic { .. }
            ) {
                self.plan
                    .unreachable
                    .insert(InvocationSite::Operation(operation.id));
            }
        }
        match &block.terminator {
            Terminator::InvokeNoNormal { edge, .. } => {
                self.plan
                    .unreachable
                    .insert(InvocationSite::InvokeNoNormal(*edge));
            }
            Terminator::ExternalCompletion { edge, .. } => {
                self.plan
                    .unreachable
                    .insert(InvocationSite::ExternalCompletion(*edge));
            }
            _ => {}
        }
        for (edge, actions) in block.terminator.cleanup_rosters() {
            for (ordinal, action) in actions.iter().enumerate() {
                if matches!(action, TerminalAffineCleanupAction::InvokeOwned(_))
                    && let Ok(action_ordinal) = u32::try_from(ordinal)
                {
                    self.plan.unreachable.insert(InvocationSite::Cleanup {
                        edge,
                        action_ordinal,
                    });
                }
            }
        }
    }
    pub fn finish(
        self,
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
    ) -> Result<ClaimsPlan, Error> {
        self.verify_catalogues(machine, types)?;
        Ok(self.plan)
    }
    fn verify_catalogues(
        &self,
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
    ) -> Result<(), Error> {
        if let Some(result) = machine.result.structural() {
            let paths = crate::owned_structure::claim_catalogue(
                |id| terminal_psi::find_structural_type(types, id),
                result.structural_type,
                usize::MAX,
            )
            .map_err(Error::malformed)?
            .into_iter()
            .collect::<Vec<_>>();
            let returns = self
                .plan
                .returned_claims
                .iter()
                .filter_map(|row| row.bindings.as_deref());
            if !crate::result_claim_paths_match_returns(&paths, returns) {
                return Err(Error::malformed(
                    "normal result claim paths differ from the guarded declared result type",
                ));
            }
        }
        Ok(())
    }
}

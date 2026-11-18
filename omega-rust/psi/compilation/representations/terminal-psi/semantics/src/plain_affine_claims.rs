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
    /// Independent content remains owed after the payload wrapper is ended.
    pub content: Vec<terminal_psi::ClaimContentProjection>,
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
    fn get(&self, id: ClaimId) -> Option<&ClaimLocation> {
        self.claim_ids
            .binary_search(&id)
            .ok()
            .and_then(|slot| self.claims[slot].as_ref())
    }
    pub(super) fn live(&self) -> impl Iterator<Item = &ClaimLocation> {
        self.claims.iter().flatten()
    }
    pub(super) fn at(&self, place: PlaceId) -> impl Iterator<Item = &ClaimLocation> {
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
    content: Vec<terminal_psi::ContentEntryClaim>,
    transformation: Option<Op>,
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
                let row = self
                    .operation_results
                    .binary_search_by_key(&operation.id, |row| row.operation)
                    .ok()
                    .map(|position| &self.operation_results[position]);
                if let Some(kind) = row.and_then(|row| row.transformation.as_ref()) {
                    operation.kind.clone_from(kind);
                }
                if let terminal_psi::OperationResult::Structural(result) = &mut operation.result {
                    result.claims = row.map(|row| row.bindings.clone()).unwrap_or_default();
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
struct Catalogues<'types> {
    rows: Vec<TypeClaims>,
    types: crate::owned_structure::ClaimTypes<'types>,
}
impl<'types> Catalogues<'types> {
    fn new(types: &'types [StructuralTypeDeclaration]) -> Result<Self, Error> {
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
        Ok(Self {
            rows,
            types: crate::owned_structure::ClaimTypes::new(types).map_err(Error::malformed)?,
        })
    }
    fn paths(&mut self, root: StructuralTypeId) -> Result<&[Vec<Path>], Error> {
        let position = self
            .rows
            .binary_search_by_key(&root, |row| row.structural_type)
            .map_err(|_| Error::malformed("claim catalogue names an absent structural type"))?;
        let paths = &mut self.rows[position].paths;
        if paths.is_none() {
            *paths = Some(
                crate::owned_structure::claim_catalogue(&self.types, root, usize::MAX)
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
    publish_claims(contract, &mut cache)
}

fn publish_claims(contract: &mut MachineContract, cache: &mut Catalogues<'_>) -> Result<(), Error> {
    let mut next = 1;
    let mut entries = Vec::new();
    for formal in &contract.structural_parameters {
        if formal.access != StructuralAccess::Owned {
            continue;
        }
        for path in cache.paths(formal.structural_type)? {
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

pub(super) struct Flow<'types> {
    mode: Mode,
    catalogues: Catalogues<'types>,
    projections: Option<crate::content_claims::ClaimProjections<'types>>,
    entry_content: Vec<terminal_psi::ContentEntryClaim>,
    result_subject: Option<PlaceId>,
    fresh_results: Vec<OperationClaims>,
    pub plan: ClaimsPlan,
}
impl<'types> Flow<'types> {
    pub fn new(
        machine: &TerminalMachine,
        types: &'types [StructuralTypeDeclaration],
        mode: Mode,
        module: Option<&'types terminal_psi::TerminalModule>,
    ) -> Result<(Self, ClaimsState), Error> {
        let mut contract = machine.contract.clone();
        let mut cache = Catalogues::new(types)?;
        if mode == Mode::Publish {
            publish_claims(&mut contract, &mut cache)?;
        }
        let content = module
            .map(|module| {
                crate::content_claims::ClaimProjections::new(
                    types,
                    &module.structural_domains,
                    &module.identities,
                )
            })
            .transpose()
            .map_err(Error::malformed)?;
        if let Some(content) = &content {
            content
                .validate_entries(&contract)
                .map_err(Error::malformed)?;
        } else if !contract.content_entry_claims.is_empty() {
            return Err(Error::unsupported(
                "content claim transport requires its exact module",
            ));
        }
        let mut entry_content = contract.content_entry_claims.iter().collect::<Vec<_>>();
        entry_content.sort_unstable_by_key(|row| row.claim);
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
                cache.paths(formal.structural_type)?
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
                    content: entry_content
                        .binary_search_by_key(&row.claim, |row| row.claim)
                        .ok()
                        .map(|position| entry_content[position].projections.clone())
                        .unwrap_or_default(),
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
                    cache.paths(formal.structural_type)?
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
            let expected = cache.paths(result.structural_type)?;
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
            let obligations = content
                .as_ref()
                .map(|content| {
                    content.at_subject(
                        crate::content_claims::Subject {
                            place: result.place,
                            carrier: result.structural_type,
                            qualifications: &result.qualifications,
                            projected_qualifications: &result.projected_qualifications,
                        },
                        rows.iter().map(|row| (row.claim, row.path.as_slice())),
                        semantic_vocabulary::ContentPlaceVersion::Current,
                    )
                })
                .transpose()
                .map_err(Error::unsupported)?
                .unwrap_or_default();
            fresh_results.push(OperationClaims {
                operation: operation.id,
                bindings: rows,
                content: obligations,
                transformation: None,
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
                content: Vec::new(),
                transformation: None,
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
                projections: content,
                entry_content: contract.content_entry_claims,
                result_subject: contract.result.structural().map(|result| result.place),
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
    fn fresh_content(
        &self,
        operation: OperationId,
        claim: ClaimId,
    ) -> Vec<terminal_psi::ClaimContentProjection> {
        self.fresh_results
            .binary_search_by_key(&operation, |row| row.operation)
            .ok()
            .and_then(|position| {
                let rows = &self.fresh_results[position].content;
                rows.binary_search_by_key(&claim, |row| row.claim)
                    .ok()
                    .map(|index| &rows[index])
            })
            .map(|row| row.projections.clone())
            .unwrap_or_default()
    }
    fn check_boundary_content(
        &self,
        subject: crate::content_claims::Subject<'_>,
        claims: &[ClaimLocation],
    ) -> Result<(), Error> {
        let Some(projections) = &self.projections else {
            return if claims.iter().all(|claim| claim.content.is_empty()) {
                Ok(())
            } else {
                Err(Error::unsupported(
                    "content transport requires its exact module",
                ))
            };
        };
        projections
            .validate_transport(
                subject,
                claims.iter().map(|claim| {
                    (
                        claim.identity,
                        claim.path.as_slice(),
                        claim.content.as_slice(),
                    )
                }),
            )
            .map_err(Error::malformed)
    }

    fn take_declared(
        &self,
        state: &mut ClaimsState,
        argument: &StructuralArgument,
        types: &[StructuralTypeDeclaration],
        roots: &[Option<super::Root>],
    ) -> Result<Vec<ClaimLocation>, Error> {
        let claims = state.take(argument)?;
        if argument.access != StructuralAccess::Owned {
            return Ok(claims);
        }
        let Some(root) = roots
            .get(super::index(argument.place.get())?)
            .and_then(Option::as_ref)
        else {
            return if claims.is_empty() {
                Ok(claims)
            } else {
                Err(Error::malformed(
                    "content transport has claims without a typed root",
                ))
            };
        };
        let carrier =
            crate::runtime_structural_path_tip(types.iter(), root.structural_type, &argument.path)
                .ok_or_else(|| Error::malformed("content transport has no exact argument carrier"))?
                .id;
        let qualifications = if argument.path.is_empty() {
            root.memberships.declared.whole.clone()
        } else {
            root.memberships
                .declared
                .projected
                .iter()
                .filter(|row| row.path == argument.path)
                .map(|row| row.domain)
                .collect()
        };
        let projected = root
            .memberships
            .declared
            .projected
            .iter()
            .filter_map(|row| {
                let suffix = row.path.strip_prefix(argument.path.as_slice())?;
                (!suffix.is_empty()).then(|| terminal_psi::StructuralPathQualification {
                    path: suffix.to_vec(),
                    domain: row.domain,
                })
            })
            .collect::<Vec<_>>();
        self.check_boundary_content(
            crate::content_claims::Subject {
                place: argument.place,
                carrier,
                qualifications: &qualifications,
                projected_qualifications: &projected,
            },
            &claims,
        )?;
        Ok(claims)
    }

    fn check_content_roster(
        &self,
        claims: &[ClaimLocation],
        derivations: &[terminal_psi::ContentTransformationDerivation],
        input: bool,
        carrier: &impl Fn(PlaceId) -> Result<StructuralTypeId, Error>,
    ) -> Result<(), Error> {
        let projections = self
            .projections
            .as_ref()
            .ok_or_else(|| Error::unsupported("content claim replay requires its exact module"))?;
        let mut claims = claims.iter().collect::<Vec<_>>();
        claims.sort_unstable_by_key(|claim| claim.identity);
        if claims
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(Error::malformed(
                "content transformation repeats a live claim",
            ));
        }
        let mut expected = claims
            .iter()
            .flat_map(|claim| {
                claim
                    .content
                    .iter()
                    .cloned()
                    .map(move |projection| (claim.identity, projection))
            })
            .collect::<Vec<_>>();
        let mut supplied = Vec::new();
        for derived in derivations {
            for binding in if input {
                &derived.inputs
            } else {
                &derived.outputs
            } {
                let claim = claims
                    .binary_search_by_key(&binding.claim, |claim| claim.identity)
                    .ok()
                    .map(|position| claims[position])
                    .ok_or_else(|| {
                        Error::malformed("content equation names no consumed or produced claim")
                    })?;
                let path = projections
                    .resolve_subject(carrier(binding.location.root)?, &binding.location)
                    .map_err(Error::malformed)?;
                if binding.location.version != semantic_vocabulary::ContentPlaceVersion::Current
                    || binding.location.root != claim.place
                    || path != claim.path
                {
                    return Err(Error::malformed(
                        "content equation changes the current claim location",
                    ));
                }
                if input {
                    crate::content_transformations::validate_input_snapshot(
                        &self.entry_content,
                        derived.conservation.algebra(),
                        binding,
                    )
                    .map_err(Error::malformed)?;
                } else if binding.subject != binding.location {
                    return Err(Error::malformed(
                        "content result observes another value revision",
                    ));
                }
                supplied.push((
                    claim.identity,
                    terminal_psi::ClaimContentProjection {
                        projection: binding.projection,
                        algebra: derived.conservation.algebra().clone(),
                    },
                ));
            }
        }
        expected.sort();
        supplied.sort();
        if expected != supplied {
            return Err(Error::malformed(
                "content equation omits or changes a live projection obligation",
            ));
        }
        Ok(())
    }
    /// Bind an unsealed source request to the claims live at this operation.
    /// Receiving only checks the resulting rows; it never fills missing evidence.
    pub fn materialize_content(
        &mut self,
        state: &ClaimsState,
        operation: &Operation,
        roots: &[Option<super::Root>],
        settled: bool,
    ) -> Result<Option<Operation>, Error> {
        use semantic_vocabulary::{ContentPlaceVersion, ContentTerm};
        use terminal_psi::{ContentClaimProjection, ContentTransformationCustody};
        let Op::TransformContent {
            source,
            derivations,
            ..
        } = &operation.kind
        else {
            return Ok(None);
        };
        let pending = derivations
            .iter()
            .any(|row| row.inputs.is_empty() || row.outputs.is_empty());
        if !pending || self.mode != Mode::Publish {
            return Ok(None);
        }
        if derivations.is_empty()
            || derivations.iter().any(|row| {
                !row.inputs.is_empty()
                    || !row.outputs.is_empty()
                    || !row.lineage_roots.is_empty()
                    || !row.ancestry.is_empty()
                    || row.custody != ContentTransformationCustody::Derived
            })
        {
            return Err(Error::malformed(
                "content request mixes pending and supplied custody",
            ));
        }
        let result = operation
            .result
            .as_structural()
            .ok_or_else(|| Error::malformed("content request has no structural result"))?;
        let formal_result = self
            .result_subject
            .ok_or_else(|| Error::malformed("content request has no declared result subject"))?;
        let projections = self
            .projections
            .as_ref()
            .ok_or_else(|| Error::unsupported("content request needs its exact module"))?;
        let root_type = |place: PlaceId| {
            roots
                .get(super::index(place.get())?)
                .and_then(Option::as_ref)
                .map(|root| root.structural_type)
                .ok_or_else(|| Error::malformed("content request names no exact carrier"))
        };
        let mut payload = state.clone();
        let payload_claims = payload.take(source)?;
        let output = projections
            .at_subject(
                crate::content_claims::Subject {
                    place: result.place,
                    carrier: result.structural_type,
                    qualifications: &result.qualifications,
                    projected_qualifications: &result.projected_qualifications,
                },
                payload_claims
                    .iter()
                    .map(|claim| (claim.identity, claim.path.as_slice())),
                ContentPlaceVersion::Current,
            )
            .map_err(Error::malformed)?;
        fn leaves<'a>(term: &'a ContentTerm, output: &mut Vec<&'a ContentTerm>) {
            match term {
                ContentTerm::Projection { .. } => output.push(term),
                ContentTerm::Separate(parts) => {
                    for part in parts {
                        leaves(part, output);
                    }
                }
            }
        }
        let mut parents = Vec::new();
        let mut bound = derivations.clone();
        for row in &mut bound {
            row.map_places(&mut |place| {
                if place == formal_result {
                    result.place
                } else {
                    place
                }
            });
            let mut terms = Vec::new();
            leaves(row.conservation.left(), &mut terms);
            leaves(row.conservation.right(), &mut terms);
            for term in terms {
                let ContentTerm::Projection {
                    projection,
                    subject,
                } = term
                else {
                    unreachable!()
                };
                if subject.version == ContentPlaceVersion::Entry {
                    let mut entries = self.entry_content.iter().filter(|entry| {
                        entry.input == *subject
                            && entry.projections.iter().any(|held| {
                                held.projection == *projection
                                    && held.algebra == *row.conservation.algebra()
                            })
                    });
                    let entry = entries.next().ok_or_else(|| {
                        Error::malformed("content law has no exact input projection obligation")
                    })?;
                    if entries.next().is_some() {
                        return Err(Error::malformed(
                            "content law has ambiguous input projection obligations",
                        ));
                    }
                    let claim = state.get(entry.claim).ok_or_else(|| {
                        Error::malformed("content law input claim is no longer live")
                    })?;
                    if !claim
                        .content
                        .contains(&terminal_psi::ClaimContentProjection {
                            projection: *projection,
                            algebra: row.conservation.algebra().clone(),
                        })
                    {
                        return Err(Error::malformed(
                            "content law input lost its projection obligation",
                        ));
                    }
                    let location = semantic_vocabulary::ContentStructuralPlace {
                        version: ContentPlaceVersion::Current,
                        root: claim.place,
                        segments: projections
                            .content_path(root_type(claim.place)?, &claim.path)
                            .map_err(Error::malformed)?,
                    };
                    parents.push(StructuralArgument {
                        place: claim.place,
                        path: claim.path.clone(),
                        access: StructuralAccess::Owned,
                    });
                    row.inputs.push(ContentClaimProjection {
                        claim: claim.identity,
                        projection: *projection,
                        location,
                        subject: subject.clone(),
                    });
                } else if subject.root == result.place {
                    let mut outputs = output.iter().filter(|entry| {
                        entry.input == *subject
                            && entry.projections.iter().any(|held| {
                                held.projection == *projection
                                    && held.algebra == *row.conservation.algebra()
                            })
                    });
                    let entry = outputs.next().ok_or_else(|| {
                        Error::malformed("content law output is not a requested result projection")
                    })?;
                    if outputs.next().is_some() {
                        return Err(Error::malformed(
                            "content law output has ambiguous payload custody",
                        ));
                    }
                    row.outputs.push(ContentClaimProjection {
                        claim: entry.claim,
                        projection: *projection,
                        location: subject.clone(),
                        subject: subject.clone(),
                    });
                } else {
                    return Err(Error::unsupported(
                        "content law needs an exact entry input or current result",
                    ));
                }
            }
            row.inputs.sort();
            row.outputs.sort();
            row.lineage_roots = row.inputs.iter().map(|binding| binding.claim).collect();
            row.lineage_roots.sort_unstable();
            row.lineage_roots.dedup();
        }
        parents.sort();
        parents.dedup();
        bound.sort();
        let mut resolved = operation.clone();
        resolved.kind = Op::TransformContent {
            source: source.clone(),
            parents,
            derivations: bound,
        };
        if settled {
            let position = self
                .plan
                .operation_results
                .binary_search_by_key(&operation.id, |row| row.operation)
                .map_err(|_| Error::malformed("content request has no operation result row"))?;
            self.plan.operation_results[position].transformation = Some(resolved.kind.clone());
        }
        Ok(Some(resolved))
    }

    fn transform_content(
        &self,
        state: &mut ClaimsState,
        operation: &Operation,
        root_type: &impl Fn(PlaceId) -> Result<StructuralTypeId, Error>,
    ) -> Result<(), Error> {
        let Op::TransformContent {
            source,
            parents,
            derivations,
        } = &operation.kind
        else {
            return Err(Error::malformed(
                "content replay received another operation",
            ));
        };
        let result = operation
            .result
            .as_structural()
            .ok_or_else(|| Error::malformed("content transformation has no structural result"))?;
        let projections = self.projections.as_ref().ok_or_else(|| {
            Error::unsupported("content transformation requires its exact module")
        })?;
        let mut consumed = Vec::new();
        for parent in parents {
            let mut claims = state.take(parent)?;
            if claims.is_empty() || claims.iter().any(|claim| claim.content.is_empty()) {
                return Err(Error::unsupported(
                    "content parent has an independent obligation outside its content roster",
                ));
            }
            // take() makes paths relative; a custody equation names
            // the actual occurrence, including its selected prefix.
            for claim in &mut claims {
                let mut path = parent.path.clone();
                path.append(&mut claim.path);
                claim.path = path;
            }
            consumed.extend(claims);
        }
        self.check_content_roster(&consumed, derivations, true, root_type)?;
        Self::relocate(state, source, result.place, &[])?;
        let output = projections
            .at_subject(
                crate::content_claims::Subject {
                    place: result.place,
                    carrier: result.structural_type,
                    qualifications: &result.qualifications,
                    projected_qualifications: &result.projected_qualifications,
                },
                state
                    .at(result.place)
                    .map(|claim| (claim.identity, claim.path.as_slice())),
                semantic_vocabulary::ContentPlaceVersion::Current,
            )
            .map_err(Error::malformed)?;
        for row in output {
            let slot = state
                .claim_ids
                .binary_search(&row.claim)
                .map_err(|_| Error::malformed("content output names an undeclared claim"))?;
            let claim = state.claims[slot]
                .as_mut()
                .ok_or_else(|| Error::malformed("content output names no live payload claim"))?;
            if claim
                .content
                .iter()
                .any(|held| !row.projections.contains(held))
            {
                return Err(Error::malformed(
                    "content transformation drops an inherited projection",
                ));
            }
            claim.content = row.projections;
        }
        let produced = state.at(result.place).cloned().collect::<Vec<_>>();
        self.check_content_roster(&produced, derivations, false, root_type)?;
        Ok(())
    }

    pub fn check_decomposition(
        &self,
        state: &ClaimsState,
        subject: &StructuralArgument,
        root: &super::Root,
    ) -> Result<(), Error> {
        if root
            .memberships
            .additional_input
            .iter()
            .any(|row| crate::structural_paths_may_overlap(&row.path, &subject.path))
        {
            return Err(Error::unsupported(
                "decomposition cannot discharge additional input obligations",
            ));
        }
        let projections = self.projections.as_ref().ok_or_else(|| {
            Error::unsupported("qualified decomposition requires its exact module")
        })?;
        let expected = projections
            .decomposition(
                crate::content_claims::Subject {
                    place: subject.place,
                    carrier: root.structural_type,
                    qualifications: &root.memberships.declared.whole,
                    projected_qualifications: &root.memberships.declared.projected,
                },
                &subject.path,
            )
            .map_err(Error::unsupported)?;
        let mut retained = state
            .at(subject.place)
            .filter(|claim| claim.path == subject.path)
            .flat_map(|claim| claim.content.iter().cloned())
            .collect::<Vec<_>>();
        retained.sort();
        if retained != expected {
            return Err(Error::malformed(
                "decomposition does not retain its complete content obligations",
            ));
        }
        Ok(())
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
                for claim in self.take_declared(state, argument, types, roots)? {
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
                        content: self.fresh_content(operation.id, binding.claim),
                    })?;
                }
            }
        } else {
            match &operation.kind {
                Op::DecomposeStructuralValue { subject } => {
                    let selected = crate::runtime_structural_path_tip(
                        types.iter(),
                        root_type(subject.place)?,
                        &subject.path,
                    )
                    .ok_or_else(|| Error::malformed("decomposition has no exact claim carrier"))?;
                    // Only the declaration's own wrapper obligation ends here.
                    // Its component claims remain at their original paths.
                    if selected.owns_linear_claim {
                        if state.at(subject.place).any(|claim| {
                            subject.path.starts_with(&claim.path) && claim.path != subject.path
                        }) {
                            return Err(Error::unsupported(
                                "decomposition crosses an intact logical claim",
                            ));
                        }
                        if state
                            .at(subject.place)
                            .filter(|claim| claim.path == subject.path)
                            .count()
                            != 1
                        {
                            return Err(Error::malformed(
                                "decomposition has no unique intrinsic wrapper claim",
                            ));
                        }
                        // Ending the payload wrapper cannot end its independently
                        // conserved content. Its exact claim stays until a checked
                        // transformation consumes every retained projection.
                        state.take_at(subject.place, |claim| {
                            claim.path == subject.path && claim.content.is_empty()
                        });
                    }
                }
                Op::MoveStructuralValue { source } => {
                    if let Some(result) = operation.result.as_structural() {
                        Self::relocate(state, source, result.place, &[])?;
                    }
                }
                Op::TransformContent { .. } => {
                    self.transform_content(state, operation, &root_type)?;
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
                                content: self.fresh_content(operation.id, row.claim),
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
                        && !self.catalogues.paths(result.structural_type)?.is_empty()
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
        machine: &TerminalMachine,
        terminator: &Terminator,
        types: &[StructuralTypeDeclaration],
        roots: &[Option<super::Root>],
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
                for claim in self.take_declared(state, argument, types, roots)? {
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
            let returned =
                state.take(&StructuralArgument::whole(*source, StructuralAccess::Owned))?;
            if let Some(result) = machine.result.structural() {
                self.check_boundary_content(
                    crate::content_claims::Subject {
                        place: result.place,
                        carrier: result.structural_type,
                        qualifications: &result.qualifications,
                        projected_qualifications: &result.projected_qualifications,
                    },
                    &returned,
                )?;
            }
            let returned = returned
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
    pub fn cleanup<'action>(
        &mut self,
        state: &ClaimsState,
        edge: EdgeId,
        actions: impl IntoIterator<Item = &'action TerminalAffineCleanupAction>,
        settled: bool,
    ) -> Result<(), Error> {
        for (ordinal, action) in actions.into_iter().enumerate() {
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
    pub fn finish(mut self, machine: &TerminalMachine) -> Result<ClaimsPlan, Error> {
        self.verify_catalogues(machine)?;
        Ok(self.plan)
    }
    fn verify_catalogues(&mut self, machine: &TerminalMachine) -> Result<(), Error> {
        if let Some(result) = machine.result.structural() {
            let paths = self.catalogues.paths(result.structural_type)?;
            let returns = self
                .plan
                .returned_claims
                .iter()
                .filter_map(|row| row.bindings.as_deref());
            if !crate::result_claim_paths_match_returns(paths, returns) {
                return Err(Error::malformed(
                    "normal result claim paths differ from the guarded declared result type",
                ));
            }
        }
        Ok(())
    }
}

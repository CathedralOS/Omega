//! Activation-wide order follows actual CFG alternatives. Block parameter
//! bindings rename custody; they never reorder establishment or create grants.
//! A normal call can publish a fresh claim id for a home that is already the
//! newest live claim. That rename is the same grant. Release order is the
//! stack position of the home, after its admission case is selected.
//!
//! An admitted claim stays in its caller home across a parked call or a
//! nominal cleanup of some other carrier. That home is its storage role:
//! the place is not moved into the callee, and an unselected admission has
//! no role to retain. Cleanup of the claim's own carrier still needs an
//! explicit disposition.

use semantic_vocabulary::{
    ClaimId, MachineId, OperationId, PlaceId, StructuralCaseId, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind as O, StructuralAccess, StructuralArgument, StructuralPathSegment as P,
    StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    root: PlaceId,
    path: Vec<P>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Live {
    site: OperationId,
    bound: u64,
    place: Place,
    claim: ClaimId,
    pending: Option<(Place, StructuralCaseId)>,
    /// Pending admissions and the active outcome of a split or merge count.
    /// The inactive outcome stays in the state until case selection drops it,
    /// and must not double the parent's demand.
    counts_demand: bool,
    split_origin: Option<OperationId>,
    /// 0 is a root claim, 1 the lower child, 2 the upper child.
    split_role: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Loan {
    site: OperationId,
    place: Place,
    origin: OperationId,
    parents: Vec<OperationId>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    live: Vec<Live>,
    loans: Vec<Loan>,
}

impl Place {
    fn argument(argument: &StructuralArgument) -> Self {
        Self {
            root: argument.place,
            path: argument.path.clone(),
        }
    }
    fn under(&self, parent: &Self) -> bool {
        self.root == parent.root && self.path.starts_with(&parent.path)
    }
    fn relocated(&self, source: &Self, destination: &Self) -> Self {
        let mut path = destination.path.clone();
        path.extend_from_slice(&self.path[source.path.len()..]);
        Self {
            root: destination.root,
            path,
        }
    }
}

impl State {
    fn validate_paths(
        &self,
        types: &[terminal_psi::StructuralTypeDeclaration],
        roots: &BTreeMap<PlaceId, StructuralTypeId>,
        claim_types: &BTreeSet<StructuralTypeId>,
    ) -> Result<(), &'static str> {
        let tip = |place: &Place| -> Result<StructuralTypeId, &'static str> {
            // Carriers live in finite owned storage. Referent unfolding needs
            // separate backing-origin evidence and is outside this serial
            // realization. An acyclic inline path uses at most one field/index
            // and one case segment per type. The derived catalog bound also
            // makes this helper total on malformed caller inputs.
            if place.path.len() > types.len().saturating_mul(2)
                || place
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, P::Field(_) | P::Case(_) | P::FixedIndex(_)))
            {
                return Err("activation transport requires a finite inline carrier path");
            }
            let root = roots
                .get(&place.root)
                .ok_or("activation carrier has no typed root declaration")?;
            crate::runtime_structural_path_tip(types.iter(), *root, &place.path)
                .map(|row| row.id)
                .ok_or("activation transport has an invalid typed carrier path")
        };
        for live in &self.live {
            let mut id = tip(&live.place)?;
            let mut seen = BTreeSet::new();
            while !claim_types.contains(&id) {
                if !seen.insert(id) {
                    return Err("activation carrier alias contains a cycle");
                }
                id = terminal_psi::find_structural_type(types, id)
                    .and_then(|row| row.quotient_carrier)
                    .ok_or("activation custody no longer names its exact claim carrier")?;
            }
            if let Some((subject, _)) = &live.pending {
                tip(subject)?;
            }
        }
        for loan in &self.loans {
            tip(&loan.place)?;
        }
        Ok(())
    }
    fn select(&mut self, place: &Place, case: StructuralCaseId, positive: bool) {
        self.live.retain_mut(|live| {
            let Some((subject, established)) = &live.pending else {
                return true;
            };
            if subject != place {
                return true;
            }
            let active = (*established == case) == positive;
            if active {
                live.pending = None;
                live.counts_demand = true;
            }
            active
        });
    }
    fn sites(&self, place: &Place) -> BTreeSet<OperationId> {
        self.live
            .iter()
            .filter(|live| live.place.under(place) || place.under(&live.place))
            .map(|live| live.site)
            .chain(
                self.loans
                    .iter()
                    .filter(|loan| loan.place.under(place) || place.under(&loan.place))
                    .map(|loan| loan.site),
            )
            .collect()
    }
    fn assemble(&mut self, source: &Place, destination: &Place) {
        // An unrestricted field can retain its original shared descriptor.
        // Keep that occurrence until ordinary loan/home cleanup retires it.
        let originals = self.loans.clone();
        self.relocate(source, destination);
        self.loans.extend(
            originals
                .into_iter()
                .filter(|loan| loan.place.under(source)),
        );
    }
    fn copy_loans(&mut self, source: &Place, destination: &Place) {
        let copies: Vec<_> = self
            .loans
            .iter()
            .filter(|loan| loan.place.under(source))
            .map(|loan| Loan {
                site: loan.site,
                place: loan.place.relocated(source, destination),
                origin: loan.origin,
                parents: loan.parents.clone(),
            })
            .collect();
        self.loans.extend(copies);
    }
    fn relocate(&mut self, source: &Place, destination: &Place) {
        for live in &mut self.live {
            if live.place.under(source) {
                live.place = live.place.relocated(source, destination);
            }
            if let Some((subject, _)) = &mut live.pending
                && subject.under(source)
            {
                *subject = subject.relocated(source, destination);
            }
        }
        for loan in &mut self.loans {
            if loan.place.under(source) {
                loan.place = loan.place.relocated(source, destination);
            }
        }
    }
    // Lexical descriptor retirement is not restoration of its resource (loans.md).
    // Descendants retain the complete ancestor roster even after intermediate
    // carriers disappear. Explicit ReleaseReference still closes deepest-first.
    fn parents(&self, source: &Place) -> Vec<OperationId> {
        self.loans
            .iter()
            .filter(|loan| loan.place.under(source) || source.under(&loan.place))
            .flat_map(|loan| std::iter::once(loan.origin).chain(loan.parents.iter().copied()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn formation_is_live(&self, origin: OperationId) -> bool {
        self.loans
            .iter()
            .any(|loan| loan.origin == origin || loan.parents.contains(&origin))
    }
    fn retire_carriers(&mut self, place: &Place) {
        self.loans.retain(|loan| !loan.place.under(place));
    }
    fn close_loans(&mut self, place: &Place) -> Result<(), &'static str> {
        let ended: BTreeSet<_> = self
            .loans
            .iter()
            .filter(|loan| loan.place.under(place))
            .map(|loan| loan.origin)
            .filter(|origin| {
                !self
                    .loans
                    .iter()
                    .any(|loan| loan.origin == *origin && !loan.place.under(place))
            })
            .collect();
        if self
            .loans
            .iter()
            .filter(|loan| !loan.place.under(place))
            .any(|loan| loan.parents.iter().any(|parent| ended.contains(parent)))
        {
            return Err("activation parent loan cannot end before its derived child loan");
        }
        self.retire_carriers(place);
        Ok(())
    }
    fn rebind_claims(
        &mut self,
        claims: &[terminal_psi::StructuralResultClaimBinding],
        root: PlaceId,
    ) {
        for live in &mut self.live {
            if live.place.root == root
                && let Some(binding) = claims
                    .iter()
                    .find(|binding| binding.path == live.place.path)
            {
                live.claim = binding.claim;
            }
        }
    }
}

fn contains_claim(
    types: &[terminal_psi::StructuralTypeDeclaration],
    ty: StructuralTypeId,
    claim_types: &BTreeSet<StructuralTypeId>,
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    if visiting.contains(&ty) {
        return false;
    }
    let Some(row) = terminal_psi::find_structural_type(types, ty) else {
        return false;
    };
    if claim_types.contains(&ty) {
        return true;
    }
    visiting.push(ty);
    let fields = |fields: &[terminal_psi::StructuralFieldDeclaration], visiting: &mut Vec<_>| {
        fields.iter().any(|field| {
            field
                .field_type
                .as_structural()
                .is_some_and(|ty| contains_claim(types, ty, claim_types, visiting))
        })
    };
    let result = row
        .quotient_carrier
        .is_some_and(|carrier| contains_claim(types, carrier, claim_types, visiting))
        || match &row.shape {
            StructuralTypeShape::Record {
                fields: members, ..
            } => fields(members, visiting),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().any(|case| fields(&case.fields, visiting))
            }
            StructuralTypeShape::Mixed {
                fields: members,
                cases,
            } => {
                fields(members, visiting) || cases.iter().any(|case| fields(&case.fields, visiting))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                *length != 0 && contains_claim(types, *element, claim_types, visiting)
            }
            _ => false,
        };
    visiting.pop();
    result
}

fn field_segment(
    types: &[terminal_psi::StructuralTypeDeclaration],
    id: semantic_vocabulary::StructuralFieldId,
) -> Result<P, &'static str> {
    let mut identity = None;
    for ty in types {
        let (common, cases): (
            &[terminal_psi::StructuralFieldDeclaration],
            &[terminal_psi::StructuralCaseDeclaration],
        ) = match &ty.shape {
            StructuralTypeShape::Record { fields, .. } => (fields, &[]),
            StructuralTypeShape::Sum { cases } => (&[], cases),
            StructuralTypeShape::Mixed { fields, cases } => (fields, cases),
            _ => continue,
        };
        for field in common
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
            .filter(|field| field.id == id)
        {
            if identity.is_some_and(|found| found != field.identity) {
                return Err("activation field identity is ambiguous");
            }
            identity = Some(field.identity);
        }
    }
    identity
        .map(P::Field)
        .ok_or("activation transport field is absent")
}

fn call_arguments(kind: &O) -> Option<&[StructuralArgument]> {
    match kind {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | O::CallDynamic {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => Some(structural_arguments),
        _ => None,
    }
}

/// Validate the bounded serial realization and compute its co-live bound.
/// Different CFG alternatives remain alternatives; sequential conditions never
/// become fabricated mutually exclusive allocation branches.
pub fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<u64, &'static str> {
    // Establishment rows, not a descriptor's geometry, identify the types
    // whose custody belongs to this module's activation grants.
    let claim_types = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, O::EstablishActivationClaim { .. }))
        .map(|operation| {
            super::activation_claim_shape(module, operation).map(|shape| shape.claim_type)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    // Parking retains the caller's admitted homes. The callee, including an
    // unknown or dynamic target, does not receive those homes. An admission
    // that is still pending has no selected storage role, so it cannot cross.
    let known: BTreeSet<_> = module.machines.iter().map(|machine| machine.id).collect();
    let mut suspending = BTreeSet::new();
    let may_suspend = |operation: &terminal_psi::Operation, machines: &BTreeSet<_>| {
        operation.suspension_crossing.is_some()
            || matches!(
                operation.kind,
                O::BoundaryCall { .. } | O::CallDynamic { .. }
            )
            || operation
                .kind
                .direct_callee()
                .is_some_and(|callee| !known.contains(&callee) || machines.contains(&callee))
    };
    loop {
        let additions: Vec<_> = module
            .machines
            .iter()
            .filter(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| may_suspend(operation, &suspending))
                    || machine.blocks.iter().any(|block| {
                        block.terminator.cleanup_rosters().flat_map(|(_, actions)| actions).any(|action| {
                            matches!(action, terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                                if !known.contains(&cleanup.cleanup_machine) || suspending.contains(&cleanup.cleanup_machine))
                        })
                    })
            })
            .map(|machine| machine.id)
            .collect();
        let previous = suspending.len();
        suspending.extend(additions);
        if previous == suspending.len() {
            break;
        }
    }
    let crossings: BTreeSet<_> = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| may_suspend(operation, &suspending))
        .map(|operation| operation.id)
        .collect();
    validate_flow(
        &module.structural_types,
        &machine.structural_parameters,
        &machine.block_entry_claims,
        machine.entry,
        &machine.blocks,
        &claim_types,
        &|operation| crossings.contains(&operation),
        &|callee| !known.contains(&callee) || suspending.contains(&callee),
    )
}

/// Replay current executable flow independently of its producing representation.
/// Only storage types and current blocks participate; retained source snapshots
/// cannot stand in for an optimizer's actual operation order or successor graph.
pub fn validate_flow(
    types: &[terminal_psi::StructuralTypeDeclaration],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    block_entry_claims: &[terminal_psi::EntryClaim],
    entry: semantic_vocabulary::BlockId,
    blocks: &[terminal_psi::Block],
    claim_types: &BTreeSet<StructuralTypeId>,
    may_suspend: &dyn Fn(OperationId) -> bool,
    machine_may_suspend: &dyn Fn(MachineId) -> bool,
) -> Result<u64, &'static str> {
    if structural_parameters.iter().any(|parameter| {
        parameter.access == StructuralAccess::Owned
            && contains_claim(
                types,
                parameter.structural_type,
                claim_types,
                &mut Vec::new(),
            )
    }) {
        return Err("an owned activation claim cannot enter another activation");
    }
    let mut roots = BTreeMap::new();
    for (place, ty) in structural_parameters
        .iter()
        .map(|parameter| (parameter.place, parameter.structural_type))
        .chain(blocks.iter().flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(|parameter| (parameter.place, parameter.structural_type))
        }))
        .chain(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.structural())
                .map(|result| (result.place, result.structural_type)),
        )
    {
        if roots
            .insert(place, ty)
            .is_some_and(|previous| previous != ty)
        {
            return Err("activation carrier root has inconsistent structural types");
        }
    }
    let mut work = vec![(entry, State::default())];
    let mut seen = BTreeSet::new();
    let mut maximum = 0;
    let observations: BTreeMap<_, _> = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|op| {
            let O::StructuralCaseMembership { source, path, case } = &op.kind else {
                return None;
            };
            Some((
                op.result.scalar()?.id,
                (
                    Place {
                        root: *source,
                        path: path.clone(),
                    },
                    *case,
                ),
            ))
        })
        .collect();
    while let Some((block, mut state)) = work.pop() {
        state.validate_paths(types, &roots, claim_types)?;
        if !seen.insert((block, state.clone())) {
            continue;
        }
        let block = blocks
            .iter()
            .find(|row| row.id == block)
            .ok_or("activation control target is absent")?;
        for operation in &block.operations {
            // The caller's home stays put. Only an admission that has not
            // selected its case lacks a storage role at this crossing.
            if (operation.suspension_crossing.is_some() || may_suspend(operation.id))
                && state.live.iter().any(|live| live.pending.is_some())
            {
                return Err("live activation claims at suspension exceed this serial realization");
            }
            if let Some(arguments) = call_arguments(&operation.kind) {
                if operation.result.structural().is_some()
                    && arguments.iter().any(|argument| {
                        let from = Place::argument(argument);
                        state.live.iter().any(|live| live.place.under(&from))
                            || state.loans.iter().any(|loan| loan.place.under(&from))
                    })
                {
                    return Err(
                        "activation-derived structural call result needs a retained origin mapping",
                    );
                }
                if arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned
                        && state
                            .live
                            .iter()
                            .any(|live| live.place.under(&Place::argument(argument)))
                }) {
                    return Err(
                        "an owned activation claim cannot transfer to another activation or provider",
                    );
                }
            }
            match &operation.kind {
                O::EstablishActivationClaim {
                    bound,
                    established_case,
                    ..
                } => {
                    super::activation_claim_shape_in(types, operation)?;
                    if state.live.iter().any(|live| live.site == operation.id) {
                        return Err(
                            "simultaneous live instances of one activation site exceed this serial realization",
                        );
                    }
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation result is missing")?;
                    let [binding] = result.claims.as_slice() else {
                        return Err("activation result has no unique claim");
                    };
                    state.live.push(Live {
                        site: operation.id,
                        bound: *bound,
                        claim: binding.claim,
                        place: Place {
                            root: result.place,
                            path: binding.path.clone(),
                        },
                        pending: Some((
                            Place {
                                root: result.place,
                                path: Vec::new(),
                            },
                            *established_case,
                        )),
                        counts_demand: true,
                        split_origin: None,
                        split_role: 0,
                    });
                }
                O::SplitActivationClaim {
                    source,
                    bound_at,
                    covered_case,
                    rejected_case,
                    ..
                } => {
                    let place = Place::argument(source);
                    let position = state
                        .live
                        .iter()
                        .position(|live| live.place == place && live.pending.is_none())
                        .ok_or("activation claim split has no established parent")?;
                    let parent = state.live.remove(position);
                    if state.loans.iter().any(|loan| loan.site == parent.site) {
                        return Err(
                            "activation claim cannot split while a captured loan remains live",
                        );
                    }
                    if *bound_at == 0 || *bound_at >= parent.bound {
                        return Err("activation claim split does not cover its parent");
                    }
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation split result is missing")?;
                    let root = Place {
                        root: result.place,
                        path: Vec::new(),
                    };
                    let binding = |role: u8| {
                        result.claims.iter().find(|binding| {
                            let covered = binding.path.iter().any(
                                |segment| matches!(segment, P::Case(case) if case == covered_case),
                            );
                            match role {
                                1 | 2 => covered,
                                _ => !covered,
                            }
                        })
                    };
                    let Some(lower_binding) = binding(1) else {
                        return Err("activation split result has no lower child");
                    };
                    let Some(upper_binding) = result.claims.iter().find(|candidate| {
                        candidate.claim != lower_binding.claim
                            && candidate.path.iter().any(
                                |segment| matches!(segment, P::Case(case) if case == covered_case),
                            )
                    }) else {
                        return Err("activation split result has no upper child");
                    };
                    let Some(restored) = result.claims.iter().find(|candidate| {
                        candidate.path.iter().any(
                            |segment| matches!(segment, P::Case(case) if case == rejected_case),
                        )
                    }) else {
                        return Err("activation split result has no restored parent");
                    };
                    let child =
                        |bound, binding: &terminal_psi::StructuralResultClaimBinding, role| Live {
                            site: parent.site,
                            bound,
                            claim: binding.claim,
                            place: Place {
                                root: result.place,
                                path: binding.path.clone(),
                            },
                            pending: Some((root.clone(), *covered_case)),
                            counts_demand: true,
                            split_origin: Some(operation.id),
                            split_role: role,
                        };
                    state.live.push(child(*bound_at, lower_binding, 1));
                    state
                        .live
                        .push(child(parent.bound - *bound_at, upper_binding, 2));
                    state.live.push(Live {
                        site: parent.site,
                        bound: parent.bound,
                        claim: restored.claim,
                        place: Place {
                            root: result.place,
                            path: restored.path.clone(),
                        },
                        pending: Some((root, *rejected_case)),
                        counts_demand: false,
                        split_origin: parent.split_origin,
                        split_role: parent.split_role,
                    });
                }
                O::MergeActivationClaim {
                    lower,
                    upper,
                    covered_case,
                    rejected_case,
                    ..
                } => {
                    let lower_place = Place::argument(lower);
                    let upper_place = Place::argument(upper);
                    if state.live.len() < 2 {
                        return Err("activation claim merge is not that split");
                    }
                    let upper_live = state.live.pop().expect("upper child");
                    let lower_live = state.live.pop().expect("lower child");
                    if lower_live.place != lower_place
                        || upper_live.place != upper_place
                        || lower_live.pending.is_some()
                        || upper_live.pending.is_some()
                        || lower_live.split_role != 1
                        || upper_live.split_role != 2
                        || lower_live.split_origin.is_none()
                        || lower_live.split_origin != upper_live.split_origin
                        || lower_live.bound + upper_live.bound == 0
                    {
                        return Err("activation claim merge is not that split");
                    }
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation merge result is missing")?;
                    let root = Place {
                        root: result.place,
                        path: Vec::new(),
                    };
                    let covered = result
                        .claims
                        .iter()
                        .find(|binding| {
                            binding.path.iter().any(
                                |segment| matches!(segment, P::Case(case) if case == covered_case),
                            )
                        })
                        .ok_or("activation merge result has no restored parent")?;
                    let rejected = result
                        .claims
                        .iter()
                        .filter(|binding| {
                            binding.path.iter().any(
                                |segment| matches!(segment, P::Case(case) if case == rejected_case),
                            )
                        })
                        .collect::<Vec<_>>();
                    let [rejected_lower, rejected_upper] = rejected.as_slice() else {
                        return Err("activation merge result has no returned children");
                    };
                    state.live.push(Live {
                        site: lower_live.site,
                        bound: lower_live.bound + upper_live.bound,
                        claim: covered.claim,
                        place: Place {
                            root: result.place,
                            path: covered.path.clone(),
                        },
                        pending: Some((root.clone(), *covered_case)),
                        counts_demand: true,
                        split_origin: None,
                        split_role: 0,
                    });
                    for (child, binding) in
                        [(&lower_live, rejected_lower), (&upper_live, rejected_upper)]
                    {
                        state.live.push(Live {
                            site: child.site,
                            bound: child.bound,
                            claim: binding.claim,
                            place: Place {
                                root: result.place,
                                path: binding.path.clone(),
                            },
                            pending: Some((root.clone(), *rejected_case)),
                            counts_demand: false,
                            split_origin: child.split_origin,
                            split_role: child.split_role,
                        });
                    }
                }
                O::ReleaseActivationClaim { source, claim } => {
                    let place = Place::argument(source);
                    let Some(live) = state.live.last() else {
                        return Err("activation release has no claim owned by this activation");
                    };
                    // The newest home is the sibling that must release next.
                    // Its admission case must already have been selected.
                    // Call completion and continuation specialization can
                    // spell a new claim id for that same home. An id that
                    // still names some other live claim is a different
                    // sibling, so it is not that rename.
                    let same_home = live.claim == *claim
                        || !state.live.iter().any(|other| other.claim == *claim);
                    if live.place != place || live.pending.is_some() || !same_home {
                        return Err(
                            "activation claims must release in reverse establishment order in their owning activation",
                        );
                    }
                    if state.loans.iter().any(|loan| loan.site == live.site) {
                        return Err(
                            "activation claim cannot release while a captured loan remains live",
                        );
                    }
                    state.live.pop();
                }
                O::EstablishRecord { fields } | O::EstablishStructuralCase { fields, .. } => {
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation aggregate lost its result")?;
                    for field in fields {
                        if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                            let mut path = Vec::new();
                            if let O::EstablishStructuralCase { result_case, .. } = operation.kind {
                                path.push(P::Case(result_case));
                            }
                            path.push(field_segment(types, field.field)?);
                            state.assemble(
                                &Place::argument(argument),
                                &Place {
                                    root: result.place,
                                    path,
                                },
                            );
                        }
                    }
                    state.rebind_claims(&result.claims, result.place);
                }
                O::EstablishStructuralArray { elements } => {
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation array lost its result")?;
                    for (index, argument) in elements.iter().enumerate() {
                        state.assemble(
                            &Place::argument(argument),
                            &Place {
                                root: result.place,
                                path: vec![P::FixedIndex(index as u64)],
                            },
                        );
                    }
                    state.rebind_claims(&result.claims, result.place);
                }
                O::MoveStructuralField {
                    source,
                    path,
                    field,
                } => {
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation field move lost its result")?;
                    let mut path = path.clone();
                    if let Some(field) = field {
                        path.push(field_segment(types, *field)?);
                    }
                    state.relocate(
                        &Place {
                            root: *source,
                            path,
                        },
                        &Place {
                            root: result.place,
                            path: Vec::new(),
                        },
                    );
                    state.rebind_claims(&result.claims, result.place);
                }
                O::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    if !state.sites(&Place::argument(value)).is_empty() {
                        return Err(
                            "activation-bearing store needs proof of activation-local destination storage",
                        );
                    }
                    let mut path = path.clone();
                    if let Some(field) = field {
                        path.push(field_segment(types, *field)?);
                    }
                    state.relocate(
                        &Place::argument(value),
                        &Place {
                            root: *destination,
                            path,
                        },
                    );
                }
                O::StructuralLeafCopy { source, path } => {
                    let from = Place {
                        root: *source,
                        path: path.clone(),
                    };
                    if state.live.iter().any(|live| live.place.under(&from)) {
                        return Err("activation claim cannot be copied");
                    }
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation loan copy lost its result")?;
                    state.copy_loans(
                        &from,
                        &Place {
                            root: result.place,
                            path: Vec::new(),
                        },
                    );
                }
                O::StructuralCaseLeafCopy { source, path } => {
                    let path = path
                        .iter()
                        .map(|segment| match segment {
                            semantic_vocabulary::CanonicalStructuralPathSegment::Referent => {
                                return Err(
                                    "activation claim case copy cannot cross reference storage",
                                );
                            }
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) => {
                                field_segment(types, *field)
                            }
                            semantic_vocabulary::CanonicalStructuralPathSegment::Case(case) => {
                                Ok(P::Case(*case))
                            }
                            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(
                                index,
                            ) => Ok(P::FixedIndex(*index)),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let from = Place {
                        root: *source,
                        path,
                    };
                    if state.live.iter().any(|live| live.place.under(&from)) {
                        return Err("activation claim cannot be copied");
                    }
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation loan copy lost its result")?;
                    state.copy_loans(
                        &from,
                        &Place {
                            root: result.place,
                            path: Vec::new(),
                        },
                    );
                }
                O::TransformContent {
                    source, parents, ..
                } => {
                    if std::iter::once(source).chain(parents).any(|argument| {
                        let from = Place::argument(argument);
                        state.live.iter().any(|live| live.place.under(&from))
                            || state.loans.iter().any(|loan| loan.place.under(&from))
                    }) {
                        return Err(
                            "activation-bearing content transformation exceeds this serial realization",
                        );
                    }
                }
                O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    let mut path = path.clone();
                    if let Some(field) = field {
                        path.push(field_segment(types, *field)?);
                    }
                    let destination = Place {
                        root: *destination,
                        path,
                    };
                    let source = Place {
                        root: *source,
                        path: Vec::new(),
                    };
                    let parents = state.parents(&source);
                    let captured: Vec<_> = state
                        .sites(&source)
                        .into_iter()
                        .map(|site| Loan {
                            site,
                            place: destination.clone(),
                            origin: operation.id,
                            parents: parents.clone(),
                        })
                        .collect();
                    if state.formation_is_live(operation.id) {
                        return Err(
                            "serial activation replay cannot conflate live occurrences of one loan formation",
                        );
                    }
                    state.retire_carriers(&destination);
                    state.loans.extend(captured);
                }
                O::MoveStructuralValue { source } => {
                    let result = operation
                        .result
                        .structural()
                        .ok_or("activation transport lost its result")?;
                    state.relocate(
                        &Place::argument(source),
                        &Place {
                            root: result.place,
                            path: Vec::new(),
                        },
                    );
                    state.rebind_claims(&result.claims, result.place);
                }
                O::ByteSequenceSubslice { source, .. }
                | O::ElementViewSubslice { source, .. }
                | O::EstablishReference { source, .. }
                | O::EstablishPlacedReference { source, .. }
                | O::EstablishElementView { source, .. }
                | O::EstablishMutableElementView { source, .. } => {
                    if state.formation_is_live(operation.id) {
                        return Err(
                            "serial activation replay cannot conflate live occurrences of one loan formation",
                        );
                    }
                    let parents = state.parents(&Place::argument(source));
                    for site in state.sites(&Place::argument(source)) {
                        let result = operation
                            .result
                            .structural()
                            .ok_or("activation loan lost its descriptor")?;
                        state.loans.push(Loan {
                            site,
                            place: Place {
                                root: result.place,
                                path: Vec::new(),
                            },
                            origin: operation.id,
                            parents: parents.clone(),
                        });
                    }
                }
                O::ReleaseReference { source, path } => {
                    let place = Place {
                        root: *source,
                        path: path.clone(),
                    };
                    state.close_loans(&place)?;
                }
                _ => {}
            }
            state.validate_paths(types, &roots, claim_types)?;
            if let Some(result) = operation.result.structural()
                && contains_claim(types, result.structural_type, claim_types, &mut Vec::new())
                && !matches!(
                    operation.kind,
                    O::EstablishActivationClaim { .. }
                        | O::SplitActivationClaim { .. }
                        | O::MergeActivationClaim { .. }
                        | O::EstablishRecord { .. }
                        | O::EstablishStructuralCase { .. }
                        | O::EstablishStructuralArray { .. }
                        | O::MoveStructuralValue { .. }
                        | O::MoveStructuralField { .. }
                        | O::EstablishStructuralLocal { .. }
                )
            {
                return Err(
                    "activation-bearing result has no authenticated establishment or owned transport",
                );
            }
            let branch = extents::ActivationClaimBranch::trunk();
            let demand = extents::compose_claim_bounds(
                state
                    .live
                    .iter()
                    .filter(|live| live.counts_demand)
                    .map(|live| extents::ClaimBoundRow {
                        bound: Some(live.bound),
                        alignment: 1,
                        branch: &branch,
                    }),
            )
            .map_err(|_| "activation co-live bound leaves closed stack demand open")?;
            maximum = maximum.max(demand);
        }
        let edges = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if edges.is_empty() && !state.live.is_empty() {
            return Err(
                "activation claims cannot escape or remain live when their activation unwinds",
            );
        }
        for (index, (edge, target)) in edges.into_iter().enumerate() {
            let arguments = arguments.get(index).copied().unwrap_or(&[]);
            let mut next = state.clone();
            match &block.terminator {
                Terminator::StructuralCase {
                    source,
                    path,
                    cases,
                } => {
                    let case = cases
                        .iter()
                        .find(|case| case.edge == edge)
                        .ok_or("activation case edge is absent")?;
                    next.select(
                        &Place {
                            root: *source,
                            path: path.clone(),
                        },
                        case.case,
                        true,
                    );
                }
                Terminator::Conditional {
                    condition,
                    when_true,
                    ..
                } => {
                    if let Some((place, case)) = observations.get(condition) {
                        next.select(place, *case, when_true.edge == edge);
                    }
                }
                _ => {}
            }
            let target_block = blocks
                .iter()
                .find(|block| block.id == target)
                .ok_or("activation target block is absent")?;
            let before = next.clone();
            let mut captured = Vec::new();
            for (argument, parameter) in arguments.iter().zip(&target_block.structural_parameters) {
                let source = Place::argument(argument);
                let destination = Place {
                    root: parameter.place,
                    path: Vec::new(),
                };
                captured.extend(
                    before
                        .loans
                        .iter()
                        .filter(|loan| loan.place.under(&source))
                        .map(|loan| Loan {
                            place: loan.place.relocated(&source, &destination),
                            ..loan.clone()
                        }),
                );
            }
            // Consuming transport retires only its old carrier. Captures retain
            // exact loan identities and parent relations across the edge.
            for (argument, parameter) in arguments.iter().zip(&target_block.structural_parameters) {
                if parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted {
                    next.retire_carriers(&Place::argument(argument));
                }
            }
            // Capture outgoing values first. Cleanup then retires old carrier
            // occurrences; the captured descriptors are rebound afterwards.
            let actions = block
                .terminator
                .cleanup_rosters()
                .find(|(candidate, _)| *candidate == edge)
                .map_or(&[][..], |(_, actions)| actions);
            for action in actions {
                use terminal_psi::TerminalAffineCleanupAction as C;
                let retired = match action {
                    C::RetireHome(root) | C::DiscardRoot(root) => Place {
                        root: *root,
                        path: Vec::new(),
                    },
                    C::DiscardResidual(discard) => Place {
                        root: discard.place,
                        path: discard.path.clone(),
                    },
                    C::InvokeNominal(cleanup) => {
                        // A parked hook keeps the caller's other admitted
                        // homes. Their places are those homes' storage roles.
                        // An unselected admission still cannot cross.
                        if machine_may_suspend(cleanup.cleanup_machine)
                            && next.live.iter().any(|live| live.pending.is_some())
                        {
                            return Err(
                                "activation claim cannot remain live across a possible suspension in nominal cleanup",
                            );
                        }
                        let root = Place {
                            root: cleanup.place,
                            path: Vec::new(),
                        };
                        if !next.sites(&root).is_empty() {
                            return Err(
                                "activation-bearing nominal cleanup needs explicit claim and loan disposition",
                            );
                        }
                        root
                    }
                };
                next.retire_carriers(&retired);
            }
            for (argument, parameter) in arguments.iter().zip(&target_block.structural_parameters) {
                // Sources are captured simultaneously, before names can alias
                // target homes. Apply every substitution to the captured state.
                let source = Place::argument(argument);
                let destination = Place {
                    root: parameter.place,
                    path: Vec::new(),
                };
                for (index, live) in before.live.iter().enumerate() {
                    if live.place.under(&source) {
                        next.live[index].place = live.place.relocated(&source, &destination);
                    }
                    if let Some((subject, case)) = &live.pending
                        && subject.under(&source)
                    {
                        next.live[index].pending =
                            Some((subject.relocated(&source, &destination), *case));
                    }
                }
            }
            next.loans.extend(captured);
            for live in &mut next.live {
                if let Some(binding) = block_entry_claims.iter().find(|binding| {
                    binding.input == live.place.root && binding.path == live.place.path
                }) {
                    live.claim = binding.claim;
                }
            }
            next.loans.sort();
            next.loans.dedup();
            next.validate_paths(types, &roots, claim_types)?;
            work.push((target, next));
        }
    }
    Ok(maximum)
}

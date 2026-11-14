/*
Semantic judgments for activation storage, arrival storage, borrowed storage projections, and
storage dependency tracking across `TerminalMachine` blocks.
*/

pub mod type_graph;

pub mod activation_storage {

    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralMultiplicity,
        StructuralPathSegment, StructuralTypeShape, TerminalModule,
    };

    #[derive(Debug, Clone, Copy)]
    pub struct ActivationClaimShape {
        pub claim_type: StructuralTypeId,
        pub claim_field: StructuralFieldId,
    }

    pub fn activation_claim_shape(
        module: &TerminalModule,
        operation: &Operation,
    ) -> Result<ActivationClaimShape, &'static str> {
        activation_claim_shape_in(&module.structural_types, operation)
    }

    pub fn activation_claim_shape_in(
        types: &[terminal_psi::StructuralTypeDeclaration],
        operation: &Operation,
    ) -> Result<ActivationClaimShape, &'static str> {
        let OperationKind::EstablishActivationClaim {
            bound,
            backing_type,
            established_case,
            rejected_case,
            ..
        } = operation.kind
        else {
            return Err("expected activation establishment");
        };
        let result = operation
            .result
            .structural()
            .ok_or("activation establishment requires a structural result")?;
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
        {
            return Err("activation attempt carries unexpected custody or qualifications");
        }
        let backing = terminal_psi::find_structural_type(types, backing_type)
            .ok_or("activation backing type is missing")?;
        if backing.owns_linear_claim
            || !backing.no_code_affine_cleanup
            || backing.descriptor_cleanup
            || backing.quotient_carrier.is_some()
            || backing.scalar_restriction.is_some()
            || backing.shape
                != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: bound,
                })
        {
            return Err("activation backing must reserve exactly the complete closed bound");
        }
        let attempt = terminal_psi::find_structural_type(types, result.structural_type)
            .ok_or("activation attempt type is missing")?;
        let StructuralTypeShape::Sum { cases } = &attempt.shape else {
            return Err("activation attempt is not a sum");
        };
        let [established, rejected] = cases.as_slice() else {
            return Err("activation attempt requires exactly two outcomes");
        };
        if established.id != established_case
            || rejected.id != rejected_case
            || established_case == rejected_case
            || !rejected.fields.is_empty()
            || attempt.owns_linear_claim
            || attempt.quotient_carrier.is_some()
        {
            return Err("activation attempt outcome roster differs from its operation");
        }
        let [field] = established.fields.as_slice() else {
            return Err("established outcome requires exactly one claim");
        };
        let claim_type = field
            .field_type
            .as_structural()
            .ok_or("activation claim payload is not structural")?;
        let claim = terminal_psi::find_structural_type(types, claim_type)
            .ok_or("activation claim type is missing")?;
        if field.relevance.is_erased()
            || !claim.owns_linear_claim
            || claim.no_code_affine_cleanup
            || claim.descriptor_cleanup
            || claim.quotient_carrier.is_some()
            || claim.scalar_restriction.is_some()
            || claim.shape
                != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                })
        {
            return Err("activation claim must be an opaque linear exclusive byte descriptor");
        }
        let [binding] = result.claims.as_slice() else {
            return Err("activation establishment requires one conditional claim binding");
        };
        if binding.path
            != [
                StructuralPathSegment::Case(established_case),
                StructuralPathSegment::Field(field.identity),
            ]
        {
            return Err("activation grant must occur only below its established outcome");
        }
        Ok(ActivationClaimShape {
            claim_type,
            claim_field: field.id,
        })
    }

    pub(crate) mod flow {

        use semantic_vocabulary::{
            ClaimId, MachineId, OperationId, PlaceId, StructuralCaseId, StructuralTypeId,
        };
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralResultClaimBinding;
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
            counts_demand: bool,
            split_origin: Option<OperationId>,
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
                    if place.path.len() > types.len().saturating_mul(2)
                        || place.path.iter().any(|segment| {
                            !matches!(segment, P::Field(_) | P::Case(_) | P::FixedIndex(_))
                        })
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
                let originals = self.loans.clone();
                self.relocate(source, destination);
                self.loans.extend(
                    originals
                        .into_iter()
                        .filter(|loan| loan.place.under(source)),
                );
            }
            fn copy_value(
                &mut self,
                source: &Place,
                destination: &Place,
            ) -> Result<(), &'static str> {
                let mut prefix = Place {
                    root: source.root,
                    path: Vec::with_capacity(source.path.len()),
                };
                let mut inline = true;
                for segment in &source.path {
                    if matches!(segment, P::Referent) && !self.sites(&prefix).is_empty() {
                        return Err(
                            "activation-bearing copy needs exact reference backing correspondence",
                        );
                    }
                    if inline {
                        match segment {
                            P::Field(_) | P::Case(_) | P::FixedIndex(_) => {
                                prefix.path.push(segment.clone());
                            }
                            _ => inline = false,
                        }
                    }
                }
                if self.live.iter().any(|live| live.place.under(source)) {
                    return Err("activation claim cannot be copied");
                }
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
                Ok(())
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
            fn parents(&self, source: &Place) -> Vec<OperationId> {
                self.loans
                    .iter()
                    .filter(|loan| loan.place.under(source) || source.under(&loan.place))
                    .flat_map(|loan| {
                        std::iter::once(loan.origin).chain(loan.parents.iter().copied())
                    })
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
            fn rebind_claims(&mut self, claims: &[StructuralResultClaimBinding], root: PlaceId) {
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
            let fields = |fields: &[terminal_psi::StructuralFieldDeclaration],
                          visiting: &mut Vec<_>| {
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
                        fields(members, visiting)
                            || cases.iter().any(|case| fields(&case.fields, visiting))
                    }
                    StructuralTypeShape::FixedArray { element, length } => {
                        *length != terminal_psi::ArrayExtent::Literal(0)
                            && contains_claim(types, *element, claim_types, visiting)
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

        fn referenced_subject(blocks: &[terminal_psi::Block], mut place: Place) -> Place {
            let formations = blocks
                .iter()
                .map(|block| block.operations.len())
                .sum::<usize>();
            for _ in 0..formations {
                let Some((P::Referent, rest)) = place.path.split_first() else {
                    break;
                };
                let source =
                    blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .find_map(|operation| match &operation.kind {
                            O::EstablishReference {
                                source,
                                storage: None,
                            } if operation
                                .result
                                .structural()
                                .is_some_and(|result| result.place == place.root) =>
                            {
                                Some(source)
                            }
                            _ => None,
                        });
                let Some(source) = source else {
                    break;
                };
                let mut path = source.path.clone();
                path.extend_from_slice(rest);
                place = Place {
                    root: source.place,
                    path,
                };
            }
            place
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

        pub fn validate_machine(
            module: &TerminalModule,
            machine: &TerminalMachine,
        ) -> Result<u64, &'static str> {
            ModuleActivation::new(module)?.validate_machine(machine)
        }

        pub fn validate_module(module: &TerminalModule) -> Result<(), &'static str> {
            let prepared = ModuleActivation::new(module)?;
            for machine in &module.machines {
                prepared.validate_machine(machine)?;
            }
            Ok(())
        }

        struct ModuleActivation<'module> {
            module: &'module TerminalModule,
            claim_types: BTreeSet<StructuralTypeId>,
            known: BTreeSet<MachineId>,
            suspending: BTreeSet<MachineId>,
        }

        fn may_suspend(
            operation: &terminal_psi::Operation,
            known: &BTreeSet<MachineId>,
            machines: &BTreeSet<MachineId>,
        ) -> bool {
            operation.suspension_crossing.is_some()
                || matches!(
                    operation.kind,
                    O::BoundaryCall { .. } | O::CallDynamic { .. }
                )
                || operation
                    .kind
                    .direct_callee()
                    .is_some_and(|callee| !known.contains(&callee) || machines.contains(&callee))
        }

        impl<'module> ModuleActivation<'module> {
            fn new(module: &'module TerminalModule) -> Result<Self, &'static str> {
                let claim_types = module
                    .machines
                    .iter()
                    .flat_map(|machine| &machine.blocks)
                    .flat_map(|block| &block.operations)
                    .filter(|operation| {
                        matches!(operation.kind, O::EstablishActivationClaim { .. })
                    })
                    .map(|operation| {
                        super::activation_claim_shape(module, operation)
                            .map(|shape| shape.claim_type)
                    })
                    .collect::<Result<BTreeSet<_>, _>>()?;
                let known: BTreeSet<_> = module.machines.iter().map(|machine| machine.id).collect();
                let mut suspending = BTreeSet::new();
                loop {
                    let additions: Vec<_> = module
                        .machines
                        .iter()
                        .filter(|machine| {
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .any(|operation| may_suspend(operation, &known, &suspending))
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
                Ok(Self {
                    module,
                    claim_types,
                    known,
                    suspending,
                })
            }

            fn validate_machine(&self, machine: &TerminalMachine) -> Result<u64, &'static str> {
                let crossings: BTreeSet<_> = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| may_suspend(operation, &self.known, &self.suspending))
                    .map(|operation| operation.id)
                    .collect();
                validate_flow(
                    &self.module.structural_types,
                    &machine.structural_parameters,
                    &machine.block_entry_claims,
                    machine.entry,
                    &machine.blocks,
                    &self.claim_types,
                    &|operation| crossings.contains(&operation),
                    &|callee| !self.known.contains(&callee) || self.suspending.contains(&callee),
                )
            }
        }
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
                            referenced_subject(
                                blocks,
                                Place {
                                    root: *source,
                                    path: path.clone(),
                                },
                            ),
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
                    if (operation.suspension_crossing.is_some() || may_suspend(operation.id))
                        && state.live.iter().any(|live| live.pending.is_some())
                    {
                        return Err(
                            "live activation claims at suspension exceed this serial realization",
                        );
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
                                |bound, binding: &StructuralResultClaimBinding, role| Live {
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
                                return Err(
                                    "activation release has no claim owned by this activation",
                                );
                            };
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
                        O::EstablishRecord { fields }
                        | O::EstablishStructuralCase { fields, .. } => {
                            let result = operation
                                .result
                                .structural()
                                .ok_or("activation aggregate lost its result")?;
                            for field in fields {
                                if let terminal_psi::RecordFieldValue::Structural(argument) =
                                    &field.value
                                {
                                    let mut path = Vec::new();
                                    if let O::EstablishStructuralCase { result_case, .. } =
                                        operation.kind
                                    {
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
                        O::StructuralLeafCopy { source, .. }
                        | O::StructuralCaseLeafCopy { source, .. } => {
                            let path = match &operation.kind {
                                O::StructuralLeafCopy { path, .. } => path.clone(),
                                O::StructuralCaseLeafCopy { path, .. } => {
                                    let root = roots
                                        .get(source)
                                        .copied()
                                        .ok_or("activation copy has no typed source root")?;
                                    crate::canonical_structural_storage_path(
                                        types.iter(),
                                        root,
                                        path,
                                    )
                                    .ok_or("activation copy has an invalid canonical source path")?
                                }
                                _ => return Err("activation copy lost its source path"),
                            };
                            let result = operation
                                .result
                                .structural()
                                .ok_or("activation loan copy lost its result")?;
                            state.copy_value(
                                &Place {
                                    root: *source,
                                    path,
                                },
                                &Place {
                                    root: result.place,
                                    path: Vec::new(),
                                },
                            )?;
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
                        | O::EstablishAccessReference { source, .. }
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
                        && contains_claim(
                            types,
                            result.structural_type,
                            claim_types,
                            &mut Vec::new(),
                        )
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
                                &referenced_subject(
                                    blocks,
                                    Place {
                                        root: *source,
                                        path: path.clone(),
                                    },
                                ),
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
                    for (argument, parameter) in
                        arguments.iter().zip(&target_block.structural_parameters)
                    {
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
                    for (argument, parameter) in
                        arguments.iter().zip(&target_block.structural_parameters)
                    {
                        if parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                        {
                            next.retire_carriers(&Place::argument(argument));
                        }
                    }
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
                    for (argument, parameter) in
                        arguments.iter().zip(&target_block.structural_parameters)
                    {
                        let source = Place::argument(argument);
                        let destination = Place {
                            root: parameter.place,
                            path: Vec::new(),
                        };
                        for (index, live) in before.live.iter().enumerate() {
                            if live.place.under(&source) {
                                next.live[index].place =
                                    live.place.relocated(&source, &destination);
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

        #[cfg(test)]
        mod batch_tests {
            use super::{validate_machine, validate_module};
            use semantic_vocabulary::{
                BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralCaseId,
                StructuralTypeId, ValueId,
            };
            use terminal_psi::{
                Block, Operation, OperationKind, OperationResult, StructuralAccess,
                StructuralMultiplicity, StructuralParameterDeclaration, TerminalMachine,
                TerminalMachineResult, TerminalModule, Terminator,
            };

            fn machine(id: u64) -> TerminalMachine {
                let block = BlockId::new(1).unwrap();
                TerminalMachine {
                    assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                    id: MachineId::new(id).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    ranked_scc: None,
                    structural_places: Vec::new(),
                    block_entry_claims: Vec::new(),
                    loan_completions: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: block,
                    blocks: vec![Block::plain(
                        block,
                        Vec::new(),
                        Terminator::ReturnUnit {
                            edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    )],
                    contract: terminal_psi::MachineContract {
                        declaration: None,
                        parameters: Vec::new(),
                        parameter_order: Vec::new(),
                        structural_parameters: Vec::new(),
                        result: TerminalMachineResult::Unit,
                        entry_claims: Vec::new(),
                        declared_service_reach: None,
                        closed_reach_application: None,
                        published_synchronous_entries: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        content_entry_claims: Vec::new(),
                        ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
                    },
                }
            }

            #[test]
            fn module_activation_batch_preserves_global_error_priority_and_machine_order() {
                let mut module = TerminalModule::declarations();
                assert_eq!(validate_module(&module), Ok(()));
                module.machines = vec![machine(1), machine(2)];
                assert_eq!(validate_module(&module), Ok(()));
                for machine in &module.machines {
                    assert_eq!(validate_machine(&module, machine), Ok(0));
                }

                module.machines[1].blocks.clear();
                assert_eq!(
                    validate_module(&module),
                    Err("activation control target is absent")
                );
                module.machines[1].id = module.machines[0].id;
                assert_eq!(
                    validate_module(&module),
                    Err("activation control target is absent")
                );
                let parameter = |ty| StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 0,
                    structural_type: StructuralTypeId::new(ty).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                };
                module.machines[0].structural_parameters = vec![parameter(1), parameter(2)];
                let roots = Err("activation carrier root has inconsistent structural types");
                let absent = Err("activation control target is absent");
                assert_eq!(validate_module(&module), roots);
                module.machines.swap(0, 1);
                assert_eq!(validate_module(&module), absent);

                module.machines[1].blocks[0]
                    .operations
                    .push(Operation::unsuspended(
                        OperationId::new(1).unwrap(),
                        OperationResult::Unit,
                        OperationKind::EstablishActivationClaim {
                            bound: 1,
                            committed: ValueId::new(1).unwrap(),
                            backing_type: StructuralTypeId::new(1).unwrap(),
                            established_case: StructuralCaseId::new(1).unwrap(),
                            rejected_case: StructuralCaseId::new(2).unwrap(),
                        },
                    ));
                let global = Err("activation establishment requires a structural result");
                assert_eq!(validate_module(&module), global);
                assert_eq!(
                    validate_machine(&module, &module.machines[0]).map(|_| ()),
                    global
                );
                module.machines[1].blocks[0].operations.clear();
                assert_eq!(validate_module(&module), absent);

                module.machines = vec![machine(1), machine(2)];
                assert_eq!(validate_module(&module), Ok(()));
                let mut external = machine(3);
                external.blocks.clear();
                assert_eq!(validate_machine(&module, &external).map(|_| ()), absent);
            }
        }
    }
    pub use flow::{validate_flow, validate_machine, validate_module};
}
pub(crate) mod arrival_storage {

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralTypeShape;
    use terminal_psi::{Block, StructuralParameterDeclaration, TerminalMachine};

    pub fn view_descriptor_projected_qualifications<'types>(
        declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
        root: semantic_vocabulary::StructuralTypeId,
        projected: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        if projected.is_empty() {
            return true;
        }
        let mut matching = declarations.filter(|row| row.id == root);
        let Some(declaration) = matching.next() else {
            return false;
        };
        matching.next().is_none()
            && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            && projected.iter().all(|row| row.is_element_membership())
    }

    pub fn view_descriptor_parameter<'types>(
        parameter: &StructuralParameterDeclaration,
        declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
    ) -> bool {
        if !matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || !matches!(
            parameter.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        ) {
            return false;
        }
        let mut matching =
            declarations.filter(|declaration| declaration.id == parameter.structural_type);
        let Some(declaration) = matching.next() else {
            return false;
        };
        matching.next().is_none()
            && (parameter.projected_qualifications.is_empty()
                || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    && parameter
                        .projected_qualifications
                        .iter()
                        .all(|row| row.is_element_membership()))
            && (declaration.shape.is_canonical_view_descriptor()
                || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && crate::descriptors::descriptor_results::descriptor_value_access(
                        std::iter::once(declaration),
                        parameter.structural_type,
                        parameter.multiplicity,
                    )
                    .is_some_and(|ceiling| parameter.access.admits_referent(ceiling)))
    }

    pub fn borrowed_storage_parameter<'types>(
        parameter: &StructuralParameterDeclaration,
        declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration>,
    ) -> bool {
        if parameter.access == terminal_psi::StructuralAccess::Owned
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        {
            return false;
        }
        let mut matching = declarations.filter(|row| row.id == parameter.structural_type);
        let Some(declaration) = matching.next() else {
            return false;
        };
        matching.next().is_none() && !declaration.shape.is_canonical_view_descriptor()
    }

    pub fn borrowed_storage_reference_ceiling<'types>(
        parameter: &StructuralParameterDeclaration,
        mut declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    ) -> Option<terminal_psi::StructuralAccess> {
        use terminal_psi::StructuralAccess;
        if !borrowed_storage_parameter(parameter, declarations.clone()) {
            return None;
        }
        let declaration = declarations.find(|row| row.id == parameter.structural_type)?;
        let inner = crate::stored_reference_access(&declaration.shape)?;
        match parameter.access {
            StructuralAccess::SharedBorrow if inner != StructuralAccess::WriteOnlyBorrow => {
                Some(StructuralAccess::SharedBorrow)
            }
            StructuralAccess::MutableBorrow => Some(inner),
            _ => None,
        }
    }

    pub fn descriptor_parameter_access<'types>(
        parameter: &StructuralParameterDeclaration,
        declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    ) -> Option<terminal_psi::StructuralAccess> {
        if parameter.access == terminal_psi::StructuralAccess::Owned {
            if !view_descriptor_projected_qualifications(
                declarations.clone(),
                parameter.structural_type,
                &parameter.projected_qualifications,
            ) {
                return None;
            }
            crate::descriptors::descriptor_results::descriptor_value_access(
                declarations,
                parameter.structural_type,
                parameter.multiplicity,
            )
        } else if view_descriptor_parameter(parameter, declarations.clone()) {
            Some(parameter.access)
        } else if declarations.clone().any(|row| {
            row.id == parameter.structural_type
                && crate::references::reference_views::descriptor_declared_access(&row.shape)
                    .is_some()
        }) {
            borrowed_storage_reference_ceiling(parameter, declarations)
        } else {
            None
        }
    }

    pub fn arrival_parameter(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        let place_declaration = exactly_one(machine.structural_places.iter(), |declaration| {
            declaration.id == place
        })?;

        match place_declaration.kind {
            StructuralPlaceKind::Parameter { position } => {
                let parameter = machine.structural_parameters.get(position as usize)?;
                (parameter.place == place
                    && parameter.position == position
                    && machine
                        .structural_parameters
                        .iter()
                        .filter(|candidate| candidate.place == place)
                        .count()
                        == 1
                    && machine.blocks.iter().all(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .all(|candidate| candidate.place != place)
                    }))
                .then_some(parameter)
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                if block == machine.entry
                    || machine
                        .structural_parameters
                        .iter()
                        .any(|candidate| candidate.place == place)
                {
                    return None;
                }
                let owner = exactly_one(machine.blocks.iter(), |candidate| candidate.id == block)?;
                let parameter = owner.structural_parameters.get(position as usize)?;
                (parameter.place == place
                    && parameter.position == position
                    && machine
                        .blocks
                        .iter()
                        .flat_map(|candidate| &candidate.structural_parameters)
                        .filter(|candidate| candidate.place == place)
                        .count()
                        == 1)
                    .then_some(parameter)
            }
            _ => None,
        }
    }

    pub fn root_before_edge(
        machine: &TerminalMachine,
        block: BlockId,
        predecessor: &Block,
        place: PlaceId,
    ) -> Option<PlaceId> {
        let projection = projection_before_edge(machine, block, predecessor, place)?;
        projection.path.is_empty().then_some(projection.root)
    }

    pub fn projection_before_edge<'edge>(
        machine: &TerminalMachine,
        block: BlockId,
        predecessor: &'edge Block,
        place: PlaceId,
    ) -> Option<terminal_psi::OperationProjection<'edge>> {
        let Some(position) = machine
            .structural_places
            .iter()
            .find_map(|row| match row.kind {
                StructuralPlaceKind::BlockParameter {
                    block: owner,
                    position,
                } if row.id == place && owner == block => Some(position),
                _ => None,
            })
        else {
            return Some(terminal_psi::OperationProjection {
                root: place,
                path: &[],
            });
        };
        let mut forwarded = None;
        for binding in crate::control_graph::forwarded_bindings(predecessor) {
            if binding.target != block {
                continue;
            }
            let argument = binding.structural_arguments.get(position as usize)?;
            let projection = terminal_psi::OperationProjection {
                root: argument.place,
                path: &argument.path,
            };
            if forwarded.is_some_and(|earlier| earlier != projection) {
                return None;
            }
            forwarded = Some(projection);
        }
        forwarded
    }

    pub struct StorageNames {
        behind: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    }

    impl StorageNames {
        pub fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
            self.behind.get(&block).into_iter().flatten().copied()
        }
    }

    pub fn storage_names(machine: &TerminalMachine, block: BlockId, root: PlaceId) -> StorageNames {
        let mut behind: BTreeMap<BlockId, BTreeSet<PlaceId>> = BTreeMap::new();
        let mut pending = vec![(block, root)];
        while let Some((block, root)) = pending.pop() {
            if !behind.entry(block).or_default().insert(root) {
                continue;
            }
            for predecessor in &machine.blocks {
                let reaches = crate::control_graph::reaches_by_case(predecessor, block)
                    || crate::control_graph::forwarded_bindings(predecessor)
                        .any(|binding| binding.target == block);
                if reaches && let Some(before) = root_before_edge(machine, block, predecessor, root)
                {
                    pending.push((predecessor.id, before));
                }
            }
        }
        StorageNames { behind }
    }
}
pub mod borrowed_storage {
    use crate::scalars::scalar_ordering::ScalarOrdering;
    use semantic_vocabulary::{BlockId, PlaceId, ValueId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralPathSegment;

    pub type Position = (BlockId, u32);
    pub type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Projection {
        address: Option<Vec<AddressStep>>,
        pub path: Vec<StructuralPathSegment>,
        pub exact: bool,
        pub indirect: bool,
        pub window: Option<(ValueId, ValueId)>,
        pub captured_at: Option<Position>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum AddressStep {
        Path(Vec<StructuralPathSegment>, Option<Position>),
        Window {
            start: ValueId,
            end: ValueId,
            at: Position,
        },
    }

    impl Projection {
        pub fn new(path: &[StructuralPathSegment]) -> Self {
            let length = path
                .iter()
                .position(|segment| {
                    !matches!(
                        segment,
                        StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                    )
                })
                .unwrap_or(path.len());
            let bounded_length = length
                + usize::from(matches!(
                    path.get(length),
                    Some(
                        StructuralPathSegment::FixedRange { .. }
                            | StructuralPathSegment::RuntimeIndex { .. }
                    )
                ));
            Self {
                address: Some(if path.is_empty() {
                    Vec::new()
                } else {
                    vec![AddressStep::Path(path.to_vec(), None)]
                }),
                path: path[..bounded_length].to_vec(),
                exact: length == path.len(),
                indirect: crate::crosses_unaliased_reference(path),
                window: None,
                captured_at: None,
            }
        }

        pub fn at(mut self, position: Position) -> Self {
            if let Some(steps) = &mut self.address {
                for step in steps {
                    if let AddressStep::Path(path, captured @ None) = step
                        && path.iter().any(|segment| segment.runtime_index().is_some())
                    {
                        *captured = Some(position);
                    }
                }
            }
            if self.window.is_some()
                || self
                    .path
                    .iter()
                    .any(|segment| segment.runtime_index().is_some())
            {
                self.captured_at = Some(position);
            }
            self
        }

        pub fn window(start: ValueId, end: ValueId, position: Position) -> Self {
            Self::new(&[]).with_window(start, end, position)
        }

        pub fn with_window(mut self, start: ValueId, end: ValueId, position: Position) -> Self {
            if let Some(steps) = &mut self.address {
                steps.push(AddressStep::Window {
                    start,
                    end,
                    at: position,
                });
            }
            self.exact = false;
            self.window = Some((start, end));
            self.captured_at = Some(position);
            self
        }

        pub fn without_runtime_coordinates(mut self) -> Self {
            if self.address.as_ref().is_some_and(|steps| {
                steps.iter().any(|step| match step {
                    AddressStep::Window { .. } => true,
                    AddressStep::Path(path, _) => {
                        path.iter().any(|segment| segment.runtime_index().is_some())
                    }
                })
            }) {
                self.address = None;
            }
            if let Some(position) = self
                .path
                .iter()
                .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
            {
                self.path.truncate(position);
                self.exact = false;
            }
            self.window = None;
            self.captured_at = None;
            self
        }

        pub fn enclosing_storage(&self) -> Option<Self> {
            if self.indirect {
                return None;
            }
            let length = self
                .path
                .iter()
                .take_while(|segment| {
                    matches!(
                        segment,
                        StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                    )
                })
                .count();
            Some(Self::new(&self.path[..length]))
        }

        pub fn without_captured_address(mut self) -> Self {
            self.address = None;
            self
        }

        pub fn forget_address(&mut self) {
            self.exact = false;
            self.address = None;
        }

        pub fn fixed_address(
            &self,
            ordering: &ScalarOrdering,
        ) -> Option<Vec<StructuralPathSegment>> {
            self.reference_origin_path(ordering)?
                .into_iter()
                .map(|projection| match projection {
                    terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment),
                    terminal_psi::ReferenceOriginProjection::Element => None,
                })
                .collect()
        }

        pub fn reference_origin_path(
            &self,
            ordering: &ScalarOrdering,
        ) -> Option<Vec<terminal_psi::ReferenceOriginProjection>> {
            use terminal_psi::ReferenceOriginProjection::{Element, Exact};
            if self.indirect {
                return None;
            }
            let mut result = Vec::new();
            let mut offset: Option<Option<u64>> = None;
            let append_offset = |offset: &mut Option<Option<u64>>, start: Option<u64>| {
                *offset = Some(match (offset.unwrap_or(Some(0)), start) {
                    (Some(left), Some(right)) => Some(left.checked_add(right)?),
                    _ => None,
                });
                Some(())
            };
            for step in self.address.as_ref()? {
                match step {
                    AddressStep::Window { start, at, .. } => {
                        append_offset(&mut offset, ordering.known_unsigned(*at, *start))?
                    }
                    AddressStep::Path(path, at) => {
                        for segment in path {
                            match segment {
                                StructuralPathSegment::FixedRange { start, .. } => {
                                    append_offset(&mut offset, Some(*start))?
                                }
                                StructuralPathSegment::FixedIndex(_)
                                | StructuralPathSegment::RuntimeIndex { .. } => {
                                    let index = match segment {
                                        StructuralPathSegment::FixedIndex(index) => Some(*index),
                                        StructuralPathSegment::RuntimeIndex { index, .. } => {
                                            ordering.known_unsigned((*at)?, *index)
                                        }
                                        _ => unreachable!(),
                                    };
                                    result.push(match (offset.take().unwrap_or(Some(0)), index) {
                                        (Some(start), Some(index)) => {
                                            Exact(StructuralPathSegment::FixedIndex(
                                                start.checked_add(index)?,
                                            ))
                                        }
                                        _ => Element,
                                    });
                                }
                                StructuralPathSegment::Field(_)
                                | StructuralPathSegment::Case(_)
                                    if offset.is_none() =>
                                {
                                    result.push(Exact(segment.clone()))
                                }
                                _ => return None,
                            }
                        }
                    }
                }
            }
            offset.is_none().then_some(result)
        }

        pub fn then(&self, suffix: &Self) -> Self {
            let mut composed = self.clone();
            composed.address =
                self.address
                    .as_ref()
                    .zip(suffix.address.as_ref())
                    .and_then(|(prefix, suffix)| {
                        let work = prefix.iter().chain(suffix).try_fold(0usize, |used, step| {
                            used.checked_add(match step {
                                AddressStep::Path(path, _) => path.len(),
                                AddressStep::Window { .. } => 1,
                            })
                        })?;
                        if work > 4096
                            || suffix.iter().any(|step| {
                                prefix.iter().any(|prior| match (prior, step) {
                                    (
                                        AddressStep::Window { at: before, .. },
                                        AddressStep::Window { at, .. },
                                    ) => before == at,
                                    (
                                        AddressStep::Path(previous, Some(before)),
                                        AddressStep::Path(path, Some(at)),
                                    ) if before == at => path
                                        .iter()
                                        .filter_map(StructuralPathSegment::runtime_index)
                                        .any(|selector| {
                                            previous.iter().any(|segment| {
                                                segment.runtime_index() == Some(selector)
                                            })
                                        }),
                                    _ => false,
                                })
                            })
                        {
                            return None;
                        }
                        let mut steps: Vec<AddressStep> = Vec::new();
                        for step in prefix.iter().chain(suffix) {
                            if let AddressStep::Path(path, at) = step
                                && let Some(AddressStep::Path(previous, captured)) =
                                    steps.last_mut()
                                && (captured == at
                                    || captured.is_none()
                                        && !previous
                                            .iter()
                                            .any(|segment| segment.runtime_index().is_some())
                                    || at.is_none()
                                        && !path
                                            .iter()
                                            .any(|segment| segment.runtime_index().is_some()))
                            {
                                previous.extend_from_slice(path);
                                *captured = (*captured).or(*at);
                            } else {
                                steps.push(step.clone());
                            }
                        }
                        Some(steps)
                    });
            composed.indirect |= suffix.indirect;
            if composed.exact {
                composed.path.extend_from_slice(&suffix.path);
                composed.exact = suffix.exact;
                composed.window = suffix.window;
                composed.captured_at = suffix.captured_at;
            }
            composed
        }
    }

    #[derive(Clone)]
    pub struct Region {
        pub root: Option<PlaceId>,
        pub projection: Projection,
    }

    impl Region {
        pub fn overlaps(&self, other: &Self) -> bool {
            self.projection.indirect
                || other.projection.indirect
                || self.root.is_none()
                || other.root.is_none()
                || (self.root == other.root
                    && crate::structural_paths_may_overlap(
                        &self.projection.path,
                        &other.projection.path,
                    ))
        }
    }

    pub fn regions_overlap_at(
        left: &Region,
        right: &Region,
        position: Position,
        relations: &ScalarOrdering,
    ) -> bool {
        if !left.overlaps(right) {
            return false;
        }
        if left.projection.indirect
            || right.projection.indirect
            || left.root.is_none()
            || left.root != right.root
        {
            return true;
        }
        !window_excludes_index(&left.projection, &right.projection, relations, position)
            && !window_excludes_index(&right.projection, &left.projection, relations, position)
    }

    fn window_excludes_index(
        window: &Projection,
        selected: &Projection,
        relations: &ScalarOrdering,
        position: Position,
    ) -> bool {
        let (Some((start, end)), Some(birth)) = (window.window, window.captured_at) else {
            return false;
        };
        use crate::scalars::scalar_ordering::Term;
        let index = match selected.path.get(window.path.len()) {
            Some(StructuralPathSegment::FixedIndex(index)) => Term::Unsigned(*index),
            Some(StructuralPathSegment::RuntimeIndex { index, .. })
                if selected.captured_at.is_some_and(|captured| {
                    relations.values_unchanged_since(captured, position, &[*index])
                }) =>
            {
                Term::Value(*index)
            }
            _ => return false,
        };
        if selected.path[..window.path.len()] != window.path
            || !relations.values_unchanged_since(birth, position, &[start, end])
        {
            return false;
        }
        relations.proves(position, index, Term::Value(start), true)
            || relations.proves(position, Term::Value(end), index, false)
    }

    pub fn resolve(aliases: &AliasSources, root: PlaceId, projection: Projection) -> Vec<Region> {
        let mut result = Vec::new();
        let mut pending = vec![(root, projection, BTreeSet::new())];
        let mut expanded = BTreeSet::new();
        while let Some((place, projection, mut visited)) = pending.pop() {
            if !visited.insert(place) {
                continue;
            }
            let key = (place, projection.clone());
            if !expanded.insert(key) {
                continue;
            }
            match aliases.get(&place) {
                Some(sources) => {
                    for (source, prefix) in sources {
                        pending.push((*source, prefix.then(&projection), visited.clone()));
                    }
                }
                None => result.push(Region {
                    root: Some(place),
                    projection,
                }),
            }
        }
        if result.is_empty() {
            result.push(Region {
                root: None,
                projection: Projection::new(&[]),
            });
        }
        result
    }
}
pub(crate) mod calling_storage {

    use semantic_vocabulary::{CallingSignatureId, ScalarApplicationId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralTypeShape, TerminalModule};

    pub struct ForeignRecordStorage<'module> {
        pub storage: Option<semantic_vocabulary::AuthoredRecordStorage>,
        fields: Option<&'module [terminal_psi::TargetLayoutFieldStorage]>,
    }

    impl ForeignRecordStorage<'_> {
        pub fn field(
            &self,
            field: &terminal_psi::StructuralFieldDeclaration,
        ) -> Result<Option<semantic_vocabulary::AuthoredFieldStorage>, String> {
            match self.fields {
                Some(fields) => Ok(Some(
                    fields
                        .iter()
                        .find(|candidate| candidate.field == field.id)
                        .ok_or("target boundary layout omitted a live field")?
                        .storage
                        .clone(),
                )),
                None => Ok(field.storage.clone()),
            }
        }
    }

    pub fn foreign_record_storage<'module>(
        module: &'module TerminalModule,
        record: StructuralTypeId,
        storage: terminal_psi::StructuralRecordStorage,
        target: &str,
    ) -> Result<ForeignRecordStorage<'module>, String> {
        use terminal_psi::StructuralRecordStorage;
        let selected = match storage {
            StructuralRecordStorage::TargetLayout(application) => {
                let application = module
                    .target_layout_applications
                    .iter()
                    .find(|candidate| candidate.id == application && candidate.record == record)
                    .ok_or("boundary record has no exact target layout application")?;
                Some(
                    application
                        .realizations
                        .iter()
                        .find(|row| row.target == target)
                        .ok_or("boundary record has no layout for its selected target")?,
                )
            }
            _ => None,
        };
        Ok(ForeignRecordStorage {
            storage: match storage {
                StructuralRecordStorage::Packed => None,
                StructuralRecordStorage::Authored(storage) => Some(storage),
                StructuralRecordStorage::TargetLayout(_) => selected.map(|row| row.storage),
                StructuralRecordStorage::AccessView(_) | StructuralRecordStorage::Accessor(_) => {
                    return Err("placed data has no ordinary owned boundary storage".into());
                }
            },
            fields: selected.map(|row| row.fields.as_slice()),
        })
    }

    pub fn foreign_opaque_carrier<'module>(
        module: &'module TerminalModule,
        declaration: &terminal_psi::StructuralTypeDeclaration,
    ) -> Result<Option<(usize, &'module terminal_psi::TerminalOpaqueRepresentation)>, String> {
        let selected = module
            .opaque_representations
            .iter()
            .enumerate()
            .find(|(_, row)| row.opaque == declaration.id);
        if selected.is_none() && declaration.quotient_carrier.is_some() {
            return Err(
                "nominally opaque boundary value requires retained selected-carrier custody".into(),
            );
        }
        Ok(selected)
    }

    pub fn foreign_storage_extent_dependencies(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = StructuralTypeId>,
        target: &str,
    ) -> Result<Vec<ScalarApplicationId>, String> {
        fn visit(
            module: &TerminalModule,
            subject: StructuralTypeId,
            target: &str,
            active: &mut Vec<StructuralTypeId>,
            closed: &mut Vec<StructuralTypeId>,
            applications: &mut Vec<ScalarApplicationId>,
        ) -> Result<(), String> {
            if closed.contains(&subject) {
                return Ok(());
            }
            if active.contains(&subject) {
                return Err("recursive by-value Terminal data has no finite boundary shape".into());
            }
            let declaration = module
                .structural_type(subject)
                .ok_or("boundary value has no canonical storage declaration")?;
            active.push(subject);
            let mut children = Vec::new();
            if let Some((_, selected)) = foreign_opaque_carrier(module, declaration)? {
                children.push(selected.carrier);
            } else {
                match &declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::AtomicCell(_)
                    | StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. } => {}
                    StructuralTypeShape::FixedArray { element, length } => {
                        if let terminal_psi::ArrayExtent::Application(application) = length {
                            applications.push(*application);
                        }
                        children.push(*element);
                    }
                    StructuralTypeShape::Record { fields, storage } => {
                        let selected = foreign_record_storage(module, subject, *storage, target)?;
                        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                            let storage = selected.field(field)?;
                            if storage
                                .as_ref()
                                .and_then(|storage| storage.stored_scalar())
                                .is_none()
                                && let terminal_psi::StructuralFieldType::Structural(child) =
                                    field.field_type
                            {
                                children.push(child);
                            }
                        }
                    }
                    StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => {
                        return Err("case data is not yet classifiable as a boundary value; pass it by reference".into());
                    }
                }
            }
            for child in children {
                visit(module, child, target, active, closed, applications)?;
            }
            active.pop();
            closed.push(subject);
            Ok(())
        }
        let mut applications = Vec::new();
        let mut active = Vec::new();
        let mut closed = Vec::new();
        for root in roots {
            visit(
                module,
                root,
                target,
                &mut active,
                &mut closed,
                &mut applications,
            )?;
        }
        applications.sort_unstable();
        applications.dedup();
        Ok(applications)
    }

    pub fn calling_abi_extent_dependencies(
        module: &TerminalModule,
        declaration: CallingSignatureId,
        target: &str,
    ) -> Result<Vec<ScalarApplicationId>, String> {
        let storage = module
            .calling_abi_signature(declaration)
            .ok_or("calling extent query lost its declaration")?
            .storage
            .as_ref()
            .map_err(Clone::clone)?;
        let mut roots = Vec::new();
        roots.extend(storage.result);
        for parameter in &storage.parameters {
            if parameter.access == StructuralAccess::Owned {
                roots.push(parameter.structural_type);
            }
            if let Some(backing) = parameter.callback_backing
                && callback_storage_has_slots(module, target, backing)?
            {
                roots.push(backing);
            }
        }
        foreign_storage_extent_dependencies(module, roots, target)
    }

    pub fn callback_storage_has_slots(
        module: &TerminalModule,
        target: &str,
        record: StructuralTypeId,
    ) -> Result<bool, String> {
        storage_has_slots(module, target, record, &mut Vec::new())
    }

    fn storage_has_slots(
        module: &TerminalModule,
        target: &str,
        record: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> Result<bool, String> {
        if active.len() >= 128 || active.contains(&record) {
            return Err("canonical callback storage has a recursive inline record path".into());
        }
        let declaration = module
            .structural_type(record)
            .ok_or("callback backing root has no retained storage declaration")?;
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return Ok(false);
        };
        let native = module
            .native_layouts
            .record(record)
            .ok_or("callback backing record lost its native layout association")?;
        if module
            .native_layouts
            .for_target(native, target)?
            .is_some_and(|row| !row.report.private_callback_demands.is_empty())
        {
            return Ok(true);
        }
        active.push(record);
        let result = (|| {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if let Some(child) = field.field_type.as_structural() {
                    let child_declaration = module
                        .structural_type(child)
                        .ok_or("callback inline field has no retained storage declaration")?;
                    if matches!(child_declaration.shape, StructuralTypeShape::Record { .. })
                        && storage_has_slots(module, target, child, active)?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        active.pop();
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            AuthoredFieldStorage, AuthoredRecordStorage, ScalarType, StructuralFieldId,
        };
        use terminal_psi::{
            ArrayExtent, StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
            StructuralTypeDeclaration, TargetLayoutApplication, TargetLayoutFieldStorage,
            TargetLayoutRealization, TargetLayoutSchemaArgument, TerminalIdentityIndex,
        };

        fn identity(value: u64) -> StructuralTypeId {
            StructuralTypeId::new(value).unwrap()
        }

        fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                requires: Vec::new(),
                id: identity(value),
                identity: TerminalIdentityIndex::ABSENT,
                shape,
                quotient_carrier: None,
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
            }
        }

        fn array_module() -> TerminalModule {
            let mut module = TerminalModule::declarations();
            module.structural_types = vec![
                declaration(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
                declaration(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: identity(1),
                        length: ArrayExtent::Application(ScalarApplicationId::new(1).unwrap()),
                    },
                ),
            ];
            module
        }

        #[test]
        fn finite_storage_depth_is_not_a_new_foreign_dependency_limit() {
            let mut module = array_module();
            for value in 3..=131 {
                module.structural_types.push(declaration(
                    value,
                    StructuralTypeShape::FixedArray {
                        element: identity(value - 1),
                        length: ArrayExtent::Literal(1),
                    },
                ));
            }
            assert_eq!(
                foreign_storage_extent_dependencies(&module, [identity(131)], "macos_arm64")
                    .unwrap(),
                [ScalarApplicationId::new(1).unwrap()]
            );
        }

        #[test]
        fn selected_opaque_carrier_owes_its_actual_array_extent() {
            use language_semantics::opaque_representations::*;
            let mut module = array_module();
            module.structural_types.push(declaration(
                3,
                StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: StructuralRecordStorage::Packed,
                },
            ));
            let encoding = b"dependency-query-selection".to_vec();
            let (report, conformance) =
                language_semantics::conformance_identity::source_identity_commitments(&encoding);
            let lifecycle = OpaqueRepresentationLifecycleDisposition::Inert;
            let copy = OpaqueRepresentationCopyDisposition::PlacementOnly;
            let origin = OpaqueRepresentationApplicationOrigin::NamedConformance;
            module
                .opaque_representations
                .push(terminal_psi::TerminalOpaqueRepresentation {
                    opaque: identity(3),
                    carrier: identity(2),
                    opaque_storage_identity: TerminalIdentityIndex::ABSENT,
                    carrier_storage_identity: TerminalIdentityIndex::ABSENT,
                    application: SelectedOpaqueApplication {
                        opaque_identity: "Opaque".into(),
                        carrier_identity: "Array".into(),
                        conformance_identity: "Selected".into(),
                        application_report_fingerprint: report,
                        conformance_application_commitment: conformance,
                        identity_encoding: encoding,
                        schema_version: OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
                        origin,
                        lifecycle,
                        copy_disposition: copy,
                        selected_application_commitment: selected_application_commitment(
                            conformance,
                            lifecycle,
                            copy,
                            origin,
                        ),
                    },
                });
            assert_eq!(
                foreign_storage_extent_dependencies(&module, [identity(3)], "macos_arm64").unwrap(),
                [ScalarApplicationId::new(1).unwrap()]
            );
        }

        #[test]
        fn target_stored_scalar_does_not_demand_the_logical_child_extent() {
            let mut module = array_module();
            let field = StructuralFieldId::new(1).unwrap();
            let application = terminal_psi::LayoutApplicationId(1);
            module.structural_types.push(declaration(
                3,
                StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: TerminalIdentityIndex::ABSENT,
                        inline_carrier: None,
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(identity(2)),
                        storage: None,
                    }],
                    storage: StructuralRecordStorage::TargetLayout(application),
                },
            ));
            module
                .target_layout_applications
                .push(TargetLayoutApplication {
                    id: application,
                    record: identity(3),
                    schema: identity(3),
                    policy_identity: TerminalIdentityIndex::ABSENT,
                    plan_identity: TerminalIdentityIndex::ABSENT,
                    target_inputs: Vec::new(),
                    realizations: vec![TargetLayoutRealization {
                        schema_argument: TargetLayoutSchemaArgument {
                            abi_version: 4,
                            fields: Vec::new(),
                            retired_field_identities: Vec::new(),
                        },
                        target: "macos_arm64".into(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                        storage: AuthoredRecordStorage {
                            byte_size: 1,
                            byte_alignment: 1,
                        },
                        fields: vec![TargetLayoutFieldStorage {
                            field,
                            storage: AuthoredFieldStorage::Scalar {
                                byte_offset: 0,
                                stored_scalar: Some(ScalarType::Boolean),
                            },
                        }],
                    }],
                });
            assert!(
                foreign_storage_extent_dependencies(&module, [identity(3)], "macos_arm64")
                    .unwrap()
                    .is_empty()
            );
            assert!(
                foreign_storage_extent_dependencies(&module, [identity(3)], "linux_x86_64")
                    .is_err()
            );
        }
    }
}
pub(crate) mod collection_backing {

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CollectionStorage {
        Inline,
        Borrowed,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CollectionExtent {
        Fixed(u64),
        LiveLength { capacity: u64 },
        StoredLength,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CollectionBacking {
        pub extent: CollectionExtent,
        pub storage: CollectionStorage,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CollectionViewDescriptor {
        StoredLength,
        DeclaredLength(u64),
    }

    pub fn collection_view_descriptor(
        shape: &StructuralTypeShape,
        extent: Option<CollectionExtent>,
        region: terminal_psi::ElementViewRegion,
        source_path: &[terminal_psi::StructuralPathSegment],
        element: Option<&StructuralTypeDeclaration>,
    ) -> Option<CollectionViewDescriptor> {
        match shape {
            StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }) => {
                Some(CollectionViewDescriptor::StoredLength)
            }
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity,
                ..
            }) if extent == Some(CollectionExtent::Fixed(*capacity))
                && element.is_some_and(|declaration| {
                    declaration.scalar_restriction.is_none()
                        && matches!(declaration.shape,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                            if integer.sign() == IntegerSign::Unsigned
                                && integer.bits() == 8 && !integer.is_address())
                })
                && region == terminal_psi::ElementViewRegion::WholeCollection
                && !matches!(
                    source_path.last(),
                    Some(terminal_psi::StructuralPathSegment::FixedRange { .. })
                ) =>
            {
                Some(CollectionViewDescriptor::DeclaredLength(*capacity))
            }
            _ => None,
        }
    }

    pub fn collection_backing<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
        element: StructuralTypeId,
    ) -> Option<CollectionBacking> {
        let declaration = crate::structural_declaration(types.clone(), collection)?;
        collection_shape_backing(types, &declaration.shape, element)
    }

    fn collection_shape_backing<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        shape: &StructuralTypeShape,
        element: StructuralTypeId,
    ) -> Option<CollectionBacking> {
        match *shape {
            StructuralTypeShape::ElementView {
                element: actual, ..
            } if actual == element => Some(CollectionBacking {
                extent: CollectionExtent::StoredLength,
                storage: CollectionStorage::Borrowed,
            }),
            StructuralTypeShape::FixedArray {
                element: actual,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if actual == element => Some(CollectionBacking {
                extent: CollectionExtent::Fixed(length),
                storage: CollectionStorage::Inline,
            }),
            StructuralTypeShape::ByteSequence(carrier)
                if types.clone().any(|declaration| {
                    declaration.id == element
                        && matches!(declaration.shape,
                                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                    if integer.sign() == IntegerSign::Unsigned
                                        && integer.bits() == 8 && !integer.is_address())
                }) =>
            {
                Some(CollectionBacking {
                    extent: match carrier {
                        ByteSequenceCarrier::BoundedOwned { capacity } => {
                            CollectionExtent::LiveLength { capacity }
                        }
                        ByteSequenceCarrier::BoundedView { capacity, .. } => {
                            CollectionExtent::Fixed(capacity)
                        }
                        ByteSequenceCarrier::BorrowedView { .. } => CollectionExtent::StoredLength,
                    },
                    storage: match carrier {
                        ByteSequenceCarrier::BoundedOwned { .. } => CollectionStorage::Inline,
                        _ => CollectionStorage::Borrowed,
                    },
                })
            }
            _ => None,
        }
    }

    pub fn collection_view_backing<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
        element: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Option<CollectionBacking> {
        let declaration = exactly_one(types.clone(), |row| row.id == collection)?;
        collection_shape_view_backing(types, &declaration.shape, element, access)
    }

    pub(crate) fn collection_shape_view_backing<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        shape: &StructuralTypeShape,
        element: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Option<CollectionBacking> {
        if let Some(backing) = collection_shape_backing(types.clone(), shape, element) {
            return Some(backing);
        }
        let StructuralTypeShape::FixedArray {
            element: source,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = *shape
        else {
            return None;
        };
        crate::element_representation_relation(types, source, element, access)?;
        Some(CollectionBacking {
            extent: CollectionExtent::Fixed(length),
            storage: CollectionStorage::Inline,
        })
    }

    pub fn collection_view_preserves_coordinates<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
        element: StructuralTypeId,
        region: terminal_psi::ElementViewRegion,
    ) -> bool {
        matches!(region, terminal_psi::ElementViewRegion::WholeCollection)
            && collection_backing(types, collection, element).is_some()
    }
}
pub(crate) mod element_representation {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ElementRepresentationLeaf {
        pub path: Vec<StructuralPathSegment>,
        pub offset: u64,
        pub scalar_type: ScalarType,
        pub bounds: Option<BoundedIntegerType>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ElementRepresentationLayout {
        pub size: u64,
        pub alignment: u64,
        pub leaves: Vec<ElementRepresentationLeaf>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ElementRepresentationRelation {
        pub source_layout: ElementRepresentationLayout,
        pub target_layout: ElementRepresentationLayout,
    }

    pub(crate) fn scalar_size(scalar: ScalarType) -> Option<u64> {
        match scalar {
            ScalarType::Boolean => Some(1),
            ScalarType::Integer(integer)
                if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
            {
                Some(u64::from(integer.bits() / 8))
            }
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
            _ => None,
        }
    }
    fn align(value: u64, alignment: u64) -> Option<u64> {
        value
            .checked_add(alignment.checked_sub(1)?)
            .map(|value| value / alignment * alignment)
    }
    fn scalar_layout(
        scalar: ScalarType,
        bounds: Option<BoundedIntegerType>,
    ) -> Option<ElementRepresentationLayout> {
        let size = scalar_size(scalar)?;
        Some(ElementRepresentationLayout {
            size,
            alignment: size,
            leaves: vec![ElementRepresentationLeaf {
                path: Vec::new(),
                offset: 0,
                scalar_type: scalar,
                bounds,
            }],
        })
    }
    pub fn element_representation_layout<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        element: StructuralTypeId,
    ) -> Option<ElementRepresentationLayout> {
        fn visit<'a>(
            types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
            element: StructuralTypeId,
            active: &mut Vec<StructuralTypeId>,
            budget: &mut usize,
        ) -> Option<ElementRepresentationLayout> {
            *budget = budget.checked_sub(1)?;
            if active.len() >= 128 || active.contains(&element) {
                return None;
            }
            let row = exactly_one(types.clone(), |row| row.id == element)?;
            active.push(element);
            let mut layout = match &row.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => scalar_layout(*scalar, None)?,
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    let child = visit(types.clone(), *element, active, budget)?;
                    let stride = align(child.size, child.alignment)?;
                    let count = usize::try_from(*length)
                        .ok()?
                        .checked_mul(child.leaves.len())?;
                    *budget = budget.checked_sub(count)?;
                    let mut leaves = Vec::new();
                    leaves.try_reserve_exact(count).ok()?;
                    for index in 0..*length {
                        for leaf in &child.leaves {
                            let mut path = vec![StructuralPathSegment::FixedIndex(index)];
                            path.extend_from_slice(&leaf.path);
                            leaves.push(ElementRepresentationLeaf {
                                path,
                                offset: stride.checked_mul(index)?.checked_add(leaf.offset)?,
                                scalar_type: leaf.scalar_type,
                                bounds: leaf.bounds,
                            });
                        }
                    }
                    ElementRepresentationLayout {
                        size: stride.checked_mul(*length)?,
                        alignment: child.alignment,
                        leaves,
                    }
                }
                StructuralTypeShape::Record { fields, .. } => {
                    let (size, alignment) =
                        crate::reference_storage_layout(types.clone(), element)?;
                    let mut leaves = Vec::new();
                    for field in fields {
                        if field.relevance.is_erased() {
                            return None;
                        }
                        let child = match &field.field_type {
                            StructuralFieldType::Scalar(scalar) => scalar_layout(*scalar, None)?,
                            StructuralFieldType::BoundedInteger(bounds)
                            | StructuralFieldType::AtomicBoundedInteger(bounds) => scalar_layout(
                                ScalarType::Integer(bounds.integer_type()),
                                Some(*bounds),
                            )?,
                            StructuralFieldType::IeeeFloat(format) => {
                                scalar_layout(ScalarType::IeeeFloat(*format), None)?
                            }
                            StructuralFieldType::Structural(child) => {
                                visit(types.clone(), *child, active, budget)?
                            }
                            _ => return None,
                        };
                        *budget = budget.checked_sub(child.leaves.len())?;
                        leaves.try_reserve(child.leaves.len()).ok()?;
                        for leaf in child.leaves {
                            let mut path = vec![StructuralPathSegment::Field(field.identity)];
                            path.extend(leaf.path);
                            leaves.push(ElementRepresentationLeaf {
                                path,
                                offset: leaf.offset,
                                scalar_type: leaf.scalar_type,
                                bounds: leaf.bounds,
                            });
                        }
                    }
                    ElementRepresentationLayout {
                        size,
                        alignment,
                        leaves,
                    }
                }
                _ => return None,
            };
            let (size, alignment) = crate::reference_storage_layout(types.clone(), element)?;
            for leaf in &mut layout.leaves {
                let projection =
                    crate::reference_storage_projection(types.clone(), element, &leaf.path)?;
                if projection.logical_scalar != leaf.scalar_type
                    || projection.stored_scalar != leaf.scalar_type
                {
                    return None;
                }
                leaf.offset = projection.byte_offset;
            }
            layout.size = size;
            layout.alignment = alignment;
            layout.leaves.sort_by_key(|leaf| leaf.offset);
            active.pop();
            (layout.size != 0 && !layout.leaves.is_empty()).then_some(layout)
        }
        visit(types, element, &mut Vec::new(), &mut 16384)
    }
    fn bit_intervals(leaf: &ElementRepresentationLeaf) -> Option<Vec<(u64, u64)>> {
        let bits = scalar_size(leaf.scalar_type)?.checked_mul(8)?;
        let mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let Some(bounds) = leaf.bounds else {
            return Some(vec![(
                0,
                if leaf.scalar_type == ScalarType::Boolean {
                    1
                } else {
                    mask
                },
            )]);
        };
        if leaf.scalar_type != ScalarType::Integer(bounds.integer_type()) {
            return None;
        }
        let mut intervals = match (bounds.minimum(), bounds.maximum()) {
            (IntegerValue::Unsigned(minimum), IntegerValue::Unsigned(maximum)) => {
                vec![(u64::try_from(minimum).ok()?, u64::try_from(maximum).ok()?)]
            }
            (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) => {
                let minimum = i64::try_from(minimum).ok()?;
                let maximum = i64::try_from(maximum).ok()?;
                if minimum < 0 && maximum >= 0 {
                    vec![(0, maximum as u64), ((minimum as u64) & mask, mask)]
                } else {
                    vec![((minimum as u64) & mask, (maximum as u64) & mask)]
                }
            }
            _ => return None,
        };
        if intervals.len() == 2 && intervals[0].1.saturating_add(1) >= intervals[1].0 {
            intervals = vec![(intervals[0].0, intervals[1].1)];
        }
        Some(intervals)
    }
    pub fn element_representation_relation<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        target: StructuralTypeId,
        access: StructuralAccess,
    ) -> Option<ElementRepresentationRelation> {
        if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
            return None;
        }
        let source_layout = element_representation_layout(types.clone(), source)?;
        let target_layout = element_representation_layout(types, target)?;
        if source_layout.size != target_layout.size
            || source_layout.alignment != target_layout.alignment
            || source_layout.leaves.len() != target_layout.leaves.len()
        {
            return None;
        }
        for (source, target) in source_layout.leaves.iter().zip(&target_layout.leaves) {
            if source.offset != target.offset
                || scalar_size(source.scalar_type)? != scalar_size(target.scalar_type)?
            {
                return None;
            }
            let source = bit_intervals(source)?;
            let target = bit_intervals(target)?;
            if !source.iter().all(|(low, high)| {
                target
                    .iter()
                    .any(|(other_low, other_high)| other_low <= low && high <= other_high)
            }) || (access == StructuralAccess::MutableBorrow && source != target)
            {
                return None;
            }
        }
        Some(ElementRepresentationRelation {
            source_layout,
            target_layout,
        })
    }
}
pub(crate) mod representation_regions {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        ByteSequenceCarrier, ElementViewRegion, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RepresentationScalarLeaf {
        pub path: Vec<StructuralPathSegment>,
        pub offset: u64,
        pub scalar_type: ScalarType,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RepresentationElementLayout {
        pub size: u64,
        pub alignment: u64,
        pub leaves: Vec<RepresentationScalarLeaf>,
    }

    pub fn representation_element_layout<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        element: StructuralTypeId,
    ) -> Option<RepresentationElementLayout> {
        let layout = crate::element_representation_layout(types, element)?;
        if layout
            .leaves
            .iter()
            .any(|leaf| leaf.bounds.is_some() || leaf.scalar_type == ScalarType::Boolean)
        {
            return None;
        }
        Some(RepresentationElementLayout {
            size: layout.size,
            alignment: layout.alignment,
            leaves: layout
                .leaves
                .into_iter()
                .map(|leaf| RepresentationScalarLeaf {
                    path: leaf.path,
                    offset: leaf.offset,
                    scalar_type: leaf.scalar_type,
                })
                .collect(),
        })
    }

    pub fn representation_byte_extent<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
    ) -> Option<u64> {
        let row = exactly_one(types.clone(), |row| row.id == collection)?;
        representation_shape_byte_extent(types, &row.shape)
    }

    fn representation_shape_byte_extent<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        shape: &StructuralTypeShape,
    ) -> Option<u64> {
        match shape {
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                let element = exactly_one(types, |row| row.id == *element)?;
                matches!(element.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) if !integer.is_address() && integer.sign() == IntegerSign::Unsigned && integer.bits()==8).then_some(*length)
            }
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity,
                ..
            }) => Some(*capacity),
            _ => None,
        }
    }

    #[derive(Debug, Clone)]
    pub enum ElementViewRegionGeometry {
        WholeCollection(crate::CollectionBacking),
        WholeRepresentation {
            backing: crate::CollectionBacking,
            source: RepresentationElementLayout,
            target: RepresentationElementLayout,
        },
        RepresentationBytes {
            byte_extent: u64,
            element: RepresentationElementLayout,
        },
    }
    pub fn element_view_region_geometry<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
        element: StructuralTypeId,
        region: ElementViewRegion,
        access: terminal_psi::StructuralAccess,
    ) -> Option<ElementViewRegionGeometry> {
        let declaration = exactly_one(types.clone(), |row| row.id == collection)?;
        element_view_shape_region_geometry(types, &declaration.shape, element, region, access)
    }

    pub fn element_view_shape_region_geometry<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        shape: &StructuralTypeShape,
        element: StructuralTypeId,
        region: ElementViewRegion,
        access: terminal_psi::StructuralAccess,
    ) -> Option<ElementViewRegionGeometry> {
        match region {
            ElementViewRegion::WholeCollection => {
                crate::storage::collection_backing::collection_shape_view_backing(
                    types, shape, element, access,
                )
                .map(ElementViewRegionGeometry::WholeCollection)
            }
            ElementViewRegion::WholeRepresentation { .. } => {
                if access != terminal_psi::StructuralAccess::SharedBorrow {
                    return None;
                }
                let source_element = match shape {
                    StructuralTypeShape::ElementView { element, .. }
                    | StructuralTypeShape::FixedArray { element, .. } => *element,
                    _ => return None,
                };
                let backing = crate::storage::collection_backing::collection_shape_view_backing(
                    types.clone(),
                    shape,
                    source_element,
                    access,
                )?;
                let source = representation_element_layout(types.clone(), source_element)?;
                let target = representation_element_layout(types, element)?;
                if source.size == 0
                    || target.size == 0
                    || !source.size.is_multiple_of(target.size)
                    || source.alignment < target.alignment
                    || !source.alignment.is_multiple_of(target.alignment)
                {
                    return None;
                }
                let tiled = |layout: &RepresentationElementLayout| {
                    let mut end = 0;
                    for leaf in &layout.leaves {
                        if leaf.offset != end {
                            return false;
                        }
                        let Some(width) =
                            crate::storage::element_representation::scalar_size(leaf.scalar_type)
                        else {
                            return false;
                        };
                        let Some(next) = end.checked_add(width) else {
                            return false;
                        };
                        end = next;
                    }
                    end == layout.size
                };
                if !tiled(&source) || !tiled(&target) {
                    return None;
                }
                Some(ElementViewRegionGeometry::WholeRepresentation {
                    backing,
                    source,
                    target,
                })
            }
            ElementViewRegion::RepresentationBytes { .. } => {
                Some(ElementViewRegionGeometry::RepresentationBytes {
                    byte_extent: representation_shape_byte_extent(types.clone(), shape)?,
                    element: representation_element_layout(types, element)?,
                })
            }
        }
    }
    fn word() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid")
    }
    fn literal(value: u64) -> ScalarTerm {
        ScalarTerm::integer(word(), IntegerValue::Unsigned(u128::from(value))).expect("u64 literal")
    }
    fn value(value: ValueId) -> ScalarTerm {
        ScalarTerm::value(value, ScalarType::Integer(word()))
    }
    pub fn whole_representation_count_goal(
        source_length: ValueId,
        factor: u64,
    ) -> Option<Proposition> {
        if factor == 0 {
            return None;
        }
        Some(Proposition::LessOrEqual(
            value(source_length),
            literal(u64::MAX / factor),
        ))
    }
    pub fn representation_region_goal(
        start: ValueId,
        end: ValueId,
        extent: u64,
        stride: u64,
    ) -> Option<Proposition> {
        if stride == 0 {
            return None;
        }
        let difference =
            ScalarTerm::exact_integer_subtract(word(), value(end), value(start)).ok()?;
        let remainder =
            ScalarTerm::exact_integer_remainder(word(), difference, literal(stride)).ok()?;
        Some(Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(start), value(end)),
            Proposition::LessOrEqual(value(end), literal(extent)),
            Proposition::Equal(remainder, literal(0)),
        ]))
    }
    pub fn representation_region_length(
        start: ValueId,
        end: ValueId,
        stride: u64,
    ) -> Option<ScalarTerm> {
        if stride == 0 {
            return None;
        }
        ScalarTerm::exact_integer_divide(
            word(),
            ScalarTerm::exact_integer_subtract(word(), value(end), value(start)).ok()?,
            literal(stride),
        )
        .ok()
    }
}
pub mod run_custody {

    use semantic_vocabulary::{IntegerSign, StructuralFieldId};
    use terminal_psi::{
        BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
        CheckedBoundaryExecution, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    };

    pub struct EmptyRunReleaseContract<'module> {
        pub storage: TerminalIdentityIndex,
        pub count: StructuralFieldId,
        pub length: StructuralFieldId,
        pub result: &'module BoundaryStructuralResultDeclaration,
    }

    pub fn empty_run_release_contract<'module>(
        types: &'module [StructuralTypeDeclaration],
        boundary: &'module BoundaryMachineDeclaration,
    ) -> Result<EmptyRunReleaseContract<'module>, &'static str> {
        if boundary.declaration.as_ref().map(|row| row.execution)
            != Some(CheckedBoundaryExecution::EmptyRunRelease)
            || !boundary.scalar_parameters.is_empty()
        {
            return Err("empty-run release has no authenticated core execution");
        }
        let [parameter] = boundary.structural_parameters.as_slice() else {
            return Err("empty-run release requires one owned run");
        };
        let BoundaryMachineResult::Structural(result) = &boundary.result else {
            return Err("empty-run release must return the existing extent");
        };
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Linear
            || result.multiplicity != StructuralMultiplicity::Linear
        {
            return Err("empty-run release changed its linear custody contract");
        }
        let run = terminal_psi::find_structural_type(types, parameter.structural_type)
            .ok_or("empty-run release lost its run carrier")?;
        let StructuralTypeShape::Record { fields, .. } = &run.shape else {
            return Err("empty-run release has no record descriptor");
        };
        let [storage, count] = fields.as_slice() else {
            return Err("empty-run release changed its descriptor fields");
        };
        if storage.field_type.as_structural() != Some(result.structural_type)
            || storage.relevance.is_erased()
            || count.relevance.is_erased()
            || !unsigned_count(&count.field_type)
        {
            return Err("empty-run release changed its backing or count carrier");
        }
        let extent = terminal_psi::find_structural_type(types, result.structural_type)
            .ok_or("empty-run release lost its extent carrier")?;
        let StructuralTypeShape::Record { fields, .. } = &extent.shape else {
            return Err("empty-run release has no extent descriptor");
        };
        let [_base, length] = fields.as_slice() else {
            return Err("empty-run release changed its extent fields");
        };
        if length.relevance.is_erased() || !unsigned_count(&length.field_type) {
            return Err("empty-run release changed its extent length carrier");
        }
        Ok(EmptyRunReleaseContract {
            storage: storage.identity,
            count: count.id,
            length: length.id,
            result,
        })
    }

    fn unsigned_count(field: &StructuralFieldType) -> bool {
        field
            .scalar_type()
            .and_then(|scalar| scalar.as_integer())
            .is_some_and(|integer| integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
    }
}
pub(crate) mod storage_dependencies {

    use crate::canonical_storage_dependencies;
    use crate::projection_storage_dependencies;
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{StructuralStorageUse, StructuralTypeDeclaration, TerminalMachine};

    pub fn inline_storage_extent_dependencies(
        roots: impl IntoIterator<Item = StructuralTypeId>,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Result<BTreeSet<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        let mut applications = BTreeSet::new();
        while let Some(subject) = pending.pop() {
            if !visited.insert(subject) {
                continue;
            }
            let declaration = declarations
                .get(&subject)
                .ok_or_else(|| format!("inline storage has no declaration for {subject:?}"))?;
            if let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                declaration.shape
            {
                if let terminal_psi::ArrayExtent::Application(application) = length {
                    applications.insert(application);
                }
                pending.push(element);
            }
            for field in declaration
                .shape
                .direct_fields()
                .into_iter()
                .flatten()
                .chain(
                    declaration
                        .shape
                        .sum_cases()
                        .into_iter()
                        .flatten()
                        .flat_map(|case| &case.fields),
                )
                .filter(|field| !field.relevance.is_erased())
            {
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    pending.push(child);
                }
            }
        }
        Ok(applications)
    }

    pub fn storage_operand_dependencies(
        machine: &TerminalMachine,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        storage: StructuralStorageUse<'_>,
        roots: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), String> {
        let place_type = |place| {
            machine
                .structural_place_type(place)
                .ok_or_else(|| format!("storage operand {place:?} has no declared carrier"))
        };
        match storage {
            StructuralStorageUse::Place(place) => {
                roots.insert(place_type(place)?);
            }
            StructuralStorageUse::Type(structural_type) => {
                roots.insert(structural_type);
            }
            StructuralStorageUse::Projection(projection) => {
                projection_storage_dependencies(
                    declarations.values().copied(),
                    place_type(projection.root)?,
                    projection.path,
                    roots,
                )
                .ok_or_else(|| format!("malformed storage projection {projection:?}"))?;
            }
            StructuralStorageUse::Canonical { root, path } => {
                canonical_storage_dependencies(
                    declarations.values().copied(),
                    place_type(root)?,
                    path,
                    roots,
                )
                .ok_or_else(|| {
                    format!("malformed declaration-ID storage projection at {root:?}: {path:?}")
                })?;
            }
            StructuralStorageUse::ObservedField { root, path } => {
                let carrier = place_type(root)?;
                roots.insert(carrier);
                let observed = match declarations.get(&carrier).map(|row| &row.shape) {
                    Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
                        *referent
                    }
                    _ => carrier,
                };
                canonical_storage_dependencies(
                    declarations.values().copied(),
                    observed,
                    path,
                    roots,
                )
                .ok_or_else(|| {
                    format!("malformed scalar-field carrier projection at {root:?}: {path:?}")
                })?;
            }
            StructuralStorageUse::Element { descriptor, path } => {
                let descriptor_type = projection_storage_dependencies(
                    declarations.values().copied(),
                    place_type(descriptor.root)?,
                    descriptor.path,
                    roots,
                )
                .flatten()
                .ok_or_else(|| format!("element access has no descriptor at {descriptor:?}"))?;
                let Some(terminal_psi::StructuralTypeShape::ElementView { element, .. }) =
                    declarations.get(&descriptor_type).map(|row| &row.shape)
                else {
                    return Err(format!(
                        "element access carrier {descriptor_type:?} is not a view"
                    ));
                };
                projection_storage_dependencies(
                    declarations.values().copied(),
                    *element,
                    path,
                    roots,
                )
                .ok_or_else(|| {
                    format!(
                        "malformed element storage projection below {descriptor_type:?}: {path:?}"
                    )
                })?;
            }
        }
        Ok(())
    }
}
pub(crate) mod storage_regions {

    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, live_field_by_id,
    };

    pub struct StorageRegion {
        pub path: Vec<StructuralPathSegment>,
        pub indirect: bool,
    }

    impl StorageRegion {
        pub fn direct_path(&self) -> &[StructuralPathSegment] {
            if self.indirect { &[] } else { &self.path }
        }
    }

    pub fn crosses_unaliased_reference(path: &[StructuralPathSegment]) -> bool {
        path.iter()
            .skip(1)
            .any(|segment| *segment == StructuralPathSegment::Referent)
    }

    pub fn projected_storage_region<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> StorageRegion {
        let static_length = path
            .iter()
            .position(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::RepresentationView { .. }
                        | StructuralPathSegment::Referent
                )
            })
            .unwrap_or(path.len());
        let mut region = path[..static_length].to_vec();
        if static_length == path.len()
            && let Some(field) = field
            && let Some(declaration) =
                crate::runtime_structural_path_tip(declarations, root, &region)
            && let StructuralTypeShape::Record { fields, .. } = &declaration.shape
            && let Some(field) = live_field_by_id(fields, field)
        {
            region.push(StructuralPathSegment::Field(field.identity));
        }
        StorageRegion {
            path: region,
            indirect: crosses_unaliased_reference(path),
        }
    }

    pub fn canonical_field_storage_region<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        field: StructuralFieldId,
    ) -> Vec<StructuralPathSegment> {
        let mut region = Vec::with_capacity(path.len() + 1);
        for (position, segment) in path
            .iter()
            .chain(std::iter::once(&CanonicalStructuralPathSegment::Field(
                field,
            )))
            .enumerate()
        {
            match segment {
                CanonicalStructuralPathSegment::Case(_)
                | CanonicalStructuralPathSegment::Referent => {
                    return Vec::new();
                }
                CanonicalStructuralPathSegment::FixedIndex(index) => {
                    region.push(StructuralPathSegment::FixedIndex(*index))
                }
                CanonicalStructuralPathSegment::Field(field) => {
                    let Some(declaration) = crate::canonical_structural_path_tip(
                        declarations.clone(),
                        root,
                        &path[..position],
                    ) else {
                        return Vec::new();
                    };
                    let Some(fields) = declaration.shape.record_fields() else {
                        return Vec::new();
                    };
                    let Some(field) = live_field_by_id(fields, *field) else {
                        return Vec::new();
                    };
                    region.push(StructuralPathSegment::Field(field.identity));
                }
            }
        }
        region
    }
}
pub(crate) mod structural_locals {

    use terminal_psi::{StructuralOperationResult, StructuralTypeDeclaration};

    pub fn validate_vacant_structural_local<'a>(
        declarations: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        result: &StructuralOperationResult,
    ) -> Result<(), &'static str> {
        let mut matching = declarations
            .into_iter()
            .filter(|item| item.id == result.structural_type);
        let declaration = matching
            .next()
            .ok_or("structural local has no declared storage type")?;
        if matching.next().is_some() {
            return Err("structural local repeats its storage type declaration");
        }
        if !result.claims.is_empty() || !result.qualification_establishments.is_empty() {
            return Err("vacant structural local introduces value custody");
        }
        if !result.projected_qualifications.is_empty()
            || (!result.qualifications.is_empty() && !declaration.shape.is_borrowed_byte_view())
        {
            return Err("vacant structural local has invalid declared qualifications");
        }
        Ok(())
    }
}
pub(crate) mod zero_valid_storage {

    use semantic_vocabulary::{ScalarType, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralFieldDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    pub fn zero_valid_record_storage(
        declarations: &[StructuralTypeDeclaration],
        structural_type: StructuralTypeId,
        visiting: &mut Vec<StructuralTypeId>,
        zero_valid_field: impl Fn(
            &[StructuralTypeDeclaration],
            &StructuralFieldDeclaration,
            &mut Vec<StructuralTypeId>,
        ) -> bool
        + Copy,
    ) -> bool {
        let Some(declaration) =
            terminal_psi::unique_structural_declaration(declarations, structural_type, visiting)
        else {
            return false;
        };
        visiting.push(structural_type);
        let valid = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting)),
            StructuralTypeShape::FixedArray { element, length } => {
                if length.literal() == Some(0)
                    && crate::scalar_array_leaf_declaration(declarations.iter(), structural_type)
                        .is_some()
                {
                    true
                } else {
                    zero_valid_record_storage(declarations, *element, visiting, zero_valid_field)
                }
            }
            StructuralTypeShape::Sum { cases } => cases.first().is_some_and(|case| {
                case.fields
                    .iter()
                    .all(|field| zero_valid_field(declarations, field, visiting))
            }),
            StructuralTypeShape::Mixed { fields, cases } => {
                fields
                    .iter()
                    .all(|field| zero_valid_field(declarations, field, visiting))
                    && cases.first().is_some_and(|case| {
                        case.fields
                            .iter()
                            .all(|field| zero_valid_field(declarations, field, visiting))
                    })
            }
            StructuralTypeShape::PrimitiveScalar(
                ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
            ) => scalar_declaration_accepts_zero(declaration),
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
            _ => false,
        };
        visiting.pop();
        valid
    }

    pub fn scalar_declaration_accepts_zero(declaration: &StructuralTypeDeclaration) -> bool {
        matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
            && declaration
                .scalar_restriction
                .as_ref()
                .is_none_or(crate::scalar_field_accepts_zero)
    }
}

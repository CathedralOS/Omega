//! Address descriptors and their borrowed state views have separate custody.
//! A selected formation owns bounds evaluation. Its descriptor travels
//! beside the authored referent, through whole forwarding, and dies where that
//! referent's state chain ends. Mutable descriptors retain affine custody;
//! shared fixed byte descriptors are copyable. Caller storage never becomes
//! owned transport.
//! Static geometry needs no new capture, but every projected descendant keeps
//! its lineage: a static child can still depend on a dynamic ancestor address.
use crate::LoweringError;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedStructuralControlTransferSourcePlan as Source,
    CheckedUnitStructuralArgumentSourcePlan as Root,
};
use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, TerminalMachine, Terminator,
};
use typed_trees::type_identity::TypeIdentityId;

#[derive(Default)]
pub(super) struct CapturedLoans {
    nodes: Vec<Vec<usize>>,
    owners: Vec<Vec<Owner>>,
    ancestry: Vec<(usize, Vec<usize>)>,
}

struct Owner {
    group: usize,
    declaration: StructuralParameterDeclaration,
}

fn root(links: &[usize], mut index: usize) -> usize {
    while links[index] != index {
        index = links[index];
    }
    index
}

fn depends_on(parents: &[(usize, usize)], child: usize, ancestor: usize) -> bool {
    let mut pending = vec![child];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if current == ancestor {
            return true;
        }
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        pending.extend(
            parents
                .iter()
                .filter_map(|(child, parent)| (*child == current).then_some(*parent)),
        );
    }
    false
}

fn forwarded(source: &Source) -> Option<u32> {
    match source {
        Source::Parameter { index } => Some(*index),
        Source::ProjectedLoan { subject, .. } if subject.path.is_empty() => match subject.source {
            Root::Parameter { parameter_index } => Some(parameter_index),
            _ => None,
        },
        _ => None,
    }
}

impl CapturedLoans {
    pub(super) fn plan(
        plan: &CheckedComposedUnitControlMachinePlan,
        values: &checked_trees::CheckedStructuralValuePlans,
        live: &[bool],
        views: &[Vec<StructuralParameterDeclaration>],
        headers: &[BlockId],
        aliases: &[std::collections::BTreeMap<u32, u32>],
        catalogs: &mut crate::unit::attached_unit::composed_control::catalogs::ComposedCatalogs<'_>,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<Self, LoweringError> {
        let mut links = Vec::new();
        let mut nodes = Vec::new();
        for state in &plan.states {
            nodes.push(
                state
                    .structural_parameters
                    .iter()
                    .map(|_| {
                        let index = links.len();
                        links.push(index);
                        index
                    })
                    .collect::<Vec<_>>(),
            );
        }
        for (position, state) in plan
            .states
            .iter()
            .enumerate()
            .filter(|(position, _)| live[*position])
        {
            for edge in state.successors() {
                let target =
                    plan.state_position(edge.target_state)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan target disappeared",
                        ))?;
                for transfer in &edge.transfers {
                    if position != 0
                        && target != 0
                        && let Some(source) = forwarded(&transfer.source)
                        && matches!(
                            state.structural_parameters[source as usize].access,
                            checked_trees::checked_borrowed_access_pair_pattern!()
                        )
                        && matches!(
                            plan.states[target].structural_parameters
                                [transfer.target_parameter_index as usize]
                                .access,
                            checked_trees::checked_borrowed_access_pair_pattern!()
                        )
                    {
                        let left = root(&links, nodes[position][source as usize]);
                        let right = root(
                            &links,
                            nodes[target][transfer.target_parameter_index as usize],
                        );
                        links[right] = left;
                    }
                }
            }
        }
        for state in &mut nodes {
            for index in state {
                *index = root(&links, *index);
            }
        }
        // Runtime selectors need an address snapshot before later arguments.
        // A static incoming edge to the same header forms the same descriptor;
        // headers with only static geometry keep their canonical place route.
        let mut types: Vec<(usize, TypeIdentityId)> = Vec::new();
        let mut shared_byte_carriers = Vec::new();
        for (position, state) in plan
            .states
            .iter()
            .enumerate()
            .filter(|(position, _)| live[*position])
        {
            for edge in state.successors() {
                let target =
                    plan.state_position(edge.target_state)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan target disappeared",
                        ))?;
                for transfer in &edge.transfers {
                    let carrier_type_identity = match &transfer.source {
                        Source::ProjectedLoan {
                            carrier_type_identity,
                            subject,
                            ..
                        } if subject.path.iter().any(|segment| {
                            matches!(segment,
                                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(_))
                        }) =>
                        {
                            Some(*carrier_type_identity)
                        }
                        Source::EstablishedValue { establishment, .. }
                            if super::super::result_custody::loan_subject(
                                &transfer.source,
                                values,
                            )
                            .is_some() =>
                        {
                            establishment
                                .structural_result_binding()
                                .map(|result| result.type_identity)
                        }
                        _ => None,
                    };
                    if let Some(carrier_type_identity) =
                        carrier_type_identity.filter(|identity| identity.is_valid())
                    {
                        let group = nodes[target][transfer.target_parameter_index as usize];
                        if let Some((_, previous)) =
                            types.iter().find(|(candidate, _)| *candidate == group)
                        {
                            if *previous != carrier_type_identity {
                                return Err(LoweringError::Unsupported(
                                    "captured loan forwarding changes descriptor type",
                                ));
                            }
                        } else {
                            types.push((group, carrier_type_identity));
                        }
                        let carrier = crate::terminal_identities::lookup_type_id(
                            &catalogs.type_ids,
                            &carrier_type_identity,
                        )?;
                        if plan.states[target].structural_parameters
                            [transfer.target_parameter_index as usize]
                            .access
                            == checked_trees::CheckedStructuralAccess::SharedBorrow
                            && terminal_psi::find_structural_type(
                                &catalogs.structural_types,
                                carrier,
                            )
                            .is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                    )
                                )
                            })
                            && !shared_byte_carriers.contains(&group)
                        {
                            shared_byte_carriers.push(group);
                        }
                    }
                }
            }
            let _ = position;
        }
        // Reborrowing creates a new descriptor, not another name for its
        // parent. Preserve every directed projection, including static children
        // without their own descriptor: grandchildren still need that lineage.
        // Descriptor eligibility cannot gate ancestry: a static borrowed child
        // retains its dynamically captured parent even without forming another
        // descriptor of its own.
        let mut parents = Vec::new();
        for (position, state) in plan
            .states
            .iter()
            .enumerate()
            .filter(|(position, _)| live[*position])
        {
            for edge in state.successors() {
                let target =
                    plan.state_position(edge.target_state)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan target disappeared",
                        ))?;
                for transfer in &edge.transfers {
                    if let Some((subject, _)) =
                        super::super::result_custody::loan_subject(&transfer.source, values)
                        && let Root::Parameter { parameter_index } = subject.source
                    {
                        let parent = nodes[position][parameter_index as usize];
                        let child = nodes[target][transfer.target_parameter_index as usize];
                        if !(subject.path.is_empty() && child == parent)
                            && !parents.contains(&(child, parent))
                        {
                            parents.push((child, parent));
                        }
                    }
                }
            }
        }
        // Equal descriptor types on mutually exclusive parent routes use one
        // selected incoming owner, like an ordinary owned block parameter.
        // Coexisting resources must retain separate identities: distinct
        // authored views (including their ancestors) witness coexistence.
        // An ancestor and descendant can never be alternatives of one slot.
        let original_parents = parents.clone();
        for (child, _) in &original_parents {
            let alternatives = types
                .iter()
                .filter_map(|(ancestor, _)| {
                    original_parents
                        .iter()
                        .any(|(candidate, parent)| {
                            candidate == child && depends_on(&original_parents, *parent, *ancestor)
                        })
                        .then_some(*ancestor)
                })
                .collect::<Vec<_>>();
            for (index, left) in alternatives.iter().enumerate() {
                for right in &alternatives[index + 1..] {
                    let left_root = root(&links, *left);
                    let right_root = root(&links, *right);
                    if left_root == right_root {
                        continue;
                    }
                    if types
                        .iter()
                        .find(|(group, _)| group == left)
                        .map(|(_, identity)| identity)
                        != types
                            .iter()
                            .find(|(group, _)| group == right)
                            .map(|(_, identity)| identity)
                    {
                        continue;
                    }
                    // Test complete equivalence classes. Pairwise exclusion
                    // alone is not transitive: A and C may coexist even when
                    // each is separately an alternative of B.
                    let coexist = types
                        .iter()
                        .filter(|(group, _)| root(&links, *group) == left_root)
                        .any(|(left, _)| {
                            types
                                .iter()
                                .filter(|(group, _)| root(&links, *group) == right_root)
                                .any(|(right, _)| {
                                    depends_on(&original_parents, *left, *right)
                                        || depends_on(&original_parents, *right, *left)
                                        || nodes.iter().enumerate().any(|(state, members)| {
                                            live[state]
                                                && members.iter().any(|first| {
                                                    members.iter().any(|second| {
                                                        first != second
                                                            && depends_on(
                                                                &original_parents,
                                                                *first,
                                                                *left,
                                                            )
                                                            && depends_on(
                                                                &original_parents,
                                                                *second,
                                                                *right,
                                                            )
                                                    })
                                                })
                                        })
                                })
                        });
                    if !coexist {
                        links[right_root] = left_root;
                    }
                }
            }
        }
        for state in &mut nodes {
            for group in state {
                *group = root(&links, *group);
            }
        }
        for (group, _) in &mut types {
            *group = root(&links, *group);
        }
        for group in &mut shared_byte_carriers {
            *group = root(&links, *group);
        }
        types.sort_by_key(|(group, _)| *group);
        types.dedup_by_key(|(group, _)| *group);
        for (child, parent) in &mut parents {
            *child = root(&links, *child);
            *parent = root(&links, *parent);
        }
        parents.sort_unstable();
        parents.dedup();
        let mut ancestry = Vec::new();
        let mut groups = Vec::new();
        for group in nodes.iter().flatten() {
            if !groups.contains(group) {
                groups.push(*group);
            }
        }
        for group in &groups {
            let mut ancestors = Vec::new();
            let mut pending = vec![*group];
            while let Some(child) = pending.pop() {
                for (_, parent) in parents.iter().filter(|(candidate, _)| *candidate == child) {
                    if parent == group {
                        return Err(LoweringError::Unsupported(
                            "captured loan has a cyclic parent lifetime",
                        ));
                    }
                    if !ancestors.contains(parent) {
                        ancestors.push(*parent);
                        pending.push(*parent);
                    }
                }
            }
            ancestry.push((*group, ancestors));
        }
        // Retire children before parents. Parameter order also makes the
        // independent affine cleanup passes observe that same custody order.
        types.sort_by_key(|(group, _)| {
            std::cmp::Reverse(
                ancestry
                    .iter()
                    .find(|(candidate, _)| candidate == group)
                    .map_or(0, |(_, ancestors)| ancestors.len()),
            )
        });
        let mut owners = Vec::new();
        for (position, state) in nodes.iter().enumerate() {
            let mut roster = Vec::new();
            if live[position] {
                for (group, identity) in &types {
                    if !state.contains(group)
                        && !ancestry.iter().any(|(child, ancestors)| {
                            state.contains(child) && ancestors.contains(group)
                        })
                    {
                        continue;
                    }
                    if position == 0 {
                        return Err(LoweringError::Unsupported(
                            "captured state loan requires invocation carrier arrival",
                        ));
                    }
                    let structural_type =
                        crate::terminal_identities::lookup_type_id(&catalogs.type_ids, identity)?;
                    let place = super::super::super::super::place_id(
                        super::super::super::super::allocate_dense(&mut catalogs.next_place)?,
                    );
                    let index = views[position].len() - aliases[position].len() + roster.len();
                    let parameter = StructuralParameterDeclaration {
                        place,
                        position: u32::try_from(index).map_err(|_| {
                            LoweringError::Unsupported("captured loan roster exceeds u32")
                        })?,
                        structural_type,
                        // Shared bounded byte views are copy descriptors, while
                        // exclusive views and generic references own affine
                        // lifetime resources. Transport keeps that producer
                        // contract instead of imposing a reference-only one.
                        multiplicity: if shared_byte_carriers.contains(group) {
                            StructuralMultiplicity::Unrestricted
                        } else {
                            StructuralMultiplicity::Affine
                        },
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    };
                    places.push(StructuralPlaceDeclaration {
                        id: place,
                        kind: StructuralPlaceKind::BlockParameter {
                            block: headers[position],
                            position: parameter.position,
                        },
                    });
                    roster.push(Owner {
                        group: *group,
                        declaration: parameter,
                    });
                }
            }
            owners.push(roster);
        }
        Ok(Self {
            nodes,
            owners,
            ancestry,
        })
    }

    pub(super) fn group(&self, state: usize, parameter: u32) -> Option<usize> {
        let group = *self.nodes.get(state)?.get(parameter as usize)?;
        self.owners[state]
            .iter()
            .any(|owner| owner.group == group)
            .then_some(group)
    }

    pub(super) fn declarations(
        &self,
        state: usize,
    ) -> impl Iterator<Item = StructuralParameterDeclaration> + '_ {
        self.owners[state]
            .iter()
            .map(|owner| owner.declaration.clone())
    }

    pub(super) fn parent_carrier(
        &self,
        state: usize,
        parameter: u32,
    ) -> Option<&StructuralParameterDeclaration> {
        let group = *self.nodes.get(state)?.get(parameter as usize)?;
        self.owners[state]
            .iter()
            .find(|owner| owner.group == group)
            .map(|owner| &owner.declaration)
    }

    pub(super) fn arrivals(
        &self,
        source: usize,
        target: usize,
        edge: &checked_trees::CheckedStructuralControlSuccessorPlan,
        captures: &[(usize, PlaceId)],
    ) -> Result<Vec<StructuralArgument>, LoweringError> {
        self.owners[target]
            .iter()
            .map(|owner| {
                let captures = captures
                    .iter()
                    .filter(|(group, _)| *group == owner.group)
                    .collect::<Vec<_>>();
                if captures.len() > 1 {
                    return Err(LoweringError::Unsupported(
                        "one state loan owner receives multiple captures",
                    ));
                }
                let place = if let Some((_, place)) = captures.first() {
                    *place
                } else {
                    let forwarded_here = edge.transfers.iter().any(|transfer| {
                        self.nodes[target][transfer.target_parameter_index as usize] == owner.group
                            && forwarded(&transfer.source).is_some_and(|parameter| {
                                self.nodes[source][parameter as usize] == owner.group
                            })
                    });
                    let retained_parent = self.ancestry.iter().any(|(child, ancestors)| {
                        self.nodes[target].contains(child) && ancestors.contains(&owner.group)
                    });
                    if !forwarded_here && !retained_parent {
                        return Err(LoweringError::Unsupported(
                            "captured loan arrival does not forward its source resource",
                        ));
                    }
                    self.owners[source]
                        .iter()
                        .find(|previous| previous.group == owner.group)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan arrival has no matching owner",
                        ))?
                        .declaration
                        .place
                };
                Ok(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                })
            })
            .collect()
    }

    pub(super) fn finish(
        &self,
        machine: &mut TerminalMachine,
        headers: &[BlockId],
    ) -> Result<(), LoweringError> {
        for (position, owners) in self
            .owners
            .iter()
            .enumerate()
            .filter(|(_, owners)| !owners.is_empty())
        {
            // A private cleanup capture can move the descriptor into a new
            // block parameter. Follow that exact owned binding, rather than
            // disposing the already consumed header place at a later exit.
            for owner in owners {
                if owner.declaration.multiplicity != StructuralMultiplicity::Affine {
                    continue;
                }
                let mut region = vec![(headers[position], owner.declaration.place)];
                let mut cursor = 0;
                while cursor < region.len() {
                    let (block, place) = region[cursor];
                    let block = machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan private block disappeared",
                        ))?;
                    let arrivals = match &block.terminator {
                        Terminator::Jump {
                            target,
                            structural_arguments,
                            ..
                        } => vec![(*target, structural_arguments.as_slice())],
                        Terminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => vec![
                            (when_true.target, when_true.structural_arguments.as_slice()),
                            (
                                when_false.target,
                                when_false.structural_arguments.as_slice(),
                            ),
                        ],
                        Terminator::StructuralCase { cases, .. } => {
                            cases.iter().map(|arm| (arm.target, &[][..])).collect()
                        }
                        _ => Vec::new(),
                    };
                    for (target, arguments) in arrivals {
                        if headers.contains(&target) {
                            continue;
                        }
                        let destination = machine
                            .blocks
                            .iter()
                            .find(|block| block.id == target)
                            .ok_or(LoweringError::Unsupported(
                                "captured loan private target disappeared",
                            ))?;
                        let moved = arguments
                            .iter()
                            .zip(&destination.structural_parameters)
                            .filter(|(argument, _)| {
                                argument.place == place
                                    && argument.access == StructuralAccess::Owned
                                    && argument.path.is_empty()
                            })
                            .map(|(_, parameter)| parameter.place)
                            .collect::<Vec<_>>();
                        if moved.len() > 1 {
                            return Err(LoweringError::Unsupported(
                                "captured loan duplicates a private owner",
                            ));
                        }
                        let arriving = moved.first().copied().unwrap_or(place);
                        if let Some((_, previous)) =
                            region.iter().find(|(block, _)| *block == target)
                        {
                            if *previous != arriving {
                                return Err(LoweringError::Unsupported(
                                    "captured loan private join changes owner correspondence",
                                ));
                            }
                        } else {
                            region.push((target, arriving));
                        }
                    }
                    cursor += 1;
                }
                for (block, place) in region {
                    let block = machine
                        .blocks
                        .iter_mut()
                        .find(|candidate| candidate.id == block)
                        .ok_or(LoweringError::Unsupported(
                            "captured loan private block disappeared",
                        ))?;
                    let retire =
                        |target: BlockId,
                         arguments: &[StructuralArgument],
                         actions: &mut Vec<TerminalAffineCleanupAction>| {
                            if headers.contains(&target)
                                && !arguments.iter().any(|argument| {
                                    argument.place == place
                                        && argument.access == StructuralAccess::Owned
                                        && argument.path.is_empty()
                                })
                            {
                                actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
                            }
                        };
                    match &mut block.terminator {
                        Terminator::Jump {
                            target,
                            structural_arguments,
                            cleanup_actions,
                            ..
                        } => retire(*target, structural_arguments, cleanup_actions),
                        Terminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => {
                            for arm in [when_true, when_false] {
                                retire(
                                    arm.target,
                                    &arm.structural_arguments,
                                    &mut arm.cleanup_actions,
                                );
                            }
                        }
                        Terminator::StructuralCase { cases, .. } => {
                            for arm in cases {
                                retire(arm.target, &[], &mut arm.cleanup_actions);
                            }
                        }
                        terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..)
                        | Terminator::ReturnStructural {
                            cleanup_actions, ..
                        } => cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place)),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

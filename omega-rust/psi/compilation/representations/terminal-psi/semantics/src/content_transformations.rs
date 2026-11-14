/*
Shared operation semantics for content transformations, domain-membership preservation across
mutations and transfers (`domain_preservation`), and quotient representation opacity (`quotients`).
*/

use crate::runtime_structural_path_tip;
use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentTerm, PlaceId,
    StructuralDomainId, StructuralTypeId,
};
use terminal_psi::ContentTransformationCustody;
use terminal_psi::{
    ContentClaimProjection, ContentTransformationDerivation, StructuralArgument,
    StructuralDomainDeclaration, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

pub(crate) mod ancestry {

    use semantic_vocabulary::{ClaimId, ContentProjectionIdentity, MachineId, OperationId};
    use terminal_psi::{
        ContentLineageStep, ContentTransformationDerivation, Operation, OperationKind,
        TerminalMachine, TerminalModule,
    };

    pub(crate) mod arrivals {

        use super::{OriginTraversal, local_roots};
        use semantic_vocabulary::{BlockId, ClaimId, ContentProjectionIdentity, PlaceId};
        use terminal_psi::{
            StructuralAccess, StructuralPathSegment, TerminalMachine, TerminalModule,
        };

        fn source_claim(
            machine: &TerminalMachine,
            dominators: &crate::control_graph::DominatorTree,
            predecessor: BlockId,
            place: PlaceId,
            path: &[StructuralPathSegment],
            traversal: &mut OriginTraversal,
        ) -> Result<ClaimId, &'static str> {
            let mut selected = None;
            let mut retain = |claim| {
                if selected.is_some_and(|previous| previous != claim) {
                    return Err("content arrival has ambiguous source claim identities");
                }
                selected = Some(claim);
                Ok(())
            };
            for entry in &machine.entry_claims {
                traversal.visit()?;
                if entry.input == place && entry.path == path {
                    retain(entry.claim)?;
                }
            }
            for block in &machine.blocks {
                traversal.visit()?;
                if !dominators.dominates(block.id, predecessor) {
                    continue;
                }
                if block
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == place)
                {
                    for entry in &machine.block_entry_claims {
                        traversal.visit()?;
                        if entry.input == place && entry.path == path {
                            retain(entry.claim)?;
                        }
                    }
                }
                for operation in &block.operations {
                    traversal.visit()?;
                    if let Some(result) = operation
                        .result
                        .as_structural()
                        .filter(|result| result.place == place)
                    {
                        for binding in &result.claims {
                            traversal.visit()?;
                            if binding.path == path {
                                retain(binding.claim)?;
                            }
                        }
                    }
                }
            }
            selected.ok_or("content arrival has no exact dominating source claim")
        }

        pub(crate) fn roots(
            module: &TerminalModule,
            machine: &TerminalMachine,
            claim: ClaimId,
            projection: ContentProjectionIdentity,
            traversal: &mut OriginTraversal,
        ) -> Result<Option<Vec<ClaimId>>, &'static str> {
            let mut entries = machine
                .block_entry_claims
                .iter()
                .filter(|entry| entry.claim == claim);
            let Some(entry) = entries.next() else {
                return Ok(None);
            };
            if entries.next().is_some() {
                return Err("content arrival has ambiguous destination claim declarations");
            }
            let mut parameters = machine.blocks.iter().flat_map(|block| {
                block.structural_parameters.iter().enumerate().filter_map(
                    move |(position, parameter)| {
                        (parameter.place == entry.input).then_some((block, position, parameter))
                    },
                )
            });
            let (destination, position, parameter) = parameters
                .next()
                .ok_or("content arrival lacks its destination formal")?;
            if parameters.next().is_some() || parameter.access != StructuralAccess::Owned {
                return Err("content arrival lacks one exact owned destination formal");
            }
            let dominators = crate::control_graph::dominators(machine);
            let mut roots = None;
            let mut incoming = false;
            for predecessor in &machine.blocks {
                traversal.visit()?;
                let arguments = predecessor.terminator.successor_block_arguments();
                for (index, (_, target)) in
                    predecessor.terminator.successor_edges().iter().enumerate()
                {
                    traversal.visit()?;
                    if *target != destination.id {
                        continue;
                    }
                    incoming = true;
                    let actual = arguments
                        .get(index)
                        .and_then(|arguments| arguments.get(position))
                        .ok_or("content arrival lacks an exact positional structural actual")?;
                    if actual.access != StructuralAccess::Owned {
                        return Err("content arrival changes the source's owned access");
                    }
                    let mut path = actual.path.clone();
                    path.extend_from_slice(&entry.path);
                    let source = source_claim(
                        machine,
                        &dominators,
                        predecessor.id,
                        actual.place,
                        &path,
                        traversal,
                    )?;
                    if source == claim {
                        continue;
                    }
                    let candidate = local_roots(module, machine, source, projection, traversal)?;
                    if roots
                        .as_ref()
                        .is_some_and(|previous| previous != &candidate)
                    {
                        return Err("content arrival input rosters differ across incoming paths");
                    }
                    roots = Some(candidate);
                }
            }
            if !incoming {
                return Err("content arrival has no grounded predecessor");
            }
            Ok(roots)
        }
    }
    pub(crate) mod retirement {

        use super::{OriginTraversal, call_input_origins};
        use semantic_vocabulary::{BlockId, ClaimId, ContentProjectionIdentity, PlaceId};
        use terminal_psi::{
            ClaimTransfer, Operation, OperationKind, StructuralAccess, StructuralArgument,
            StructuralPathSegment, TerminalAffineCleanupAction, TerminalMachine, TerminalModule,
            Terminator,
        };

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct ClaimLocation {
            identity: ClaimId,
            place: PlaceId,
            path: Vec<StructuralPathSegment>,
        }

        fn owned_call<'a>(
            module: &'a TerminalModule,
            operation: &'a Operation,
        ) -> Result<(&'a TerminalMachine, &'a [ClaimTransfer]), &'static str> {
            let callee = operation
                .kind
                .direct_callee()
                .and_then(|identity| {
                    module
                        .machines
                        .iter()
                        .find(|machine| machine.id == identity)
                })
                .ok_or("nominal retirement requires an exact in-module callable")?;
            let transfers = operation.kind.claim_transfers();
            Ok((callee, transfers))
        }

        fn transferred_entry_claim(
            callee: &TerminalMachine,
            transfer: &ClaimTransfer,
        ) -> Result<ClaimId, &'static str> {
            let parameter = callee
                .structural_parameters
                .get(transfer.argument_index as usize)
                .ok_or("nominal retirement transfer has no structural formal")?;
            if parameter.access != StructuralAccess::Owned {
                return Err("nominal retirement transfer does not enter an owned formal");
            }
            let mut matching = callee
                .entry_claims
                .iter()
                .filter(|entry| entry.input == parameter.place && entry.path == transfer.path);
            let entry = matching
                .next()
                .ok_or("nominal retirement transfer has no exact entry claim")?;
            if matching.next().is_some() {
                return Err("nominal retirement transfer has ambiguous entry claims");
            }
            Ok(entry.claim)
        }

        pub(crate) fn call_input_is_retired(
            module: &TerminalModule,
            operation: &Operation,
            input: ClaimId,
            projection: ContentProjectionIdentity,
            traversal: &mut OriginTraversal,
        ) -> Result<bool, &'static str> {
            let (callee, transfers) = owned_call(module, operation)?;
            let mut matching = transfers.iter().filter(|transfer| transfer.claim == input);
            let transfer = matching
                .next()
                .ok_or("nominal retirement input has no exact call transfer")?;
            if matching.next().is_some() {
                return Err("nominal retirement input transfers more than once");
            }
            entry_claim_is_retired(
                module,
                callee,
                transferred_entry_claim(callee, transfer)?,
                projection,
                traversal,
            )
        }

        fn retain_claim(
            frontier: &mut Vec<ClaimLocation>,
            claim: ClaimLocation,
        ) -> Result<(), &'static str> {
            if let Some(previous) = frontier
                .iter()
                .find(|previous| previous.identity == claim.identity)
            {
                if previous != &claim {
                    return Err("nominal retirement claim has ambiguous current storage");
                }
            } else {
                frontier.push(claim);
            }
            frontier.sort_by_key(|claim| claim.identity);
            Ok(())
        }

        fn produced_claim(
            operation: &Operation,
            identity: ClaimId,
        ) -> Result<ClaimLocation, &'static str> {
            let result = operation
                .result
                .as_structural()
                .ok_or("nominal retirement result has no structural carrier")?;
            let mut matching = result.claims.iter().filter(|claim| claim.claim == identity);
            let claim = matching
                .next()
                .ok_or("nominal retirement result has no exact claim path")?;
            if matching.next().is_some() {
                return Err("nominal retirement result repeats a claim");
            }
            Ok(ClaimLocation {
                identity,
                place: result.place,
                path: claim.path.clone(),
            })
        }

        fn advance_owned_frontier(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
            frontier: &mut Vec<ClaimLocation>,
            projection: ContentProjectionIdentity,
            traversal: &mut OriginTraversal,
        ) -> Result<bool, &'static str> {
            if frontier.is_empty() {
                return Ok(true);
            }
            if let OperationKind::TransformContent { derivations, .. } = &operation.kind {
                let mut consumed = Vec::new();
                let mut outputs = Vec::new();
                for derived in derivations {
                    traversal.visit()?;
                    if !derived
                        .inputs
                        .iter()
                        .any(|input| frontier.iter().any(|claim| claim.identity == input.claim))
                    {
                        continue;
                    }
                    for input in &derived.inputs {
                        traversal.visit()?;
                        if let Some(claim) =
                            frontier.iter().find(|claim| claim.identity == input.claim)
                        {
                            let root = super::super::place_type(machine, input.subject.root)
                                .ok_or(
                                    "nominal retirement transformation lacks its current carrier",
                                )?;
                            let path = super::super::structural_path(
                                module.structural_types.iter(),
                                &module.identities,
                                root,
                                &input.subject.segments,
                            )
                            .ok_or(
                                "nominal retirement transformation lacks its exact typed path",
                            )?;
                            if claim.place != input.subject.root || claim.path != path {
                                return Err(
                                    "nominal retirement transformation names another current subject",
                                );
                            }
                        }
                    }
                    for input in &derived.inputs {
                        traversal.visit()?;
                        if !consumed.contains(&input.claim) {
                            consumed.push(input.claim);
                        }
                    }
                    for output in &derived.outputs {
                        traversal.visit()?;
                        if !outputs.contains(&output.claim) {
                            outputs.push(output.claim);
                        }
                    }
                }
                frontier.retain(|claim| !consumed.contains(&claim.identity));
                for output in outputs {
                    retain_claim(frontier, produced_claim(operation, output)?)?;
                }
            } else if operation.kind.direct_callee().is_some() {
                let (callee, transfers) = owned_call(module, operation)?;
                let moving = frontier
                    .iter()
                    .filter(|claim| {
                        transfers
                            .iter()
                            .any(|transfer| transfer.claim == claim.identity)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !moving.is_empty() {
                    let mut returned = Vec::new();
                    if let Some(result) = operation.result.as_structural() {
                        for output in &result.claims {
                            let inputs = call_input_origins(
                                module,
                                operation,
                                output.claim,
                                projection,
                                traversal,
                            )?;
                            returned.push((output.claim, inputs));
                        }
                    }
                    for claim in moving {
                        traversal.visit()?;
                        let mut matching = transfers
                            .iter()
                            .filter(|transfer| transfer.claim == claim.identity);
                        let transfer = matching
                            .next()
                            .ok_or("nominal retirement lost a selected call transfer")?;
                        if matching.next().is_some() {
                            return Err("nominal retirement call duplicates an owned input");
                        }
                        let argument = operation
                            .kind
                            .structural_arguments()
                            .get(transfer.argument_index as usize)
                            .ok_or(
                                "nominal retirement transfer has no actual structural operand",
                            )?;
                        let mut actual_path = argument.path.clone();
                        actual_path.extend_from_slice(&transfer.path);
                        if argument.access != StructuralAccess::Owned
                            || argument.place != claim.place
                            || actual_path != claim.path
                        {
                            return Err(
                                "nominal retirement call transfers a different current claim subject",
                            );
                        }
                        let outputs = returned
                            .iter()
                            .filter(|(_, inputs)| inputs.contains(&claim.identity))
                            .map(|(output, _)| *output)
                            .collect::<Vec<_>>();
                        if outputs.is_empty()
                            && !entry_claim_is_retired(
                                module,
                                callee,
                                transferred_entry_claim(callee, transfer)?,
                                projection,
                                traversal,
                            )?
                        {
                            return Ok(false);
                        }
                        frontier.retain(|current| current.identity != claim.identity);
                        for output in outputs {
                            retain_claim(frontier, produced_claim(operation, output)?)?;
                        }
                    }
                }
            } else {
                if operation.kind.claim_transfers().iter().any(|transfer| {
                    frontier
                        .iter()
                        .any(|claim| claim.identity == transfer.claim)
                }) {
                    return Err(
                        "opaque or dynamic custody transfer is not an authenticated nominal retirement",
                    );
                }
                if let OperationKind::StoreStructuralField { value, .. } = &operation.kind
                    && value.access == StructuralAccess::Owned
                    && frontier.iter().any(|claim| {
                        claim.place == value.place && claim.path.starts_with(&value.path)
                    })
                {
                    return Err(
                        "borrowed-storage custody exit needs its own exact restoration evidence, not a nominal-disposal inference",
                    );
                }
            }
            if let Some(result) = operation.result.as_structural() {
                for claim in frontier.iter_mut() {
                    if result
                        .claims
                        .iter()
                        .any(|binding| binding.claim == claim.identity)
                    {
                        *claim = produced_claim(operation, claim.identity)?;
                    }
                }
            }
            Ok(true)
        }

        fn apply_nominal_disposals(
            module: &TerminalModule,
            machine: &TerminalMachine,
            actions: &[TerminalAffineCleanupAction],
            frontier: &mut Vec<ClaimLocation>,
        ) -> Result<(), &'static str> {
            for action in actions {
                match action {
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        let selected = frontier
                            .iter()
                            .filter(|claim| cleanup.consumed_claims.contains(&claim.identity))
                            .cloned()
                            .collect::<Vec<_>>();
                        if selected.is_empty() {
                            continue;
                        }
                        crate::structural::nominal_cleanup::target(
                            module.structural_types.iter(),
                            module.machines.iter(),
                            cleanup,
                        )?;
                        if cleanup.retains_descendants {
                            return Err(
                                "nominal retirement needs the completed descendant partition, not a retaining hook",
                            );
                        }
                        let declaration = module
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.id == cleanup.structural_type)
                            .ok_or("nominal retirement subject type is absent")?;
                        if !declaration.owns_linear_claim
                            || cleanup.consumed_claims.len() != 1
                            || selected.len() != 1
                        {
                            return Err("nominal retirement requires an exact whole leaf claim");
                        }
                        let claim = &selected[0];
                        if claim.place != cleanup.place || claim.path != cleanup.path {
                            return Err(
                                "nominal retirement cleanup names different current storage",
                            );
                        }
                        let root = super::super::place_type(machine, cleanup.place)
                            .ok_or("nominal retirement current root has no structural type")?;
                        let selected = crate::runtime_structural_path_tip(
                            module.structural_types.iter(),
                            root,
                            &cleanup.path,
                        )
                        .ok_or("nominal retirement current path has no structural type")?;
                        if selected.id != cleanup.structural_type {
                            return Err(
                                "nominal retirement cleanup differs from its current typed subject",
                            );
                        }
                        let terminal_psi::StructuralTypeShape::Record { fields, .. } =
                            &declaration.shape
                        else {
                            return Err(
                                "nominal retirement subject has an unsupported descendant shape",
                            );
                        };
                        if fields
                            .iter()
                            .filter(|field| !field.relevance.is_erased())
                            .any(|field| {
                                matches!(
                                    field.field_type,
                                    terminal_psi::StructuralFieldType::Structural(_)
                                        | terminal_psi::StructuralFieldType::ByteSequence(_)
                                        | terminal_psi::StructuralFieldType::PredicateByteSequence { .. }
                                )
                            })
                        {
                            return Err(
                                "nominal retirement subject has an unaccounted structural descendant",
                            );
                        }
                        frontier.retain(|current| current.identity != claim.identity);
                    }
                    TerminalAffineCleanupAction::DiscardRoot(place)
                    | TerminalAffineCleanupAction::RetireHome(place) => {
                        if frontier.iter().any(|claim| claim.place == *place) {
                            return Err(
                                "home retirement or claim-free discard cannot discharge a nominal claim",
                            );
                        }
                    }
                    TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        if frontier.iter().any(|claim| {
                            claim.place == discard.place
                                && crate::structural_paths_may_overlap(&claim.path, &discard.path)
                        }) {
                            return Err(
                                "claim-free residual disposal cannot discharge a nominal claim",
                            );
                        }
                    }
                }
            }
            Ok(())
        }

        fn bind_successor_claims(
            machine: &TerminalMachine,
            target: BlockId,
            arguments: &[StructuralArgument],
            frontier: &mut [ClaimLocation],
        ) -> Result<(), &'static str> {
            let block = machine
                .block(&target)
                .ok_or("nominal retirement successor block is absent")?;
            for claim in frontier {
                let mut matching = arguments.iter().enumerate().filter(|(_, argument)| {
                    argument.access == StructuralAccess::Owned
                        && argument.place == claim.place
                        && claim.path.starts_with(&argument.path)
                });
                let Some((position, argument)) = matching.next() else {
                    continue;
                };
                if matching.next().is_some() {
                    return Err("nominal retirement edge duplicates a live claim");
                }
                let parameter = block
                    .structural_parameters
                    .get(position)
                    .ok_or("nominal retirement edge lacks its target formal")?;
                if parameter.access != StructuralAccess::Owned {
                    return Err("nominal retirement edge changes owned access");
                }
                let path = claim.path[argument.path.len()..].to_vec();
                let mut bindings = machine
                    .block_entry_claims
                    .iter()
                    .filter(|entry| entry.input == parameter.place && entry.path == path);
                if let Some(binding) = bindings.next() {
                    if bindings.next().is_some() {
                        return Err("nominal retirement arrival has ambiguous claim names");
                    }
                    claim.identity = binding.claim;
                }
                claim.place = parameter.place;
                claim.path = path;
            }
            Ok(())
        }

        fn entry_claim_is_retired(
            module: &TerminalModule,
            machine: &TerminalMachine,
            input: ClaimId,
            projection: ContentProjectionIdentity,
            traversal: &mut OriginTraversal,
        ) -> Result<bool, &'static str> {
            traversal.visit()?;
            let key = (machine.id, input);
            if let Some((_, result)) = traversal
                .retired
                .iter()
                .find(|(candidate, _)| *candidate == key)
            {
                return Ok(*result);
            }
            if traversal.retirement_active.contains(&key)
                || traversal.retirement_active.len() >= 128
            {
                return Err(
                    "nominal retirement call graph is circular or exceeds its receiving depth",
                );
            }
            let declaration = machine
                .entry_claims
                .iter()
                .find(|entry| entry.claim == input)
                .ok_or("nominal retirement has no exact original entry claim")?;
            traversal.retirement_active.push(key);
            let result = (|| {
                let mut pending = vec![(
                    machine.entry,
                    vec![ClaimLocation {
                        identity: input,
                        place: declaration.input,
                        path: declaration.path.clone(),
                    }],
                )];
                let mut entered: Vec<(BlockId, Vec<ClaimLocation>)> = Vec::new();
                let mut normal = false;
                while let Some((block, mut frontier)) = pending.pop() {
                    traversal.visit()?;
                    frontier.sort_by_key(|claim| claim.identity);
                    if let Some((_, previous)) =
                        entered.iter().find(|(candidate, _)| *candidate == block)
                    {
                        if previous != &frontier {
                            return Err(
                                "nominal retirement frontier differs across incoming paths",
                            );
                        }
                        continue;
                    }
                    entered.push((block, frontier.clone()));
                    let body = machine
                        .block(&block)
                        .ok_or("nominal retirement control block is absent")?;
                    for current in &body.operations {
                        traversal.visit()?;
                        if !advance_owned_frontier(
                            module,
                            machine,
                            current,
                            &mut frontier,
                            projection,
                            traversal,
                        )? {
                            return Ok(false);
                        }
                    }
                    match &body.terminator {
                        Terminator::Return {
                            cleanup_actions, ..
                        }
                        | Terminator::ReturnUnit {
                            cleanup_actions, ..
                        }
                        | Terminator::ReturnStructural {
                            cleanup_actions, ..
                        } => {
                            if let Terminator::ReturnStructural {
                                returned_claims, ..
                            } = &body.terminator
                                && returned_claims.iter().any(|returned| {
                                    frontier
                                        .iter()
                                        .any(|claim| claim.identity == returned.claim)
                                })
                            {
                                return Ok(false);
                            }
                            apply_nominal_disposals(
                                module,
                                machine,
                                cleanup_actions,
                                &mut frontier,
                            )?;
                            normal = true;
                            if !frontier.is_empty() {
                                return Ok(false);
                            }
                        }
                        Terminator::Jump {
                            target,
                            structural_arguments,
                            cleanup_actions,
                            ..
                        } => {
                            apply_nominal_disposals(
                                module,
                                machine,
                                cleanup_actions,
                                &mut frontier,
                            )?;
                            bind_successor_claims(
                                machine,
                                *target,
                                structural_arguments,
                                &mut frontier,
                            )?;
                            pending.push((*target, frontier));
                        }
                        Terminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => {
                            for edge in [when_true, when_false] {
                                let mut selected = frontier.clone();
                                apply_nominal_disposals(
                                    module,
                                    machine,
                                    &edge.cleanup_actions,
                                    &mut selected,
                                )?;
                                bind_successor_claims(
                                    machine,
                                    edge.target,
                                    &edge.structural_arguments,
                                    &mut selected,
                                )?;
                                pending.push((edge.target, selected));
                            }
                        }
                        Terminator::StructuralCase { cases, .. } => {
                            for edge in cases {
                                let mut selected = frontier.clone();
                                apply_nominal_disposals(
                                    module,
                                    machine,
                                    &edge.cleanup_actions,
                                    &mut selected,
                                )?;
                                bind_successor_claims(
                                    machine,
                                    edge.target,
                                    &edge.structural_arguments,
                                    &mut selected,
                                )?;
                                pending.push((edge.target, selected));
                            }
                        }
                        Terminator::Crash { .. }
                        | Terminator::ExternalCompletion { .. }
                        | Terminator::InvokeNoNormal { .. } => {}
                    }
                }
                if !normal {
                    return Err("nominal retirement has no grounded normal return");
                }
                Ok(true)
            })();
            traversal.retirement_active.pop();
            if let Ok(result) = result {
                traversal.retired.push((key, result));
            }
            result
        }
    }

    type OriginKey = (MachineId, ClaimId, ContentProjectionIdentity);

    pub(crate) struct OriginTraversal {
        active: Vec<OriginKey>,
        completed: Vec<(OriginKey, Vec<ClaimId>)>,
        remaining: usize,
        retirement_active: Vec<(MachineId, ClaimId)>,
        retired: Vec<((MachineId, ClaimId), bool)>,
    }

    impl OriginTraversal {
        fn new() -> Self {
            Self {
                active: Vec::new(),
                completed: Vec::new(),
                remaining: 65_536,
                retirement_active: Vec::new(),
                retired: Vec::new(),
            }
        }

        fn visit(&mut self) -> Result<(), &'static str> {
            self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or("content ancestry receiving work budget exhausted")?;
            Ok(())
        }
    }

    pub(crate) fn validate_frontier(
        derived: &ContentTransformationDerivation,
    ) -> Result<(), &'static str> {
        if derived.lineage_roots.is_empty()
            || derived
                .lineage_roots
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err("content ancestry root roster is empty or not strictly canonical");
        }
        let mut frontier = derived.lineage_roots.clone();
        let mut producers = Vec::new();
        for step in &derived.ancestry {
            if step.inputs.is_empty()
                || step.outputs.is_empty()
                || step.inputs.windows(2).any(|pair| pair[0] >= pair[1])
                || step.outputs.windows(2).any(|pair| pair[0] >= pair[1])
                || producers.contains(&step.producer_operation)
                || !step.inputs.iter().all(|input| frontier.contains(input))
            {
                return Err("content ancestry does not replace a complete known parent roster");
            }
            producers.push(step.producer_operation);
            frontier.retain(|claim| !step.inputs.contains(claim));
            if step.outputs.iter().any(|output| frontier.contains(output)) {
                return Err("content ancestry duplicates a live descendant");
            }
            frontier.extend_from_slice(&step.outputs);
        }
        frontier.sort();
        let mut inputs = derived
            .inputs
            .iter()
            .map(|input| input.claim)
            .collect::<Vec<_>>();
        inputs.sort();
        if frontier != inputs {
            return Err("content ancestry does not end at the consumed claim frontier");
        }
        Ok(())
    }

    pub(crate) fn validate_producers(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
        derivations: &[ContentTransformationDerivation],
    ) -> Result<(), &'static str> {
        let position = |identity: OperationId| {
            machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|operation| operation.id == identity)
                    .map(|ordinal| (block.id, ordinal))
            })
        };
        let consumer = position(operation.id).ok_or("content consumer position is absent")?;
        let dominators = crate::control_graph::dominators(machine);
        let mut traversal = OriginTraversal::new();
        for derived in derivations {
            let projection = derived
                .inputs
                .first()
                .ok_or("content ancestry has no input")?
                .projection;
            for step in &derived.ancestry {
                let producer = position(step.producer_operation)
                    .ok_or("content ancestry producer is absent")?;
                if (producer.0 == consumer.0 && producer.1 >= consumer.1)
                    || !dominators.dominates(producer.0, consumer.0)
                {
                    return Err(
                        "content ancestry producer does not precede the consuming completion",
                    );
                }
                let producer = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == step.producer_operation)
                    .ok_or("content ancestry producer is absent")?;
                authenticate_step(module, producer, projection, step, &mut traversal)?;
            }
        }
        Ok(())
    }

    fn authenticate_step(
        module: &TerminalModule,
        producer: &Operation,
        projection: ContentProjectionIdentity,
        step: &ContentLineageStep,
        traversal: &mut OriginTraversal,
    ) -> Result<(), &'static str> {
        if let OperationKind::TransformContent { derivations, .. } = &producer.kind {
            if derivations.iter().any(|derived| {
                let mut inputs = derived
                    .inputs
                    .iter()
                    .filter(|input| input.projection == projection)
                    .map(|input| input.claim)
                    .collect::<Vec<_>>();
                let mut outputs = derived
                    .outputs
                    .iter()
                    .filter(|output| output.projection == projection)
                    .map(|output| output.claim)
                    .collect::<Vec<_>>();
                inputs.sort();
                outputs.sort();
                inputs == step.inputs && outputs == step.outputs
            }) {
                return Ok(());
            }
            return Err("content ancestry differs from its completed transformation");
        }
        let mut inputs = Vec::new();
        for output in &step.outputs {
            for input in call_input_origins(module, producer, *output, projection, traversal)? {
                if !inputs.contains(&input) {
                    inputs.push(input);
                }
            }
        }
        for input in &step.inputs {
            if !inputs.contains(input)
                && retirement::call_input_is_retired(
                    module, producer, *input, projection, traversal,
                )?
            {
                inputs.push(*input);
            }
        }
        inputs.sort();
        if inputs != step.inputs {
            return Err(
                "content ancestry call inputs differ from its exact returned claim origins",
            );
        }
        Ok(())
    }

    fn call_input_origins(
        module: &TerminalModule,
        operation: &Operation,
        output: ClaimId,
        projection: ContentProjectionIdentity,
        traversal: &mut OriginTraversal,
    ) -> Result<Vec<ClaimId>, &'static str> {
        let callee = operation
            .kind
            .direct_callee()
            .and_then(|identity| {
                module
                    .machines
                    .iter()
                    .find(|machine| machine.id == identity)
            })
            .ok_or("content ancestry requires an authenticated returning callable")?;
        let transfers = operation.kind.claim_transfers();
        let result = operation
            .result
            .as_structural()
            .ok_or("content ancestry call has no structural result")?;
        let output = result
            .claims
            .iter()
            .find(|binding| binding.claim == output)
            .ok_or("content ancestry output is absent from its call result")?;
        let mut origin = None;
        for block in &callee.blocks {
            traversal.visit()?;
            let terminal_psi::Terminator::ReturnStructural {
                returned_claims, ..
            } = &block.terminator
            else {
                continue;
            };
            let returned = returned_claims
                .iter()
                .find(|binding| binding.path == output.path)
                .ok_or("content ancestry result lacks an exact returned-claim correspondence")?;
            let roots = local_roots(module, callee, returned.claim, projection, traversal)?;
            let mut candidate = Vec::new();
            for root in roots {
                let entry = callee
                    .entry_claims
                    .iter()
                    .find(|entry| entry.claim == root)
                    .ok_or("content ancestry callee root is not an owned input")?;
                let parameter = callee
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == entry.input)
                    .ok_or("content ancestry entry has no structural parameter")?;
                let mut matching = transfers.iter().filter(|transfer| {
                    transfer.argument_index as usize == parameter && transfer.path == entry.path
                });
                let transfer = matching
                    .next()
                    .ok_or("content ancestry entry has no exact actual claim transfer")?;
                if matching.next().is_some() {
                    return Err("content ancestry entry has ambiguous actual claim transfers");
                }
                if !candidate.contains(&transfer.claim) {
                    candidate.push(transfer.claim);
                }
            }
            candidate.sort();
            if candidate.is_empty()
                || origin
                    .as_ref()
                    .is_some_and(|previous| previous != &candidate)
            {
                return Err("content ancestry has ambiguous normal-return input rosters");
            }
            origin = Some(candidate);
        }
        origin.ok_or("content ancestry callee has no grounded normal result")
    }

    fn local_roots(
        module: &TerminalModule,
        machine: &TerminalMachine,
        claim: ClaimId,
        projection: ContentProjectionIdentity,
        traversal: &mut OriginTraversal,
    ) -> Result<Vec<ClaimId>, &'static str> {
        traversal.visit()?;
        let key = (machine.id, claim, projection);
        if let Some((_, roots)) = traversal
            .completed
            .iter()
            .find(|(completed, _)| *completed == key)
        {
            return Ok(roots.clone());
        }
        if machine.content_entry_claims.iter().any(|entry| {
            entry.claim == claim
                && entry
                    .projections
                    .iter()
                    .any(|row| row.projection == projection)
        }) {
            return Ok(vec![claim]);
        }
        if traversal.active.contains(&key) {
            return Err("content ancestry is circular rather than grounded in an input");
        }
        if traversal.active.len() >= 128 {
            return Err("content ancestry receiving depth budget exhausted");
        }
        traversal.active.push(key);
        let result = (|| {
            let mut roots = arrivals::roots(module, machine, claim, projection, traversal)?;
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                traversal.visit()?;
                let mut candidates = Vec::new();
                if let OperationKind::TransformContent { derivations, .. } = &operation.kind {
                    for derived in derivations.iter().filter(|derived| {
                        derived
                            .outputs
                            .iter()
                            .any(|output| output.claim == claim && output.projection == projection)
                    }) {
                        if derived.inputs.is_empty() {
                            return Err("content ancestry transformation has no parent");
                        }
                        let mut parents = Vec::new();
                        for input in &derived.inputs {
                            if input.projection != projection {
                                return Err("content ancestry changes qualification owner");
                            }
                            parents.push(input.claim);
                        }
                        candidates.push(parents);
                    }
                } else if operation.kind.direct_callee().is_some()
                    && operation.result.as_structural().is_some_and(|result| {
                        result.claims.iter().any(|output| output.claim == claim)
                    })
                {
                    candidates.push(call_input_origins(
                        module, operation, claim, projection, traversal,
                    )?);
                }
                for parents in candidates {
                    let mut candidate = Vec::new();
                    for parent in parents {
                        for ancestor in local_roots(module, machine, parent, projection, traversal)?
                        {
                            if !candidate.contains(&ancestor) {
                                candidate.push(ancestor);
                            }
                        }
                    }
                    candidate.sort();
                    if candidate.is_empty()
                        || roots
                            .as_ref()
                            .is_some_and(|previous| previous != &candidate)
                    {
                        return Err("content ancestry has conflicting producer input rosters");
                    }
                    roots = Some(candidate);
                }
            }
            roots.ok_or("content ancestry cannot relate the returned qualification to an input")
        })();
        traversal.active.pop();
        if let Ok(roots) = &result {
            traversal.completed.push((key, roots.clone()));
        }
        result
    }
}
pub(crate) mod boundary_correspondence {

    use semantic_vocabulary::StructuralPlaceKind;
    use semantic_vocabulary::{
        ContentPlaceSegment, ContentPlaceVersion, ContentTerm, PlaceId, StructuralTypeId,
    };
    use terminal_psi::{
        ContentClaimAddress, ContentTransformationCustody, ContentTransformationDerivation,
        StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTable, TerminalMachine,
        TerminalModule,
    };

    pub(crate) fn validate_relation(
        derived: &ContentTransformationDerivation,
    ) -> Result<(), &'static str> {
        let ContentTransformationCustody::Boundary {
            requirements,
            addresses,
        } = &derived.custody
        else {
            return Ok(());
        };
        if requirements.is_empty()
            || addresses.is_empty()
            || requirements.windows(2).any(|pair| pair[0] >= pair[1])
            || addresses.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "boundary body custody requires canonical complete public laws and backing paths",
            );
        }
        for requirement in requirements {
            if requirement.substitutions.is_empty()
                || requirement
                    .substitutions
                    .windows(2)
                    .any(|pair| pair[0].source >= pair[1].source)
            {
                return Err("boundary body public-law substitutions are not exact and canonical");
            }
            let mut used = vec![false; requirement.substitutions.len()];
            fn substitute(
                term: &ContentTerm,
                substitutions: &[terminal_psi::ContentBodyProjectionSubstitution],
                used: &mut [bool],
            ) -> Result<ContentTerm, &'static str> {
                match term {
                    ContentTerm::Projection { projection, .. } => {
                        let position = substitutions
                            .iter()
                            .position(|substitution| &substitution.source == term)
                            .ok_or("boundary body public projection has no exact substitution")?;
                        fn same_owner(
                            term: &ContentTerm,
                            expected: semantic_vocabulary::ContentProjectionIdentity,
                        ) -> bool {
                            match term {
                                ContentTerm::Projection {
                                    projection,
                                    subject,
                                } => {
                                    *projection == expected
                                        && subject.version == ContentPlaceVersion::Current
                                }
                                ContentTerm::Separate(children) => {
                                    !children.is_empty()
                                        && children.iter().all(|child| same_owner(child, expected))
                                }
                            }
                        }
                        if !same_owner(&substitutions[position].target, *projection) {
                            return Err(
                                "boundary body substitution changes a projection owner or observation version",
                            );
                        }
                        used[position] = true;
                        Ok(substitutions[position].target.clone())
                    }
                    ContentTerm::Separate(children) => ContentTerm::separate(
                        children
                            .iter()
                            .map(|child| substitute(child, substitutions, used))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                    .map_err(|_| "boundary body substitution repeats a public projection"),
                }
            }
            let equation = semantic_vocabulary::ContentConservation::new(
                requirement.guarantee.conservation.algebra().clone(),
                substitute(
                    requirement.guarantee.conservation.left(),
                    &requirement.substitutions,
                    &mut used,
                )?,
                substitute(
                    requirement.guarantee.conservation.right(),
                    &requirement.substitutions,
                    &mut used,
                )?,
            );
            if used.contains(&false) || equation != derived.conservation {
                return Err(
                    "boundary body custody differs from the complete substituted public law",
                );
            }
        }
        Ok(())
    }

    pub(crate) fn validate_authority(
        module: &TerminalModule,
        machine: &TerminalMachine,
        derived: &ContentTransformationDerivation,
    ) -> Result<(), &'static str> {
        let ContentTransformationCustody::Boundary { requirements, .. } = &derived.custody else {
            return Ok(());
        };
        for requirement in requirements {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|boundary| boundary.id == requirement.boundary)
                .ok_or("boundary body custody requirement is absent")?;
            if !boundary.content_guarantees.iter().any(|guarantee| matches!(guarantee,
                terminal_psi::BoundaryContentGuarantee::Conservation(guarantee) if guarantee == &requirement.guarantee)) || !module
                .provider_candidates
                .iter()
                .any(|candidate| candidate.boundary == boundary.id && candidate.candidate == machine.id)
            {
                return Err(
                    "boundary body custody lacks the exact declared law and checked candidate correspondence",
                );
            }
            for substitution in &requirement.substitutions {
                let ContentTerm::Projection {
                    projection,
                    subject,
                } = &substitution.source
                else {
                    return Err("boundary body substitution source is not one public projection");
                };
                let root = requirement
                    .guarantee
                    .structural_places
                    .iter()
                    .find(|place| place.id == subject.root)
                    .ok_or("boundary body public subject has no declaration")?;
                let roster = match root.kind {
                    StructuralPlaceKind::Parameter { position }
                        if subject.version == ContentPlaceVersion::Entry =>
                    {
                        let boundary_index = boundary
                            .structural_parameters
                            .iter()
                            .position(|parameter| parameter.position == position)
                            .ok_or("boundary body public subject is not a structural formal")?;
                        let anchored = module
                            .provider_candidates
                            .iter()
                            .filter(|candidate| {
                                candidate.boundary == boundary.id
                                    && candidate.candidate == machine.id
                            })
                            .any(|candidate| {
                                candidate
                                    .refinement
                                    .positional_parameters
                                    .iter()
                                    .filter(|row| row.boundary_index as usize == boundary_index)
                                    .any(|row| {
                                        machine
                                            .structural_parameters
                                            .get(row.candidate_index as usize)
                                            .is_some_and(|parameter| {
                                                machine.content_entry_claims.iter().any(|entry| {
                                                    derived.lineage_roots.contains(&entry.claim)
                                                        && entry.input.root == parameter.place
                                                        && entry.input.segments == subject.segments
                                                        && entry.projections.iter().any(|entry| {
                                                            entry.projection == *projection
                                                        })
                                                })
                                            })
                                    })
                            });
                        if !anchored {
                            return Err(
                                "boundary body input law is not anchored to its exact public formal and entry claim",
                            );
                        }
                        &derived.inputs
                    }
                    StructuralPlaceKind::Result
                        if subject.version == ContentPlaceVersion::Current =>
                    {
                        &derived.outputs
                    }
                    _ => {
                        return Err(
                            "boundary body law does not bind an entry input or current result",
                        );
                    }
                };
                fn in_roster(
                    term: &ContentTerm,
                    roster: &[terminal_psi::ContentClaimProjection],
                ) -> bool {
                    match term {
                        ContentTerm::Projection {
                            projection,
                            subject,
                        } => roster.iter().any(|binding| {
                            binding.projection == *projection && binding.subject == *subject
                        }),
                        ContentTerm::Separate(children) => {
                            children.iter().all(|child| in_roster(child, roster))
                        }
                    }
                }
                if !in_roster(&substitution.target, roster) {
                    return Err(
                        "boundary body public subject is substituted into another custody occurrence",
                    );
                }
            }
            for output in &derived.outputs {
                let domain = module
                    .structural_domains
                    .iter()
                    .find(|domain| {
                        domain
                            .content_projection
                            .as_ref()
                            .is_some_and(|projection| projection.identity == output.projection)
                    })
                    .ok_or("boundary body output has no exact domain owner")?;
                if !domain.authorizes_boundary_requirement(
                    boundary.identity,
                    boundary
                        .declaration
                        .as_ref()
                        .map(|checked| checked.source.semantic_declaration),
                ) {
                    return Err(
                        "boundary body public requirement is not authorized for this output domain",
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_backing(
        types: &[StructuralTypeDeclaration],
        identities: &TerminalIdentityTable,
        carrier_of: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
        result: &StructuralOperationResult,
        derived: &ContentTransformationDerivation,
    ) -> Result<(), &'static str> {
        let ContentTransformationCustody::Boundary { addresses, .. } = &derived.custody else {
            return Ok(());
        };
        let mut expected = [Vec::new(), Vec::new()];
        for (position, bindings) in [&derived.inputs, &derived.outputs].into_iter().enumerate() {
            for binding in bindings {
                let root = if position == 0 {
                    carrier_of(binding.subject.root)
                        .ok_or("boundary backing input carrier is absent")?
                } else if binding.subject.root == result.place {
                    result.structural_type
                } else {
                    return Err("boundary backing output names another occurrence");
                };
                let path = super::structural_path(
                    types.iter(),
                    identities,
                    root,
                    &binding.subject.segments,
                )
                .ok_or("boundary backing claim has no exact typed owned path")?;
                let carrier = crate::runtime_structural_path_tip(types.iter(), root, &path)
                    .ok_or("boundary backing claim carrier is absent")?;
                for relative in owned_addresses(types, identities, carrier.id, addresses.len())? {
                    let mut subject = binding.subject.clone();
                    subject.segments.extend(relative);
                    expected[position].push(ContentClaimAddress {
                        claim: binding.claim,
                        subject,
                    });
                }
            }
            expected[position].sort();
            expected[position].dedup();
        }
        let mut actual = [Vec::new(), Vec::new()];
        for address in addresses {
            actual[0].push(address.input.clone());
            actual[1].push(address.output.clone());
        }
        for position in 0..2 {
            actual[position].sort();
            if actual[position].is_empty()
                || actual[position].windows(2).any(|pair| pair[0] == pair[1])
                || actual[position] != expected[position]
            {
                return Err(
                    "boundary custody backing relation is not a bijection of the complete owned address rosters",
                );
            }
        }
        Ok(())
    }

    fn owned_addresses(
        types: &[StructuralTypeDeclaration],
        identities: &TerminalIdentityTable,
        root: StructuralTypeId,
        declared_addresses: usize,
    ) -> Result<Vec<Vec<ContentPlaceSegment>>, &'static str> {
        fn fields(
            types: &[StructuralTypeDeclaration],
            identities: &TerminalIdentityTable,
            declarations: &[StructuralFieldDeclaration],
            path: &mut Vec<ContentPlaceSegment>,
            ancestors: &mut Vec<StructuralTypeId>,
            output: &mut Vec<Vec<ContentPlaceSegment>>,
            declared_addresses: usize,
        ) -> Result<(), &'static str> {
            for field in declarations
                .iter()
                .filter(|field| !field.relevance.is_erased())
            {
                path.push(ContentPlaceSegment::Field(
                    identities
                        .text(field.identity)
                        .ok_or("backing field has no declared identity")?
                        .to_owned(),
                ));
                match &field.field_type {
                    StructuralFieldType::Structural(child) => visit(
                        types,
                        identities,
                        *child,
                        path,
                        ancestors,
                        output,
                        declared_addresses,
                    )?,
                    StructuralFieldType::ByteSequence(_)
                    | StructuralFieldType::PredicateByteSequence { .. } => {
                        return Err("boundary backing contains a borrowed byte descriptor");
                    }
                    StructuralFieldType::Atomic(_) => {
                        return Err("boundary backing contains an atomic cell");
                    }
                    field => {
                        if matches!(field.scalar_type(), Some(semantic_vocabulary::ScalarType::Integer(integer)) if integer.is_address())
                        {
                            output.push(path.clone());
                        }
                    }
                }
                path.pop();
                if output.len() > declared_addresses {
                    return Err("boundary backing omits declared address occurrences");
                }
            }
            Ok(())
        }
        fn visit(
            types: &[StructuralTypeDeclaration],
            identities: &TerminalIdentityTable,
            root: StructuralTypeId,
            path: &mut Vec<ContentPlaceSegment>,
            ancestors: &mut Vec<StructuralTypeId>,
            output: &mut Vec<Vec<ContentPlaceSegment>>,
            declared_addresses: usize,
        ) -> Result<(), &'static str> {
            if ancestors.contains(&root) {
                return Err("boundary backing has recursive owned storage");
            }
            let carrier = types
                .iter()
                .find(|carrier| carrier.id == root)
                .ok_or("boundary backing storage declaration is absent")?;
            ancestors.push(root);
            match &carrier.shape {
                StructuralTypeShape::AtomicCell(_) => {}
                StructuralTypeShape::PrimitiveScalar(scalar) => {
                    if matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.is_address())
                    {
                        output.push(path.clone());
                    }
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    let mut element_paths = Vec::new();
                    visit(
                        types,
                        identities,
                        *element,
                        &mut Vec::new(),
                        ancestors,
                        &mut element_paths,
                        declared_addresses,
                    )?;
                    if !element_paths.is_empty() {
                        let length = &length
                            .literal()
                            .ok_or("boundary backing addresses require a closed array extent")?;
                        let count = usize::try_from(*length)
                            .ok()
                            .and_then(|length| length.checked_mul(element_paths.len()))
                            .and_then(|count| output.len().checked_add(count))
                            .ok_or("boundary backing address count overflows")?;
                        if count > declared_addresses {
                            return Err("boundary backing omits array address occurrences");
                        }
                        for index in 0..*length {
                            for relative in &element_paths {
                                let mut indexed = path.clone();
                                indexed.push(ContentPlaceSegment::FixedIndex(index));
                                indexed.extend_from_slice(relative);
                                output.push(indexed);
                            }
                        }
                    }
                }
                StructuralTypeShape::Record {
                    fields: declarations,
                    ..
                } => fields(
                    types,
                    identities,
                    declarations,
                    path,
                    ancestors,
                    output,
                    declared_addresses,
                )?,
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    if let StructuralTypeShape::Mixed {
                        fields: declarations,
                        ..
                    } = &carrier.shape
                    {
                        fields(
                            types,
                            identities,
                            declarations,
                            path,
                            ancestors,
                            output,
                            declared_addresses,
                        )?;
                    }
                    for case in cases {
                        path.push(ContentPlaceSegment::Case(
                            identities
                                .text(case.identity)
                                .ok_or("backing case has no declared identity")?
                                .to_owned(),
                        ));
                        fields(
                            types,
                            identities,
                            &case.fields,
                            path,
                            ancestors,
                            output,
                            declared_addresses,
                        )?;
                        path.pop();
                    }
                }
                StructuralTypeShape::Opaque
                | StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::ElementView { .. } => {
                    return Err("boundary backing is not a closed owned storage shape");
                }
            }
            ancestors.pop();
            if output.len() > declared_addresses {
                return Err("boundary backing omits declared address occurrences");
            }
            Ok(())
        }
        let mut output = Vec::new();
        visit(
            types,
            identities,
            root,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut output,
            declared_addresses,
        )?;
        Ok(output)
    }
}

pub fn structural_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    segments: &[ContentPlaceSegment],
) -> Option<Vec<StructuralPathSegment>> {
    let mut path = Vec::new();
    for segment in segments {
        path.push(match segment {
            ContentPlaceSegment::Field(identity) => {
                StructuralPathSegment::Field(identities.index_of(identity)?)
            }
            ContentPlaceSegment::FixedIndex(position) => {
                StructuralPathSegment::FixedIndex(*position)
            }
            ContentPlaceSegment::Case(identity) => {
                let carrier = runtime_structural_path_tip(types.clone(), root, &path)?;
                let identity = identities.index_of(identity)?;
                StructuralPathSegment::Case(
                    carrier
                        .shape
                        .sum_cases()?
                        .iter()
                        .find(|case| case.identity == identity)?
                        .id,
                )
            }
        });
    }
    runtime_structural_path_tip(types, root, &path)?;
    Some(path)
}

pub fn content_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    let mut segments = Vec::new();
    for (position, segment) in path.iter().enumerate() {
        segments.push(match segment {
            StructuralPathSegment::Field(identity) => {
                ContentPlaceSegment::Field(identities.text(*identity)?.to_owned())
            }
            StructuralPathSegment::FixedIndex(position) => {
                ContentPlaceSegment::FixedIndex(*position)
            }
            StructuralPathSegment::Case(identity) => {
                let carrier = runtime_structural_path_tip(types.clone(), root, &path[..position])?;
                let case = carrier
                    .shape
                    .sum_cases()?
                    .iter()
                    .find(|case| case.id == *identity)?;
                ContentPlaceSegment::Case(identities.text(case.identity)?.to_owned())
            }
            _ => return None,
        });
    }
    runtime_structural_path_tip(types, root, path)?;
    Some(segments)
}

pub fn claim_term(bindings: &[ContentClaimProjection]) -> Result<ContentTerm, &'static str> {
    if bindings.is_empty()
        || bindings
            .iter()
            .any(|binding| binding.subject.version != ContentPlaceVersion::Current)
    {
        return Err("content transformation requires current claim projections");
    }
    let mut terms = bindings
        .iter()
        .map(|binding| ContentTerm::Projection {
            projection: binding.projection,
            subject: binding.subject.clone(),
        })
        .collect::<Vec<_>>();
    if terms.len() == 1 {
        Ok(terms.remove(0))
    } else {
        ContentTerm::separate(terms).map_err(|_| "content transformation repeats a projection")
    }
}

pub fn place_type(
    machine: &terminal_psi::TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameter(place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .producing_operation(place)
                .map(|(_, carrier)| carrier)
        })
}

pub fn validate_shape(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), &'static str> {
    use terminal_psi::OperationKind;
    let OperationKind::TransformContent {
        source,
        parents,
        derivations,
    } = &operation.kind
    else {
        return Err("content transformation dispatch differs");
    };
    let result = operation
        .result
        .as_structural()
        .ok_or("content transformation requires a structural result")?;
    validate_contract(
        &module.structural_types,
        &module.structural_domains,
        &module.identities,
        &|place| place_type(machine, place),
        source,
        parents,
        derivations,
        result,
    )?;
    ancestry::validate_producers(module, machine, operation, derivations)?;
    for derived in derivations {
        boundary_correspondence::validate_authority(module, machine, derived)?;
    }
    let position = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation.id)
                .and_then(|ordinal| {
                    u32::try_from(ordinal)
                        .ok()
                        .map(|ordinal| (block.id, ordinal))
                })
        })
        .ok_or("content transformation position is absent")?;
    let graph = crate::descriptors::descriptor_results::terminal_graph(module);
    let contract = crate::descriptors::descriptor_results::current_selected_payload_contract(
        &module.structural_types,
        &module.structural_domains,
        &graph,
        machine.id,
        position,
        source.place,
        &source.path,
        &|place| crate::descriptors::descriptor_results::terminal_payload_contract(machine, place),
    )?;
    validate_memberships(
        &module.structural_types,
        &module.structural_domains,
        &module.identities,
        result,
        derivations,
        &contract.qualifications,
        &contract.projected_qualifications,
    )?;
    Ok(())
}

fn projection_owner<'domains>(
    domains: &'domains [StructuralDomainDeclaration],
    carrier: StructuralTypeId,
    projection: &ContentProjectionIdentity,
    derived: &ContentTransformationDerivation,
) -> Option<&'domains StructuralDomainDeclaration> {
    let algebra = derived.conservation.algebra();
    domains.iter().find(|domain| {
        let Some(declared) = &domain.content_projection else {
            return false;
        };
        domain.carrier == carrier
            && declared.identity == *projection
            && declared.algebra == *algebra
    })
}

pub fn validate_parent_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    root_type: StructuralTypeId,
    parent: &StructuralArgument,
    derivations: &[ContentTransformationDerivation],
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    inactive: &impl Fn(&[StructuralPathSegment]) -> bool,
) -> Result<(), &'static str> {
    let mut actual = Vec::new();
    for (path, identity) in whole.iter().map(|domain| (&[][..], domain)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), &row.domain)),
    ) {
        let domain = domains
            .iter()
            .find(|domain| domain.id == *identity)
            .ok_or("content parent membership owner is absent")?;
        if domain.content_projection.is_some() && !inactive(path) {
            actual.push(StructuralPathQualification {
                path: path.to_vec(),
                domain: *identity,
            });
        }
    }
    let mut expected = Vec::new();
    for derived in derivations {
        for input in &derived.inputs {
            if input.subject.root != parent.place {
                continue;
            }
            let path =
                structural_path(types.iter(), identities, root_type, &input.subject.segments)
                    .ok_or("content parent membership has no typed path")?;
            let Some(relative) = path.strip_prefix(parent.path.as_slice()) else {
                continue;
            };
            let carrier = runtime_structural_path_tip(types.iter(), root_type, &path)
                .ok_or("content parent membership carrier is absent")?;
            let domain = projection_owner(domains, carrier.id, &input.projection, derived)
                .ok_or("content parent membership has no exact qualification owner")?;
            expected.push(StructuralPathQualification {
                path: relative.to_vec(),
                domain: domain.id,
            });
        }
    }
    actual.sort();
    actual.dedup();
    expected.sort();
    expected.dedup();
    if actual != expected {
        return Err("content inputs differ from the parent's complete current membership roster");
    }
    Ok(())
}

pub fn validate_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    carrier_of: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
    source: &StructuralArgument,
    parents: &[StructuralArgument],
    derivations: &[ContentTransformationDerivation],
    result: &StructuralOperationResult,
) -> Result<(), &'static str> {
    use terminal_psi::StructuralAccess;
    if parents.is_empty()
        || derivations.is_empty()
        || !result.qualification_establishments.is_empty()
    {
        return Err("content transformation requires parents and equations, not an issuance route");
    }
    if derivations.windows(2).any(|pair| pair[0] >= pair[1])
        || derivations.iter().any(|derived| {
            derived.inputs.windows(2).any(|pair| pair[0] >= pair[1])
                || derived.outputs.windows(2).any(|pair| pair[0] >= pair[1])
        })
    {
        return Err("content transformation rosters are not strictly canonical");
    }
    let arguments = std::iter::once(source).chain(parents).collect::<Vec<_>>();
    for (position, argument) in arguments.iter().enumerate() {
        if argument.access != StructuralAccess::Owned
            || argument.place == result.place
            || arguments[..position].iter().any(|previous| {
                previous.place == argument.place
                    && crate::structural_paths_may_overlap(&previous.path, &argument.path)
            })
        {
            return Err("content transformation requires disjoint owned operands");
        }
    }
    let source_type = carrier_of(source.place).ok_or("content payload has no carrier")?;
    if runtime_structural_path_tip(types.iter(), source_type, &source.path)
        .is_none_or(|carrier| carrier.id != result.structural_type)
    {
        return Err("content transformation changes payload carrier");
    }
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut covered_parents = vec![false; parents.len()];
    for derived in derivations {
        let mut qualification_owner = None;
        let ordinary = matches!(derived.custody, ContentTransformationCustody::Derived);
        boundary_correspondence::validate_relation(derived)?;
        boundary_correspondence::validate_backing(types, identities, carrier_of, result, derived)?;
        let equation = semantic_vocabulary::ContentConservation::new(
            derived.conservation.algebra().clone(),
            claim_term(&derived.inputs)?,
            claim_term(&derived.outputs)?,
        );
        ancestry::validate_frontier(derived)?;
        if equation != derived.conservation {
            return Err(
                "content equation or authenticated lineage differs from its claim projections",
            );
        }
        for (is_input, bindings) in [(true, &derived.inputs), (false, &derived.outputs)] {
            for binding in bindings {
                let roster = if is_input { &mut inputs } else { &mut outputs };
                if roster.contains(&(binding.claim, binding.projection)) {
                    return Err("content transformation repeats a claim projection");
                }
                roster.push((binding.claim, binding.projection));
                let root_type = if is_input {
                    carrier_of(binding.subject.root).ok_or("content parent has no carrier")?
                } else if binding.subject.root == result.place {
                    result.structural_type
                } else {
                    return Err("content output names another occurrence");
                };
                let path = structural_path(
                    types.iter(),
                    identities,
                    root_type,
                    &binding.subject.segments,
                )
                .ok_or("content claim projection has no exact typed path")?;
                let carrier = runtime_structural_path_tip(types.iter(), root_type, &path)
                    .ok_or("content claim projection carrier is absent")?;
                let domain = projection_owner(domains, carrier.id, &binding.projection, derived)
                    .ok_or("content projection differs from its qualification owner")?;
                let owner = (domain.id, domain.semantic_domain, binding.projection);
                if ordinary && qualification_owner.is_some_and(|previous| previous != owner) {
                    return Err("content equation combines different qualification owners");
                }
                qualification_owner = Some(owner);
                if is_input {
                    let position = parents
                        .iter()
                        .position(|parent| {
                            parent.place == binding.subject.root && path.starts_with(&parent.path)
                        })
                        .ok_or("content input is outside every consumed parent")?;
                    covered_parents[position] = true;
                } else {
                    if !result
                        .claims
                        .iter()
                        .any(|claim| claim.claim == binding.claim && claim.path == path)
                    {
                        return Err("content output differs from the existing result claim");
                    }
                    if ordinary
                        && if path.is_empty() {
                            !result.qualifications.contains(&domain.id)
                        } else {
                            !result
                                .projected_qualifications
                                .iter()
                                .any(|row| row.path == path && row.domain == domain.id)
                        }
                    {
                        return Err("content output membership is absent from the result");
                    }
                }
            }
        }
    }
    if covered_parents.contains(&false)
        || inputs
            .iter()
            .any(|(claim, _)| outputs.iter().any(|(output, _)| claim == output))
    {
        return Err("content transformation omits a parent or both consumes and returns one claim");
    }
    Ok(())
}

pub fn validate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    result: &StructuralOperationResult,
    derivations: &[ContentTransformationDerivation],
    inherited: &[StructuralDomainId],
    inherited_projected: &[StructuralPathQualification],
) -> Result<(), &'static str> {
    let mut whole = inherited.to_vec();
    let mut projected = inherited_projected.to_vec();
    for derived in derivations {
        if !matches!(derived.custody, ContentTransformationCustody::Derived) {
            continue;
        }
        for output in &derived.outputs {
            if output.subject.root != result.place {
                return Err("content output names another occurrence");
            }
            let path = structural_path(
                types.iter(),
                identities,
                result.structural_type,
                &output.subject.segments,
            )
            .ok_or("content output membership has no exact typed path")?;
            let carrier = runtime_structural_path_tip(types.iter(), result.structural_type, &path)
                .ok_or("content output membership carrier is absent")?;
            let domain = projection_owner(domains, carrier.id, &output.projection, derived)
                .ok_or("content output membership has no qualification owner")?;
            if path.is_empty() {
                if whole.contains(&domain.id) {
                    return Err("content output already carries its derived membership");
                }
                whole.push(domain.id);
            } else {
                let membership = StructuralPathQualification {
                    path,
                    domain: domain.id,
                };
                if projected.contains(&membership) {
                    return Err("content output already carries its derived membership");
                }
                projected.push(membership);
            }
        }
    }
    whole.sort();
    projected.sort();
    if whole != result.qualifications || projected != result.projected_qualifications {
        return Err("content result memberships differ from inherited and derived occurrences");
    }
    Ok(())
}

pub mod domain_preservation {
    use crate::exactly_one::exactly_one;
    use crate::projected_field_declaration;
    use crate::structural_paths_may_overlap;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment as Segment, ContentProjectionExpression,
        ContentProjectionScalar, PlaceId, Proposition, ScalarReadProjection, ScalarTerm,
        ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        StructuralDomainDeclaration, StructuralDomainPredicate, StructuralPathQualification,
        StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
    };

    fn term_dependencies<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        subject: PlaceId,
        term: &ScalarTerm,
        output: &mut Vec<Vec<Segment>>,
        depth: usize,
    ) -> Option<()> {
        if depth > 256 {
            return None;
        }
        let observation = match term {
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {
                return Some(());
            }
            ScalarTerm::BooleanField { root, path } => (*root, path.clone(), ScalarType::Boolean),
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => (*root, path.clone(), ScalarType::Integer(*scalar_type)),
            ScalarTerm::IeeeFloatField { root, path, format } => {
                (*root, path.clone(), ScalarType::IeeeFloat(*format))
            }
            ScalarTerm::ByteSequencePredicate { root, path, .. } => {
                if *root != subject {
                    return None;
                }
                let selected = crate::canonical_structural_path_tip(types.clone(), carrier, path)?;
                if !crate::byte_predicate_carrier(types, selected.id) {
                    return None;
                }
                output.push(
                    if matches!(
                        selected.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { .. }
                    ) {
                        path.clone()
                    } else {
                        Vec::new()
                    },
                );
                return Some(());
            }
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                let path = path
                    .iter()
                    .map(|step| match step {
                        ScalarReadProjection::Static(step) => Some(*step),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                (*root, path, *scalar_type)
            }
            semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                return term_dependencies(types, carrier, subject, operand, output, depth + 1);
            }
            semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                term_dependencies(types.clone(), carrier, subject, left, output, depth + 1)?;
                return term_dependencies(types, carrier, subject, right, output, depth + 1);
            }
            _ => return None,
        };
        if observation.0 != subject
            || crate::primitive_read_projection_type(
                types.clone(),
                carrier,
                &storage_path(types, carrier, &observation.1)?,
            )? != observation.2
        {
            return None;
        }
        output.push(observation.1);
        Some(())
    }

    fn predicate_dependencies<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        predicate: &StructuralDomainPredicate,
    ) -> Option<Vec<Vec<Segment>>> {
        fn visit<'a>(
            types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
            carrier: StructuralTypeId,
            subject: PlaceId,
            clause: &Proposition,
            output: &mut Vec<Vec<Segment>>,
            depth: usize,
        ) -> Option<()> {
            if depth > 256 {
                return None;
            }
            match clause {
                Proposition::Truth | Proposition::Falsehood => Some(()),
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    let mut pending = vec![(left, depth + 1), (right, depth + 1)];
                    while let Some((term, depth)) = pending.pop() {
                        if depth > 256 {
                            return None;
                        }
                        match term {
                            semantic_vocabulary::IntegerMathTerm::Embed(scalar) => {
                                term_dependencies(
                                    types.clone(),
                                    carrier,
                                    subject,
                                    scalar,
                                    output,
                                    depth + 1,
                                )?
                            }
                            semantic_vocabulary::IntegerMathTerm::MathValue { .. } => return None,
                            term => pending.extend(
                                term.operands()
                                    .into_iter()
                                    .map(|operand| (operand, depth + 1)),
                            ),
                        }
                    }
                    Some(())
                }
                Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right)
                | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                    term_dependencies(types.clone(), carrier, subject, left, output, 0)?;
                    term_dependencies(types, carrier, subject, right, output, 0)
                }
                Proposition::IeeeFloatComparison {
                    format,
                    left,
                    right,
                    ..
                } => {
                    for field in [left, right] {
                        term_dependencies(
                            types.clone(),
                            carrier,
                            subject,
                            &ScalarTerm::ieee_float_field_path(
                                field.root(),
                                field.path().to_vec(),
                                *format,
                            ),
                            output,
                            depth + 1,
                        )?;
                    }
                    Some(())
                }
                Proposition::Conjunction(clauses) | Proposition::Disjunction(clauses) => {
                    for clause in clauses {
                        visit(types.clone(), carrier, subject, clause, output, depth + 1)?;
                    }
                    Some(())
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    visit(types.clone(), carrier, subject, premise, output, depth + 1)?;
                    visit(types, carrier, subject, conclusion, output, depth + 1)
                }
                Proposition::StructuralCaseMembership {
                    subject: observed,
                    case,
                } => {
                    if observed.root() != subject {
                        return None;
                    }
                    crate::canonical_structural_path_tip(types, carrier, observed.path())?
                        .shape
                        .sum_cases()?
                        .iter()
                        .any(|declared| declared.id == *case)
                        .then_some(())?;
                    output.push(observed.path().to_vec());
                    Some(())
                }
                _ => None,
            }
        }
        let mut output = Vec::new();
        predicate.proposition.validate().ok()?;
        visit(
            types,
            carrier,
            predicate.subject,
            &predicate.proposition,
            &mut output,
            0,
        )?;
        Some(output)
    }

    pub fn domain_predicate_is_closed<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        predicate: &StructuralDomainPredicate,
    ) -> bool {
        predicate_dependencies(types, carrier, predicate).is_some()
    }

    pub fn domain_predicate_observes_no_storage<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        predicate: &StructuralDomainPredicate,
    ) -> bool {
        predicate_dependencies(types, carrier, predicate).is_some_and(|paths| paths.is_empty())
    }

    fn storage_path<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        path: &[Segment],
    ) -> Option<Vec<StructuralPathSegment>> {
        if path.contains(&Segment::Referent) {
            return None;
        }
        if let Some((Segment::Field(field), prefix)) = path.split_last() {
            let (sum, case) = match prefix.split_last() {
                Some((Segment::Case(case), sum)) => (sum, Some(*case)),
                _ => (prefix, None),
            };
            let mut storage =
                crate::canonical_structural_storage_path(types.clone(), carrier, sum)?;
            storage.extend(case.map(StructuralPathSegment::Case));
            let field = crate::observed_field_declaration(types, carrier, prefix, *field)?;
            storage.push(StructuralPathSegment::Field(field.identity));
            Some(storage)
        } else {
            crate::canonical_structural_storage_path(types, carrier, path)
        }
    }

    fn content_paths<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        carrier: StructuralTypeId,
        scalar: &ContentProjectionScalar,
        output: &mut Vec<Vec<Segment>>,
        depth: usize,
    ) -> Option<()> {
        if depth > 256 {
            return None;
        }
        match scalar {
            ContentProjectionScalar::Natural(value) => (!value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0')))
            .then_some(()),
            ContentProjectionScalar::Successor(value) => {
                content_paths(types, identities, carrier, value, output, depth + 1)
            }
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                content_paths(types.clone(), identities, carrier, left, output, depth + 1)?;
                content_paths(types, identities, carrier, right, output, depth + 1)
            }
            ContentProjectionScalar::SubjectField(path)
            | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                let mut current = carrier;
                let mut canonical = Vec::new();
                for (index, identity) in path.iter().enumerate() {
                    let declaration =
                        exactly_one(types.clone(), |declaration| declaration.id == current)?;
                    let field = exactly_one(declaration.shape.direct_fields()?.iter(), |field| {
                        identities.text(field.identity) == Some(identity.as_str())
                            && !field.relevance.is_erased()
                    })?;
                    canonical.push(Segment::Field(field.id));
                    if index + 1 < path.len() {
                        current = match field.field_type {
                            terminal_psi::StructuralFieldType::Structural(child) => child,
                            _ => return None,
                        };
                    } else if field.field_type.scalar_type().is_none() {
                        return None;
                    }
                }
                if canonical.is_empty() {
                    return None;
                }
                output.push(canonical);
                Some(())
            }
        }
    }

    pub fn domain_write_is_disjoint<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        domain: &StructuralDomainDeclaration,
        write: &[StructuralPathSegment],
    ) -> bool {
        if write.iter().any(|step| {
            matches!(
                step,
                StructuralPathSegment::Referent | StructuralPathSegment::RepresentationView { .. }
            )
        }) {
            return false;
        }
        let Some(predicate) = &domain.predicate else {
            return false;
        };
        let Some(mut paths) = predicate_dependencies(types.clone(), domain.carrier, predicate)
        else {
            return false;
        };
        content_dependencies(types.clone(), identities, domain, &mut paths).is_some()
            && paths.iter().all(|path| {
                storage_path(types.clone(), domain.carrier, path)
                    .is_some_and(|dependency| !structural_paths_may_overlap(&dependency, write))
            })
    }

    fn content_dependencies<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        domain: &StructuralDomainDeclaration,
        paths: &mut Vec<Vec<Segment>>,
    ) -> Option<()> {
        if let Some(projection) = &domain.content_projection {
            let valid = match &projection.expression {
                ContentProjectionExpression::CountedQuantity(scalar) => {
                    content_paths(types.clone(), identities, domain.carrier, scalar, paths, 0)
                }
                ContentProjectionExpression::IntervalSet(intervals) => {
                    intervals.iter().try_for_each(|(start, end)| {
                        content_paths(types.clone(), identities, domain.carrier, start, paths, 0)?;
                        content_paths(types.clone(), identities, domain.carrier, end, paths, 0)
                    })
                }
            };
            valid?;
        }
        Some(())
    }

    fn domain_content_write_is_disjoint<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        domain: &StructuralDomainDeclaration,
        write: &[StructuralPathSegment],
    ) -> bool {
        if domain.content_projection.is_none() {
            return true;
        }
        if write.iter().any(|step| {
            matches!(
                step,
                StructuralPathSegment::Referent | StructuralPathSegment::RepresentationView { .. }
            )
        }) {
            return false;
        }
        let mut dependencies = Vec::new();
        content_dependencies(types.clone(), identities, domain, &mut dependencies).is_some()
            && dependencies.iter().all(|dependency| {
                storage_path(types.clone(), domain.carrier, dependency)
                    .is_some_and(|dependency| !structural_paths_may_overlap(&dependency, write))
            })
    }

    pub fn store_preserves_qualifications<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[StructuralPathQualification],
        write: &[StructuralPathSegment],
    ) -> bool {
        let preserved = |id, path: &[StructuralPathSegment]| {
            let mut matching = domains.clone().filter(|domain| domain.id == id);
            matching.next().is_some_and(|domain| {
                matching.next().is_none()
                    && domain_write_is_disjoint(types.clone(), identities, domain, path)
            })
        };
        whole.iter().all(|domain| preserved(*domain, write))
            && projected.iter().all(|row| {
                if !structural_paths_may_overlap(&row.path, write) {
                    return true;
                }
                write
                    .strip_prefix(row.path.as_slice())
                    .is_some_and(|relative| preserved(row.domain, relative))
            })
    }

    pub fn store_preserves_domain_content<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[StructuralPathQualification],
        write: &[StructuralPathSegment],
    ) -> bool {
        let preserved = |identity, path: &[StructuralPathSegment]| {
            let mut matching = domains.clone().filter(|domain| domain.id == identity);
            matching.next().is_some_and(|domain| {
                matching.next().is_none()
                    && domain_content_write_is_disjoint(types.clone(), identities, domain, path)
            })
        };
        whole.iter().all(|domain| preserved(*domain, write))
            && projected.iter().all(|row| {
                !structural_paths_may_overlap(&row.path, write)
                    || write
                        .strip_prefix(row.path.as_slice())
                        .is_some_and(|relative| preserved(row.domain, relative))
            })
    }

    pub fn scalar_field_store_preserves_domain_content<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        root: StructuralTypeId,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[StructuralPathQualification],
        path: &[StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> bool {
        let Some(declaration) = projected_field_declaration(types.clone(), root, path, field)
        else {
            return false;
        };
        let mut write = path.to_vec();
        write.push(StructuralPathSegment::Field(declaration.identity));
        store_preserves_domain_content(types, domains, identities, whole, projected, &write)
    }

    pub fn reference_preserves_ancestor_qualifications<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[StructuralPathQualification],
        path: &[StructuralPathSegment],
        access: terminal_psi::StructuralAccess,
    ) -> bool {
        if access == terminal_psi::StructuralAccess::SharedBorrow || path.is_empty() {
            return true;
        }
        let ancestors = projected
            .iter()
            .filter(|row| {
                row.path.len() < path.len() && structural_paths_may_overlap(&row.path, path)
            })
            .cloned()
            .collect::<Vec<_>>();
        store_preserves_qualifications(types, domains, identities, whole, &ancestors, path)
    }

    pub fn scalar_field_store_preserves_content<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        root: StructuralTypeId,
        place: PlaceId,
        claims: impl Iterator<Item = &'a terminal_psi::ContentEntryClaim>,
        path: &[StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> bool {
        let Some(declaration) = projected_field_declaration(types.clone(), root, path, field)
        else {
            return false;
        };
        let mut write = path.to_vec();
        write.push(StructuralPathSegment::Field(declaration.identity));
        store_preserves_content(types, domains, identities, root, place, claims, &write)
    }

    pub fn store_preserves_content<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
        identities: &TerminalIdentityTable,
        root: StructuralTypeId,
        place: PlaceId,
        claims: impl Iterator<Item = &'a terminal_psi::ContentEntryClaim>,
        write: &[StructuralPathSegment],
    ) -> bool {
        claims
            .filter(|claim| claim.input.root == place)
            .all(|claim| {
                if claim.projections.is_empty() {
                    return false;
                }
                if write.iter().any(|step| {
                    matches!(
                        step,
                        StructuralPathSegment::Referent
                            | StructuralPathSegment::RepresentationView { .. }
                    )
                }) {
                    return false;
                }
                let Some(input) = crate::content_transformations::structural_path(
                    types.clone(),
                    identities,
                    root,
                    &claim.input.segments,
                ) else {
                    return false;
                };
                if !structural_paths_may_overlap(&input, write) {
                    return true;
                }
                let Some(relative) = write.strip_prefix(input.as_slice()) else {
                    return false;
                };
                let Some(carrier) = crate::runtime_structural_path_tip(types.clone(), root, &input)
                else {
                    return false;
                };
                claim.projections.iter().all(|projection| {
                    let mut owners = domains.clone().filter(|domain| {
                        domain.content_projection.as_ref().is_some_and(|owner| {
                            owner.identity == projection.projection
                                && owner.algebra == projection.algebra
                        })
                    });
                    owners.next().is_some_and(|owner| {
                        owners.next().is_none()
                            && owner.carrier == carrier.id
                            && domain_content_write_is_disjoint(
                                types.clone(),
                                identities,
                                owner,
                                relative,
                            )
                    })
                })
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, StructuralCaseId, StructuralCaseSubject,
            StructuralFieldId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralTypeShape, TerminalIdentityIndex,
        };

        #[test]
        fn case_tests_and_payload_reads_close_over_the_subjects_declared_case() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let scalar = StructuralTypeId::new(1).unwrap();
            let carrier = StructuralTypeId::new(2).unwrap();
            let subject = PlaceId::new(1).unwrap();
            let low = StructuralFieldId::new(1).unwrap();
            let case_id = |id| StructuralCaseId::new(id).unwrap();
            let declaration = |id, shape| StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id,
                identity: TerminalIdentityIndex(id.get() as u32),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: true,
                shape,
                scalar_restriction: None,
            };
            let types = [
                declaration(
                    scalar,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
                ),
                declaration(
                    carrier,
                    StructuralTypeShape::Sum {
                        cases: vec![
                            StructuralCaseDeclaration {
                                id: case_id(1),
                                identity: TerminalIdentityIndex(3),
                                fields: Vec::new(),
                            },
                            StructuralCaseDeclaration {
                                id: case_id(2),
                                identity: TerminalIdentityIndex(4),
                                fields: vec![StructuralFieldDeclaration {
                                    inline_carrier: None,
                                    id: low,
                                    identity: TerminalIdentityIndex(5),
                                    relevance: BindingRelevance::Relevant,
                                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                        integer,
                                    )),
                                    storage: None,
                                }],
                            },
                        ],
                    },
                ),
            ];
            let predicate = |proposition| StructuralDomainPredicate {
                subject,
                proposition,
            };
            let closed = |proposition| {
                domain_predicate_is_closed(types.iter(), carrier, &predicate(proposition))
            };
            let membership = |root, case| Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, Vec::new()),
                case: case_id(case),
            };
            let payload_at_most_five = |root, case| {
                Proposition::LessOrEqual(
                    ScalarTerm::integer_field_path(
                        root,
                        vec![Segment::Case(case_id(case)), Segment::Field(low)],
                        integer,
                    ),
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap(),
                )
            };

            assert!(closed(membership(subject, 2)));
            assert!(!domain_predicate_observes_no_storage(
                types.iter(),
                carrier,
                &predicate(membership(subject, 2))
            ));
            assert!(closed(payload_at_most_five(subject, 2)));

            let other = PlaceId::new(2).unwrap();
            assert!(!closed(membership(other, 2)));
            assert!(!closed(membership(subject, 3)));
            assert!(!closed(payload_at_most_five(other, 2)));
            assert!(!closed(payload_at_most_five(subject, 1)));
            assert!(!closed(payload_at_most_five(subject, 3)));
        }
    }
}

pub mod quotients {
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        Operation, OperationKind, StructuralMultiplicity, TerminalMachine, TerminalModule,
        TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense,
    };

    pub(crate) mod opacity {
        use super::*;
        fn quotient_path_tip(
            module: &TerminalModule,
            ty: StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> Result<Option<StructuralTypeId>, &'static str> {
            use terminal_psi::{StructuralPathSegment as P, StructuralTypeShape as S};
            let opaque = |ty| {
                module
                    .structural_types
                    .iter()
                    .any(|row| row.id == ty && row.quotient_carrier.is_some())
            };
            let (path, window) = match path.split_last() {
                Some((P::FixedRange { .. }, prefix)) => (prefix, true),
                _ => (path, false),
            };
            let tip = crate::structural::primitive_place::projection_tip_with_policy(
                module.structural_types.iter(),
                ty,
                path,
                |source, segment| {
                    !source.is_some_and(opaque)
                        && match segment {
                            P::RepresentationView {
                                structural_type, ..
                            } => {
                                !source
                                    .is_some_and(|source| contains_quotient_storage(module, source))
                                    && !contains_quotient_storage(module, *structural_type)
                            }
                            _ => true,
                        }
                },
            )
            .ok_or("structural projection is malformed or crosses an opaque quotient")?;
            if window {
                let selected = tip
                    .and_then(|id| module.structural_types.iter().find(|row| row.id == id))
                    .ok_or("quotient window has no declared backing")?;
                if opaque(selected.id) || !matches!(selected.shape, S::FixedArray { .. }) {
                    return Err("quotient window does not select ordinary array backing");
                }
            }
            Ok(tip)
        }

        fn contains_quotient_storage(module: &TerminalModule, root: StructuralTypeId) -> bool {
            use terminal_psi::{StructuralFieldType as F, StructuralTypeShape as S};
            let children = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
                fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .filter_map(|field| {
                        if let F::Structural(child) = field.field_type {
                            Some(child)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let mut pending = vec![root];
            let mut seen = Vec::new();
            while let Some(id) = pending.pop() {
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id);
                let Some(declaration) = module.structural_types.iter().find(|row| row.id == id)
                else {
                    return true;
                };
                if declaration.quotient_carrier.is_some() {
                    return true;
                }
                match &declaration.shape {
                    S::Record { fields, .. } => pending.extend(children(fields)),
                    S::Sum { cases } => {
                        for case in cases {
                            pending.extend(children(&case.fields));
                        }
                    }
                    S::Mixed { fields, cases } => {
                        pending.extend(children(fields));
                        for case in cases {
                            pending.extend(children(&case.fields));
                        }
                    }
                    S::FixedArray { element, .. } => pending.push(*element),
                    S::Reference { .. }
                    | S::ElementView { .. }
                    | S::ByteSequence(_)
                    | S::PrimitiveScalar(_) => {}
                    S::AtomicCell(_) | S::Opaque => {}
                }
            }
            false
        }

        pub(crate) fn validate_quotient_opacity(
            module: &TerminalModule,
        ) -> Result<(), &'static str> {
            if module.quotients.representations.is_empty() {
                return Ok(());
            }
            for machine in &module.machines {
                for operation in machine.operations() {
                    if let Some(result) = operation.result.structural()
                        && module.structural_types.iter().any(|ty| {
                            ty.id == result.structural_type && ty.quotient_carrier.is_some()
                        })
                    {
                        let transport = matches!(
                            &operation.kind,
                            OperationKind::StructuralLeafCopy { .. }
                                | OperationKind::MoveStructuralValue { .. }
                                | OperationKind::StructuralCaseLeafCopy { .. }
                                | OperationKind::CallStructural { .. }
                                | OperationKind::CallStructuralWithScalarArguments { .. }
                        );
                        if !transport {
                            return Err(
                                "only licensed minting or whole transport can produce a quotient",
                            );
                        }
                    }
                    if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind {
                        let root = quotient_place_type(machine, *source)
                            .ok_or("quotient copy source is absent")?;
                        let tip = quotient_path_tip(module, root, path)?;
                        let result = operation
                            .result
                            .structural()
                            .ok_or("quotient copy result is absent")?;
                        if tip != Some(result.structural_type)
                            && module.structural_types.iter().any(|ty| {
                                ty.quotient_carrier.is_some()
                                    && (Some(ty.id) == tip || ty.id == result.structural_type)
                            })
                            && quotient_copy_adaptation(module, machine, operation).is_none()
                        {
                            return Err("unlicensed nominal quotient copy");
                        }
                    }
                    if let OperationKind::MoveStructuralValue { source } = &operation.kind {
                        let root = quotient_place_type(machine, source.place)
                            .ok_or("quotient move source is absent")?;
                        let tip = quotient_path_tip(module, root, &source.path)?;
                        if operation
                            .result
                            .structural()
                            .map(|result| result.structural_type)
                            != tip
                        {
                            return Err("owned transfer cannot change nominal quotient identity");
                        }
                    }
                    if let OperationKind::StructuralCaseLeafCopy { source, path } = &operation.kind
                    {
                        let root = quotient_place_type(machine, *source)
                            .ok_or("quotient case-copy source is absent")?;
                        let path = crate::canonical_structural_storage_path(
                            module.structural_types.iter(),
                            root,
                            path,
                        )
                        .ok_or("quotient case-copy path is absent")?;
                        quotient_path_tip(module, root, &path)?;
                    }

                    for projection in operation.kind.structural_projections() {
                        let root = quotient_place_type(machine, projection.root)
                            .ok_or("quotient projection has no declared source")?;
                        let tip = quotient_path_tip(module, root, projection.path)?;
                        if !module
                            .quotients
                            .representations
                            .iter()
                            .any(|row| Some(row.quotient) == tip)
                        {
                            continue;
                        }
                        let transport = matches!(
                            &operation.kind,
                            OperationKind::StructuralLeafCopy { .. }
                                | OperationKind::MoveStructuralValue { .. }
                                | OperationKind::EstablishRecord { .. }
                                | OperationKind::EstablishStructuralCase { .. }
                                | OperationKind::EstablishStructuralArray { .. }
                                | OperationKind::CallStructuralScalar { .. }
                                | OperationKind::CallStructural { .. }
                                | OperationKind::CallStructuralWithScalarArguments { .. }
                                | OperationKind::CallUnit { .. }
                        );
                        if !transport {
                            return Err("operation observes an opaque quotient value");
                        }
                    }
                }
                for block in &machine.blocks {
                    if let terminal_psi::Terminator::StructuralCase { source, path, .. } =
                        &block.terminator
                    {
                        let root = quotient_place_type(machine, *source)
                            .ok_or("quotient case source is absent")?;
                        let tip = quotient_path_tip(module, root, path)?;
                        if module
                            .structural_types
                            .iter()
                            .any(|ty| Some(ty.id) == tip && ty.quotient_carrier.is_some())
                        {
                            return Err("case terminator observes an opaque quotient");
                        }
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) mod operation {
        use super::origins::{scalar_origin, whole_origin};
        use super::*;
        pub(crate) fn validate_operation(
            module: &TerminalModule,
            index: usize,
            row: &terminal_psi::TerminalQuotientOperation,
        ) -> Result<(), &'static str> {
            use language_semantics::quotient_correspondence::QuotientPositionalRelation as Relation;
            let public = module
                .machine(row.public_machine)
                .ok_or("quotient public body is absent")?;
            let representative = module
                .machine(row.representative_machine)
                .ok_or("quotient representative body is absent")?;
            let contract = row;
            let call = public
                .operation(row.call)
                .ok_or("quotient selected call is absent")?;
            if call.kind.direct_callee() != Some(representative.id) {
                return Err("quotient selected callee drifted");
            }
            let arguments = call.kind.structural_arguments();
            if arguments.len() != representative.structural_parameters.len()
                || arguments.len() != contract.input_relations.len()
            {
                return Err("quotient argument correspondence is incomplete");
            }
            for (position, ((actual, formal), relation)) in arguments
                .iter()
                .zip(&representative.structural_parameters)
                .zip(&contract.input_relations)
                .enumerate()
            {
                if !actual.path.is_empty()
                    || actual.access != formal.access
                    || formal.multiplicity != StructuralMultiplicity::Unrestricted
                    || !formal.qualifications.is_empty()
                    || !formal.projected_qualifications.is_empty()
                {
                    return Err("quotient input modes drifted");
                }
                match relation {
                    Relation::Quotient(relation) => {
                        let mut adaptations =
                            module.quotients.adaptations.iter().filter(|adaptation| {
                                adaptation.license
                                    == TerminalQuotientAdaptationLicense::Input {
                                        operation: index as u32,
                                        position: position as u32,
                                    }
                            });
                        let adaptation = adaptations
                            .next()
                            .ok_or("quotient input adaptation is absent")?;
                        if adaptations.next().is_some() {
                            return Err("quotient input adaptation is duplicated");
                        }
                        if adaptation.machine != public.id || adaptation.result != actual.place {
                            return Err(
                                "quotient input adaptation does not supply the selected call",
                            );
                        }
                        let representation = module
                            .quotients
                            .representations
                            .get(adaptation.representation as usize)
                            .ok_or("quotient input representation is absent")?;
                        if adaptation.source_type != representation.quotient
                            || adaptation.result_type != formal.structural_type
                            || adaptation.result_type != representation.carrier
                        {
                            return Err(
                                "quotient input adaptation types differ from the selected formals",
                            );
                        }
                        if module
                            .quotients
                            .representations
                            .get(adaptation.representation as usize)
                            .map(|row| &row.formation)
                            != Some(relation)
                        {
                            return Err(
                                "quotient input adaptation formation differs from the correspondence",
                            );
                        }
                        for operation in public.operations() {
                            if operation.id == call.id {
                                continue;
                            }
                            let mut escapes = false;
                            operation.kind.clone().map_structural_uses(&mut |place| {
                                escapes |= place == adaptation.result;
                                place
                            });
                            if escapes {
                                return Err(
                                    "quotient representative input escapes its selected call",
                                );
                            }
                        }
                        for block in &public.blocks {
                            let mut escapes = false;
                            block.terminator.clone().map_structural_uses(&mut |place| {
                                escapes |= place == adaptation.result;
                                place
                            });
                            if escapes {
                                return Err(
                                    "quotient representative input escapes through control flow",
                                );
                            }
                        }
                    }
                    Relation::ExactEquality {
                        public_type,
                        representative_type,
                    } => {
                        if quotient_place_type(public, actual.place) != Some(formal.structural_type)
                            || public_type != representative_type
                        {
                            return Err("quotient ordinary input identity drifted");
                        }
                    }
                }
            }
            if let Relation::ExactEquality {
                public_type,
                representative_type,
            } = &contract.result_relation
                && (public
                    .declaration
                    .as_ref()
                    .and_then(|row| module.identities.text(row.result_type))
                    != Some(public_type.as_str())
                    || representative
                        .declaration
                        .as_ref()
                        .and_then(|row| module.identities.text(row.result_type))
                        != Some(representative_type.as_str()))
            {
                return Err("quotient result type identity drifted");
            }
            let mut returns = 0;
            for block in &public.blocks {
                match &block.terminator {
                    terminal_psi::Terminator::ReturnStructural {
                        source,
                        returned_claims,
                        ..
                    } => {
                        returns += 1;
                        if !returned_claims.is_empty() {
                            return Err("quotient result carries claims");
                        }
                        let origin = whole_origin(public, *source)
                            .ok_or("quotient result has no exact origin")?;
                        match &contract.result_relation {
                            Relation::Quotient(relation) => {
                                let mut adaptations =
                                    module.quotients.adaptations.iter().filter(|adaptation| {
                                        adaptation.license
                                            == TerminalQuotientAdaptationLicense::Result {
                                                operation: index as u32,
                                            }
                                    });
                                let adaptation = adaptations
                                    .next()
                                    .ok_or("quotient result adaptation is absent")?;
                                if adaptations.next().is_some()
                                    || adaptation.machine != public.id
                                    || adaptation.result != origin
                                    || call.result.structural().map(|result| result.place)
                                        != Some(adaptation.source)
                                    || module
                                        .quotients
                                        .representations
                                        .get(adaptation.representation as usize)
                                        .map(|row| &row.formation)
                                        != Some(relation)
                                {
                                    return Err(
                                        "quotient normal result does not return its selected call",
                                    );
                                }
                            }
                            Relation::ExactEquality {
                                public_type,
                                representative_type,
                            } => {
                                if public_type != representative_type
                                    || call.result.structural().map(|result| result.place)
                                        != Some(origin)
                                {
                                    return Err("quotient exact structural result drifted");
                                }
                            }
                        }
                    }
                    terminal_psi::Terminator::Return { value, .. } => {
                        returns += 1;
                        let Relation::ExactEquality {
                            public_type,
                            representative_type,
                        } = &contract.result_relation
                        else {
                            return Err("quotient nominal result became scalar");
                        };
                        if public_type != representative_type
                            || scalar_origin(public, *value)
                                != call.result.scalar().map(|result| result.id)
                            || public.result.scalar().map(|result| result.value_type())
                                != representative
                                    .result
                                    .scalar()
                                    .map(|result| result.value_type())
                        {
                            return Err("quotient exact scalar result drifted");
                        }
                    }
                    terminal_psi::Terminator::ReturnUnit { .. } => {
                        return Err("quotient result disappeared");
                    }
                    _ => {}
                }
            }
            if returns == 0 {
                return Err("quotient operation has no normal result");
            }
            Ok(())
        }
    }
    pub(crate) mod origins {
        use super::*;
        use semantic_vocabulary::{BlockId, ValueId};
        use terminal_psi::StructuralArgument;

        fn incoming(
            machine: &TerminalMachine,
            target: BlockId,
        ) -> Option<Vec<(Option<&[ValueId]>, &[StructuralArgument])>> {
            let mut rows = Vec::new();
            for block in &machine.blocks {
                for binding in crate::control_graph::forwarded_bindings(block) {
                    if binding.target == target {
                        rows.push((binding.arguments, binding.structural_arguments));
                    }
                }
            }
            (!rows.is_empty()).then_some(rows)
        }

        pub(crate) fn whole_origin(machine: &TerminalMachine, place: PlaceId) -> Option<PlaceId> {
            whole(machine, place, &mut Vec::new())
        }
        fn whole(
            machine: &TerminalMachine,
            place: PlaceId,
            active: &mut Vec<PlaceId>,
        ) -> Option<PlaceId> {
            if active.contains(&place) {
                return None;
            }
            active.push(place);
            let result = (|| {
                if let Some((block, position)) = machine.blocks.iter().find_map(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .position(|parameter| parameter.place == place)
                        .map(|position| (block, position))
                }) {
                    let mut origin = None;
                    for (_, arguments) in incoming(machine, block.id)? {
                        let actual = arguments.get(position)?;
                        if !actual.path.is_empty()
                            || quotient_place_type(machine, actual.place)
                                != Some(block.structural_parameters[position].structural_type)
                        {
                            return None;
                        }
                        let candidate = whole(machine, actual.place, active)?;
                        if origin.is_some_and(|prior| prior != candidate) {
                            return None;
                        }
                        origin = Some(candidate);
                    }
                    return origin;
                }
                if let Some((operation, ty)) = machine.producing_operation(place) {
                    let selected = match &operation.kind {
                        OperationKind::StructuralLeafCopy { source, path } => Some((*source, path)),
                        OperationKind::MoveStructuralValue { source } => {
                            Some((source.place, &source.path))
                        }
                        _ => None,
                    };
                    if let Some((source, path)) = selected
                        && path.is_empty()
                        && quotient_place_type(machine, source) == Some(ty)
                    {
                        return whole(machine, source, active);
                    }
                }
                Some(place)
            })();
            active.pop();
            result
        }

        pub(crate) fn scalar_origin(machine: &TerminalMachine, value: ValueId) -> Option<ValueId> {
            scalar(machine, value, &mut Vec::new())
        }
        fn scalar(
            machine: &TerminalMachine,
            value: ValueId,
            active: &mut Vec<ValueId>,
        ) -> Option<ValueId> {
            if active.contains(&value) {
                return None;
            }
            active.push(value);
            let result = (|| {
                if let Some((block, position)) = machine.blocks.iter().find_map(|block| {
                    block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == value)
                        .map(|position| (block, position))
                }) {
                    let mut origin = None;
                    for (arguments, _) in incoming(machine, block.id)? {
                        let candidate = scalar(machine, *arguments?.get(position)?, active)?;
                        if origin.is_some_and(|prior| prior != candidate) {
                            return None;
                        }
                        origin = Some(candidate);
                    }
                    return origin;
                }
                let operation = machine.operations().find(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == value)
                })?;
                if let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind {
                    if !path.is_empty() {
                        return None;
                    }
                    let source = whole_origin(machine, *source)?;
                    let (producer, _) = machine.producing_operation(source)?;
                    if let OperationKind::EstablishPrimitiveLocal { value, .. } = producer.kind {
                        return scalar(machine, value, active);
                    }
                    return None;
                }
                Some(value)
            })();
            active.pop();
            result
        }
    }
    pub(crate) mod representations {
        use super::*;
        use terminal_psi::ByteSequenceCarrier;
        use terminal_psi::StructuralCaseDeclaration;
        use terminal_psi::StructuralFieldDeclaration;
        use terminal_psi::StructuralTypeShape;
        pub fn validate_quotient_representations(
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            catalog: &terminal_psi::TerminalQuotientCatalog,
        ) -> Result<(), &'static str> {
            for declaration in declarations {
                if let Some(carrier) = declaration.quotient_carrier {
                    let mut rows = catalog
                        .representations
                        .iter()
                        .filter(|row| row.quotient == declaration.id && row.carrier == carrier);
                    if rows.next().is_none() || rows.next().is_some() {
                        return Err("quotient declaration lost its unique formation row");
                    }
                }
            }
            for (index, row) in catalog.representations.iter().enumerate() {
                if row.quotient == row.carrier
                    || catalog.representations[..index]
                        .iter()
                        .any(|prior| prior.quotient == row.quotient)
                {
                    return Err("duplicate or reflexive quotient representation");
                }
                let quotient = declarations
                    .iter()
                    .find(|ty| ty.id == row.quotient)
                    .ok_or("undeclared quotient representation")?;
                let carrier = declarations
                    .iter()
                    .find(|ty| ty.id == row.carrier)
                    .ok_or("undeclared quotient carrier")?;
                validate_carrier(declarations, row.carrier, &mut Vec::new())?;
                if quotient.quotient_carrier != Some(row.carrier) {
                    return Err("quotient formation targets an ordinary declaration");
                }
                if row.formation.quotient_declaration.is_empty()
                    || row.formation.relation.is_empty()
                    || !same_carrier_shape(&quotient.shape, &carrier.shape)
                    || quotient.scalar_restriction != carrier.scalar_restriction
                    || quotient.descriptor_cleanup != carrier.descriptor_cleanup
                    || quotient.owns_linear_claim != carrier.owns_linear_claim
                    || quotient.no_code_affine_cleanup != carrier.no_code_affine_cleanup
                {
                    return Err("quotient representation identity or retained shape mismatch");
                }
            }
            for (index, row) in catalog.adaptations.iter().enumerate() {
                if catalog
                    .representations
                    .get(row.representation as usize)
                    .is_none()
                    || catalog.adaptations[..index]
                        .iter()
                        .any(|other| other.machine == row.machine && other.copy == row.copy)
                {
                    return Err("invalid or duplicate quotient adaptation reference");
                }
                let selected = match row.license {
                    TerminalQuotientAdaptationLicense::Mint => None,
                    TerminalQuotientAdaptationLicense::Input { operation, .. }
                    | TerminalQuotientAdaptationLicense::Result { operation } => Some(operation),
                };
                if selected.is_some_and(|index| catalog.operations.get(index as usize).is_none()) {
                    return Err("quotient adaptation references an absent operation");
                }
            }
            Ok(())
        }

        pub fn validate_quotient_representation_identities(
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            catalog: &terminal_psi::TerminalQuotientCatalog,
            identities: &terminal_psi::TerminalIdentityTable,
        ) -> Result<(), &'static str> {
            for row in &catalog.representations {
                let quotient = declarations
                    .iter()
                    .find(|ty| ty.id == row.quotient)
                    .ok_or("absent quotient declaration")?;
                let carrier = declarations
                    .iter()
                    .find(|ty| ty.id == row.carrier)
                    .ok_or("absent quotient carrier")?;
                if quotient.identity != row.quotient_storage_identity
                    || carrier.identity != row.carrier_storage_identity
                    || identities
                        .text(row.quotient_storage_identity)
                        .is_none_or(str::is_empty)
                    || identities
                        .text(row.carrier_storage_identity)
                        .is_none_or(str::is_empty)
                {
                    return Err("quotient nominal identity drifted");
                }
            }
            Ok(())
        }

        fn validate_carrier(
            declarations: &[terminal_psi::StructuralTypeDeclaration],
            id: StructuralTypeId,
            active: &mut Vec<StructuralTypeId>,
        ) -> Result<(), &'static str> {
            use terminal_psi::{StructuralFieldType as F, StructuralTypeShape as S};
            if active.contains(&id) {
                return Err("cyclic quotient carrier storage");
            }
            let declaration = declarations
                .iter()
                .find(|ty| ty.id == id)
                .ok_or("quotient carrier storage is absent")?;
            if declaration.owns_linear_claim {
                return Err("quotient carrier owns a linear obligation");
            }
            active.push(id);
            let mut children = Vec::new();
            let mut custody_field = false;
            let mut field = |field: &StructuralFieldDeclaration| {
                custody_field |= matches!(field.field_type, F::Atomic(_))
                    || field
                        .field_type
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| {
                            !matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
                        });
                if let F::Structural(child) = field.field_type {
                    children.push(child);
                }
            };
            match &declaration.shape {
                S::Opaque => return Err("quotient carrier has no visible storage representation"),
                S::AtomicCell(_) => return Err("quotient carrier contains atomic storage"),
                S::Reference { .. }
                | S::ElementView { .. }
                | S::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { .. }
                    | ByteSequenceCarrier::BoundedView { .. },
                ) => {
                    return Err("quotient carrier contains reference or routed storage");
                }
                S::Record { fields, .. } => fields.iter().for_each(&mut field),
                S::Sum { cases } => {
                    for case in cases {
                        case.fields.iter().for_each(&mut field);
                    }
                }
                S::Mixed { fields, cases } => {
                    fields.iter().for_each(&mut field);
                    for case in cases {
                        case.fields.iter().for_each(&mut field);
                    }
                }
                S::FixedArray { element, .. } => children.push(*element),
                S::PrimitiveScalar(_) | S::ByteSequence(_) => {}
            }
            if custody_field {
                return Err("quotient carrier contains borrowed or atomic field storage");
            }
            for child in children {
                validate_carrier(declarations, child, active)?;
            }
            active.pop();
            Ok(())
        }

        fn same_carrier_shape(left: &StructuralTypeShape, right: &StructuralTypeShape) -> bool {
            use terminal_psi::StructuralTypeShape as S;
            fn fields(
                left: &[StructuralFieldDeclaration],
                right: &[StructuralFieldDeclaration],
            ) -> bool {
                left.len() == right.len()
                    && left.iter().zip(right).all(|(a, b)| {
                        a.identity == b.identity
                            && a.relevance == b.relevance
                            && a.storage == b.storage
                            && a.field_type == b.field_type
                    })
            }
            fn cases(
                left: &[StructuralCaseDeclaration],
                right: &[StructuralCaseDeclaration],
            ) -> bool {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(a, b)| a.identity == b.identity && fields(&a.fields, &b.fields))
            }
            match (left, right) {
                (
                    S::Record {
                        fields: a,
                        storage: sa,
                    },
                    S::Record {
                        fields: b,
                        storage: sb,
                    },
                ) => sa == sb && fields(a, b),
                (S::Sum { cases: a }, S::Sum { cases: b }) => cases(a, b),
                (
                    S::Mixed {
                        fields: af,
                        cases: ac,
                    },
                    S::Mixed {
                        fields: bf,
                        cases: bc,
                    },
                ) => fields(af, bf) && cases(ac, bc),
                _ => left == right,
            }
        }
    }
    use opacity::validate_quotient_opacity;
    use operation::validate_operation;
    pub use representations::{
        validate_quotient_representation_identities, validate_quotient_representations,
    };

    pub fn quotient_place_type(
        machine: &TerminalMachine,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        machine
            .structural_parameter(place)
            .map(|row| row.structural_type)
            .or_else(|| machine.producing_operation(place).map(|(_, ty)| ty))
    }

    pub fn quotient_copy_adaptation<'a>(
        module: &'a TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Option<&'a TerminalQuotientAdaptation> {
        let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
            return None;
        };
        if !path.is_empty() {
            return None;
        }
        let result = operation.result.structural()?;
        let source_type = quotient_place_type(machine, *source)?;
        let mut rows = module
            .quotients
            .adaptations
            .iter()
            .filter(|row| row.machine == machine.id && row.copy == operation.id);
        let row = rows.next()?;
        if rows.next().is_some()
            || row.source != *source
            || row.result != result.place
            || row.source_type != source_type
            || row.result_type != result.structural_type
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
        {
            return None;
        }
        let representation = module
            .quotients
            .representations
            .get(row.representation as usize)?;
        match row.license {
            TerminalQuotientAdaptationLicense::Mint
                if source_type == representation.carrier
                    && result.structural_type == representation.quotient =>
            {
                Some(row)
            }
            TerminalQuotientAdaptationLicense::Input {
                operation,
                position,
            } if source_type == representation.quotient
                && result.structural_type == representation.carrier =>
            {
                let owner = module.quotients.operations.get(operation as usize)?;
                let call = machine.operation(owner.call)?;
                (owner.public_machine == machine.id
                        && call.kind.direct_callee() == Some(owner.representative_machine)
                        && call.kind.structural_arguments().get(position as usize)
                            .is_some_and(|argument| argument.place == result.place && argument.path.is_empty())
                        && owner.input_relations.get(position as usize)
                            == Some(&language_semantics::quotient_correspondence::QuotientPositionalRelation::Quotient(representation.formation.clone())))
                        .then_some(row)
            }
            TerminalQuotientAdaptationLicense::Result { operation }
                if source_type == representation.carrier
                    && result.structural_type == representation.quotient =>
            {
                let owner = module.quotients.operations.get(operation as usize)?;
                let call = machine.operation(owner.call)?;
                (owner.public_machine == machine.id
                        && call.kind.direct_callee() == Some(owner.representative_machine)
                        && call.result.structural().is_some_and(|result| result.place == *source)
                        && owner.result_relation
                            == language_semantics::quotient_correspondence::QuotientPositionalRelation::Quotient(representation.formation.clone()))
                        .then_some(row)
            }
            _ => None,
        }
    }

    pub fn validate_quotient_catalog(module: &TerminalModule) -> Result<(), &'static str> {
        validate_quotient_representations(&module.structural_types, &module.quotients)?;
        validate_quotient_representation_identities(
            &module.structural_types,
            &module.quotients,
            &module.identities,
        )?;
        for (index, operation) in module.quotients.operations.iter().enumerate() {
            if module.quotients.operations[..index].iter().any(|prior| {
                prior.public_machine == operation.public_machine && prior.call == operation.call
            }) {
                return Err("duplicate quotient call correspondence");
            }
            validate_operation(module, index, operation)?;
        }
        validate_quotient_opacity(module)?;
        for row in &module.quotients.adaptations {
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == row.machine)
                .ok_or("quotient adaptation owner is absent")?;
            let operation = machine
                .operation(row.copy)
                .ok_or("quotient copy operation is absent")?;
            if quotient_copy_adaptation(module, machine, operation) != Some(row) {
                return Err("quotient copy receipt does not bind its exact endpoints");
            }
            let source_copy = machine
                .structural_parameter(row.source)
                .map(|source| {
                    source.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.qualifications.is_empty()
                        && source.projected_qualifications.is_empty()
                })
                .or_else(|| {
                    machine
                        .producing_operation(row.source)
                        .and_then(|(op, _)| op.result.structural())
                        .map(|source| {
                            source.multiplicity == StructuralMultiplicity::Unrestricted
                                && source.claims.is_empty()
                                && source.qualifications.is_empty()
                                && source.projected_qualifications.is_empty()
                        })
                })
                .unwrap_or(false);
            if !source_copy {
                return Err("quotient copy source carries custody or qualifications");
            }
        }
        Ok(())
    }
}

/*
Semantic judgments for structural reference origins, placed loan regions, reborrow custody, and
reference access capabilities.
*/

pub(crate) mod reference_completion {

    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    };

    pub struct ReferenceCompletionOccurrence<'a, Identity> {
        pub identity: &'a Identity,
        pub parents: Vec<&'a Identity>,
        pub returned: bool,
        pub in_caller_storage: bool,
        pub disposed: bool,
    }

    pub fn borrowed_entry_reference_referent<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        parameter: &StructuralParameterDeclaration,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        if parameter.access == StructuralAccess::Owned
            || !crate::projection_stays_in_inline_storage(
                declarations.clone(),
                parameter.structural_type,
                path,
            )
        {
            return None;
        }
        match crate::runtime_structural_path_tip(declarations, parameter.structural_type, path)?
            .shape
        {
            StructuralTypeShape::Reference { referent, .. } => Some(referent),
            _ => None,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ReferenceCompletionError {
        DuplicateOccurrence,
        MissingParent,
        CyclicAncestry,
        UndisposedParent,
        SurvivingChild,
    }

    impl ReferenceCompletionError {
        pub const fn description(self) -> &'static str {
            match self {
                Self::DuplicateOccurrence => {
                    "reference completion has duplicate occurrence identities"
                }
                Self::MissingParent => "reference completion lost an immediate parent",
                Self::CyclicAncestry => "reference completion has cyclic ancestry",
                Self::UndisposedParent => {
                    "reference completion does not dispose an intermediate parent"
                }
                Self::SurvivingChild => {
                    "reference completion leaves a child of a closing parent live"
                }
            }
        }
    }

    pub fn validate_reference_result_completion<Identity: PartialEq>(
        occurrences: &[ReferenceCompletionOccurrence<'_, Identity>],
    ) -> Result<(), ReferenceCompletionError> {
        for (position, occurrence) in occurrences.iter().enumerate() {
            if occurrences[..position]
                .iter()
                .any(|previous| previous.identity == occurrence.identity)
            {
                return Err(ReferenceCompletionError::DuplicateOccurrence);
            }
        }
        let mut visited = vec![0u8; occurrences.len()];
        let mut closing = vec![false; occurrences.len()];
        for (position, occurrence) in occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| occurrence.returned || occurrence.in_caller_storage)
        {
            if occurrence.disposed {
                return Err(ReferenceCompletionError::UndisposedParent);
            }
            let mut pending = vec![(position, false)];
            while let Some((position, completed)) = pending.pop() {
                if completed {
                    visited[position] = 2;
                    continue;
                }
                match visited[position] {
                    1 => return Err(ReferenceCompletionError::CyclicAncestry),
                    2 => continue,
                    _ => {}
                }
                visited[position] = 1;
                pending.push((position, true));
                for parent in occurrences[position].parents.iter().rev() {
                    let parent_position = occurrences
                        .iter()
                        .position(|candidate| candidate.identity == *parent)
                        .ok_or(ReferenceCompletionError::MissingParent)?;
                    let parent = &occurrences[parent_position];
                    if !parent.in_caller_storage && (!parent.disposed || parent.returned) {
                        return Err(ReferenceCompletionError::UndisposedParent);
                    }
                    closing[parent_position] = !parent.in_caller_storage;
                    pending.push((parent_position, false));
                }
            }
        }
        for (position, occurrence) in occurrences.iter().enumerate() {
            if !closing[position] {
                continue;
            }
            if occurrences.iter().any(|child| {
                child.parents.contains(&occurrence.identity)
                    && !child.returned
                    && !child.in_caller_storage
                    && !child.disposed
            }) {
                return Err(ReferenceCompletionError::SurvivingChild);
            }
        }
        Ok(())
    }

    pub fn cleanup_disposes_reference(
        actions: &[TerminalAffineCleanupAction],
        carrier: PlaceId,
        path: &[StructuralPathSegment],
    ) -> bool {
        if path.contains(&StructuralPathSegment::Referent) {
            return false;
        }
        actions.iter().any(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => *place == carrier,
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                discard.place == carrier
                    && path.starts_with(&discard.path)
                    && !discard.path.contains(&StructuralPathSegment::Referent)
            }
            TerminalAffineCleanupAction::InvokeNominal(_)
            | TerminalAffineCleanupAction::RetireHome(_) => false,
        })
    }
}
pub(crate) mod reference_origins {

    use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
    use terminal_psi::{
        ReferenceOriginProjection, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    pub(crate) mod borrowed_leaves {

        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration,
            StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
            StructuralTypeShape,
        };

        pub enum BorrowedResultLeafKind {
            Reference(StructuralTypeId),
            View,
        }

        pub struct BorrowedResultLeaf {
            pub path: Vec<StructuralPathSegment>,
            pub access: StructuralAccess,
            pub kind: BorrowedResultLeafKind,
        }

        pub fn borrowed_result_leaves<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            multiplicity: StructuralMultiplicity,
            maximum_leaves: usize,
        ) -> Result<Vec<BorrowedResultLeaf>, &'static str> {
            let mut roster = BorrowedResultRoster {
                types,
                multiplicity,
                maximum_leaves,
                active: Vec::new(),
                leaves: Vec::new(),
            };
            roster.visit(root, &mut Vec::new())?;
            roster
                .leaves
                .sort_by(|left, right| left.path.cmp(&right.path));
            Ok(roster.leaves)
        }

        struct BorrowedResultRoster<I> {
            types: I,
            multiplicity: StructuralMultiplicity,
            maximum_leaves: usize,
            active: Vec<StructuralTypeId>,
            leaves: Vec<BorrowedResultLeaf>,
        }

        impl<'types, I: Iterator<Item = &'types StructuralTypeDeclaration> + Clone>
            BorrowedResultRoster<I>
        {
            fn push(
                &mut self,
                path: &[StructuralPathSegment],
                access: StructuralAccess,
                kind: BorrowedResultLeafKind,
            ) -> Result<(), &'static str> {
                if self.leaves.len() == self.maximum_leaves {
                    return Err("dynamic result has more borrowed leaves than public regions");
                }
                if access == StructuralAccess::Owned {
                    return Err("dynamic borrowed result leaf has owned access");
                }
                self.leaves.push(BorrowedResultLeaf {
                    path: path.to_vec(),
                    access,
                    kind,
                });
                Ok(())
            }

            fn visit(
                &mut self,
                root: StructuralTypeId,
                path: &mut Vec<StructuralPathSegment>,
            ) -> Result<(), &'static str> {
                if self.active.contains(&root) {
                    return Err("dynamic result contains a cyclic owned type");
                }
                let mut declarations = self.types.clone().filter(|row| row.id == root);
                let declaration = declarations
                    .next()
                    .ok_or("dynamic result has an unknown structural type")?;
                if declarations.next().is_some() {
                    return Err("dynamic result has an ambiguous structural type");
                }
                match &declaration.shape {
                    StructuralTypeShape::Reference { access, referent } => {
                        return self.push(
                            path,
                            *access,
                            BorrowedResultLeafKind::Reference(*referent),
                        );
                    }
                    StructuralTypeShape::ElementView { access, .. }
                    | StructuralTypeShape::ByteSequence(
                        ByteSequenceCarrier::BorrowedView { access }
                        | ByteSequenceCarrier::BoundedView { access, .. },
                    ) => {
                        let access = if path.is_empty() {
                            crate::descriptors::descriptor_results::descriptor_value_access(
                                self.types.clone(),
                                root,
                                self.multiplicity,
                            )
                            .ok_or("dynamic result descriptor has incompatible access and multiplicity")?
                        } else {
                            access.ok_or("stored result descriptor has no declared access")?
                        };
                        return self.push(path, access, BorrowedResultLeafKind::View);
                    }
                    _ => {}
                }
                self.active.push(root);
                match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length }
                        if length.literal() != Some(0) =>
                    {
                        let first = self.leaves.len();
                        path.push(StructuralPathSegment::FixedIndex(0));
                        self.visit(*element, path)?;
                        path.pop();
                        let per_element = self.leaves.len() - first;
                        if per_element != 0 {
                            let length = length.literal().ok_or(
                                "dynamic result borrowed leaf enumeration requires a closed array extent",
                            )?;
                            let count = usize::try_from(length)
                                .ok()
                                .and_then(|length| length.checked_mul(per_element))
                                .and_then(|count| first.checked_add(count))
                                .ok_or("dynamic result borrowed leaf count overflows")?;
                            if count > self.maximum_leaves {
                                return Err(
                                    "dynamic result has more borrowed leaves than public regions",
                                );
                            }
                            for index in 1..length {
                                path.push(StructuralPathSegment::FixedIndex(index));
                                self.visit(*element, path)?;
                                path.pop();
                            }
                        }
                    }
                    StructuralTypeShape::Record { fields, .. }
                    | StructuralTypeShape::Mixed { fields, .. } => self.fields(fields, path)?,
                    _ => {}
                }
                if let StructuralTypeShape::Sum { cases }
                | StructuralTypeShape::Mixed { cases, .. } = &declaration.shape
                {
                    for case in cases {
                        path.push(StructuralPathSegment::Case(case.id));
                        self.fields(&case.fields, path)?;
                        path.pop();
                    }
                }
                self.active.pop();
                Ok(())
            }

            fn fields(
                &mut self,
                fields: &[StructuralFieldDeclaration],
                path: &mut Vec<StructuralPathSegment>,
            ) -> Result<(), &'static str> {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    path.push(StructuralPathSegment::Field(field.identity));
                    if let Some(child) = field.field_type.as_structural() {
                        self.visit(child, path)?;
                    } else if let Some(
                        ByteSequenceCarrier::BorrowedView { access }
                        | ByteSequenceCarrier::BoundedView { access, .. },
                    ) = field.field_type.byte_sequence_carrier()
                    {
                        self.push(
                            path,
                            access.ok_or("stored result byte view has no declared access")?,
                            BorrowedResultLeafKind::View,
                        )?;
                    }
                    path.pop();
                }
                Ok(())
            }
        }
    }
    pub(crate) mod call_results {

        use super::reference_origin_type;
        use semantic_vocabulary::{ReferenceOriginExtent, StructuralTypeId};
        use terminal_psi::TerminalDynamicReferenceOrigin;
        use terminal_psi::{
            ReferenceOriginProjection, StructuralArgument, StructuralParameterDeclaration,
            StructuralPathSegment, StructuralReferenceResultSource, StructuralTypeDeclaration,
            TerminalDynamicSlot,
        };

        pub enum ReferenceCallContract<'declaration> {
            Machine {
                parameters: &'declaration [StructuralParameterDeclaration],
                sources: &'declaration [StructuralReferenceResultSource],
            },
            Requirement(&'declaration TerminalDynamicSlot),
        }

        pub enum ReferenceCallOrigin {
            Parameter {
                source: StructuralArgument,
                transfers_existing: bool,
            },
            Invocation {
                carrier_type: StructuralTypeId,
                path: Vec<ReferenceOriginProjection>,
                access: terminal_psi::StructuralAccess,
            },
        }

        pub struct ReferenceCallSource {
            pub origin: ReferenceCallOrigin,
            pub suffix: Vec<ReferenceOriginProjection>,
            pub endpoint: StructuralTypeId,
            pub extent: ReferenceOriginExtent,
        }

        pub struct ReferenceCallResult {
            pub path: Vec<StructuralPathSegment>,
            pub sources: Vec<ReferenceCallSource>,
        }

        impl ReferenceCallSource {
            /// The origin suffix a returned reference projects from its actual
            /// argument, or `None` when that storage cannot be the declared
            /// referent. `source_root` is the argument's resolved storage type.
            ///
            /// The returned reference keeps exactly the storage its argument lent.
            /// An argument may lend a scalar under another carrier: an exclusive
            /// loan of a restricted field stays a place loan, so its recast is not
            /// materialized before the call. The origin then records the presented
            /// view over the stored carrier, as an established reference view does,
            /// so later reads and writes still address the original backing.
            pub fn projected_origin_suffix<'types>(
                &self,
                types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
                source_root: StructuralTypeId,
                expected: StructuralTypeId,
                access: terminal_psi::StructuralAccess,
            ) -> Option<std::borrow::Cow<'_, [ReferenceOriginProjection]>> {
                let endpoint = reference_origin_type(types.clone(), source_root, &self.suffix)?;
                if self.extent != ReferenceOriginExtent::Projected || endpoint == expected {
                    return Some(std::borrow::Cow::Borrowed(&self.suffix));
                }
                let scalar = |identity| {
                    crate::structural_declaration(types.clone(), identity)
                        .filter(|declaration| declaration.scalar_restriction.is_none())
                        .and_then(|declaration| match declaration.shape {
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => {
                                Some(scalar)
                            }
                            _ => None,
                        })
                };
                if !crate::scalar_reference_view_admits(
                    access,
                    scalar(endpoint)?,
                    scalar(expected)?,
                ) {
                    return None;
                }
                let mut suffix = self.suffix.clone();
                suffix.push(ReferenceOriginProjection::Exact(
                    StructuralPathSegment::RepresentationView {
                        structural_type: expected,
                        stored_scalar: None,
                    },
                ));
                Some(std::borrow::Cow::Owned(suffix))
            }
        }

        pub fn bind_reference_call_results<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            contract: ReferenceCallContract<'_>,
            arguments: &[StructuralArgument],
        ) -> Result<Vec<ReferenceCallResult>, &'static str> {
            let mut results = Vec::new();
            match contract {
                ReferenceCallContract::Machine {
                    parameters,
                    sources,
                } => {
                    for mapping in sources {
                        let mut candidates = Vec::with_capacity(mapping.sources.len());
                        for origin in &mapping.sources {
                            let origin = match origin {
                                terminal_psi::StructuralReferenceOrigin::Parameter(origin) => {
                                    origin
                                }
                                terminal_psi::StructuralReferenceOrigin::Invocation {
                                    carrier_type,
                                    path,
                                    extent,
                                    access,
                                } => {
                                    let endpoint =
                                        reference_origin_type(types.clone(), *carrier_type, path).ok_or(
                                            "invocation reference origin has no typed provider endpoint",
                                        )?;
                                    candidates.push(ReferenceCallSource {
                                        origin: ReferenceCallOrigin::Invocation {
                                            carrier_type: *carrier_type,
                                            path: path.clone(),
                                            access: *access,
                                        },
                                        suffix: Vec::new(),
                                        endpoint,
                                        extent: *extent,
                                    });
                                    continue;
                                }
                            };
                            let argument_index = terminal_psi::structural_parameter_position(
                                parameters,
                                origin.place,
                            )
                            .ok_or("returned origin names no formal input")?;
                            let parameter = &parameters[argument_index];
                            let binding = super::bind_reference_origin(
                                types.clone(),
                                parameter.structural_type,
                                origin,
                            )?;
                            let transfers_existing = binding.transfers_existing(
                                types.clone(),
                                parameter.structural_type,
                                parameter.access,
                            );
                            let mut source = arguments
                                .get(argument_index)
                                .ok_or("returned origin has no actual structural argument")?
                                .clone();
                            source.path.extend(binding.source.path);
                            source.access = origin.access;
                            candidates.push(ReferenceCallSource {
                                origin: ReferenceCallOrigin::Parameter {
                                    source,
                                    transfers_existing,
                                },
                                suffix: binding.suffix.to_vec(),
                                endpoint: binding.endpoint,
                                extent: binding.extent,
                            });
                        }
                        results.push(ReferenceCallResult {
                            path: mapping.path.clone(),
                            sources: candidates,
                        });
                    }
                }
                ReferenceCallContract::Requirement(slot) => {
                    return bind_public_reference_call_results(types, slot, arguments);
                }
            }
            Ok(results)
        }

        pub fn bind_public_reference_call_results<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            slot: &impl terminal_psi::ReferenceResultShape,
            arguments: &[StructuralArgument],
        ) -> Result<Vec<ReferenceCallResult>, &'static str> {
            let mut results: Vec<ReferenceCallResult> = Vec::new();
            let leaves = match slot.result() {
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    super::borrowed_result_leaves(
                        types.clone(),
                        result.structural_type,
                        result.multiplicity,
                        slot.reference_regions().len(),
                    )?
                }
                _ => Vec::new(),
            };
            for region in slot.reference_regions() {
                if !leaves.iter().any(|leaf| {
                    leaf.path == region.path
                        && matches!(leaf.kind, super::BorrowedResultLeafKind::Reference(_))
                }) {
                    continue;
                }
                let index = match region.origin {
                    TerminalDynamicReferenceOrigin::Parameter { argument_index } => argument_index,
                    TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                        let path = region
                            .source_path
                            .iter()
                            .cloned()
                            .map(ReferenceOriginProjection::Exact)
                            .collect::<Vec<_>>();
                        let endpoint = reference_origin_type(types.clone(), carrier_type, &path)
                            .ok_or("dynamic invocation reference origin has no typed endpoint")?;
                        let candidate = ReferenceCallSource {
                            origin: ReferenceCallOrigin::Invocation {
                                carrier_type,
                                path,
                                access: region.access,
                            },
                            suffix: Vec::new(),
                            endpoint,
                            extent: ReferenceOriginExtent::Projected,
                        };
                        if let Some(result) =
                            results.iter_mut().find(|result| result.path == region.path)
                        {
                            result.sources.push(candidate);
                        } else {
                            results.push(ReferenceCallResult {
                                path: region.path.clone(),
                                sources: vec![candidate],
                            });
                        }
                        continue;
                    }
                };
                let argument_index = usize::try_from(index)
                    .map_err(|_| "dynamic returned region input ordinal overflows")?;
                let (parameter_type, _) = slot
                    .structural_parameter(argument_index)
                    .ok_or("dynamic returned region names no formal input")?;
                let origin_path = region
                    .source_path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>();
                let endpoint = reference_origin_type(types.clone(), parameter_type, &origin_path)
                    .ok_or("dynamic returned region has no typed input endpoint")?;
                let source_path = crate::reference_source_storage_path(
                    types.clone(),
                    parameter_type,
                    &region.source_path,
                );
                let mut source = arguments
                    .get(argument_index)
                    .ok_or("dynamic returned region has no actual structural argument")?
                    .clone();
                source.path.extend_from_slice(source_path);
                source.access = region.access;
                let candidate = ReferenceCallSource {
                    origin: ReferenceCallOrigin::Parameter {
                        source,
                        transfers_existing: false,
                    },
                    suffix: Vec::new(),
                    endpoint,
                    extent: ReferenceOriginExtent::WithinRegion,
                };
                if let Some(result) = results.iter_mut().find(|result| result.path == region.path) {
                    result.sources.push(candidate);
                } else {
                    results.push(ReferenceCallResult {
                        path: region.path.clone(),
                        sources: vec![candidate],
                    });
                }
            }

            Ok(results)
        }
    }
    pub(crate) mod memberships {
        use super::{ReferenceCallContract, ReferenceCallOrigin, ReferenceCallResult};
        use crate::descriptors::descriptor_lifetimes::DescriptorPosition;
        use crate::descriptors::descriptor_results::{
            self as descriptors, DescriptorAnalysis, DescriptorMachine, PayloadMembershipContract,
        };
        use semantic_vocabulary::{MachineId, PlaceId, ReferenceOriginExtent, StructuralTypeId};
        use terminal_psi::{
            Operation, ReferenceOriginProjection, StructuralAccess, StructuralOperationResult,
            StructuralPathQualification, StructuralPathSegment, StructuralResultDeclaration,
            StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
        };

        pub struct ReferenceResultMemberships<'module> {
            module: &'module TerminalModule,
            graph: std::cell::OnceCell<Vec<DescriptorMachine>>,
        }

        impl<'module> ReferenceResultMemberships<'module> {
            pub fn new(module: &'module TerminalModule) -> Self {
                Self {
                    module,
                    graph: std::cell::OnceCell::new(),
                }
            }

            pub fn for_operation(
                &self,
                caller: &TerminalMachine,
                operation: &Operation,
            ) -> Result<Vec<StructuralPathQualification>, &'static str> {
                let (terminal_psi::call_structural_pair_pattern!(callee, structural_arguments, ..)) =
                    &operation.kind
                else {
                    return Ok(Vec::new());
                };
                let caller = self
                    .module
                    .machine(caller.id)
                    .ok_or("reference call has no caller")?;
                let result = operation
                    .result
                    .structural()
                    .ok_or("reference call has no result")?;
                let callee = self
                    .module
                    .machine(*callee)
                    .ok_or("reference call has no callee")?;
                let signature = callee
                    .result
                    .structural()
                    .ok_or("reference callee has no result")?;
                if signature.reference_sources.is_empty() {
                    return Ok(Vec::new());
                }
                if result.structural_type != signature.structural_type
                    || result.multiplicity != signature.multiplicity
                {
                    return Err("reference call result disagrees with its public carrier");
                }
                let mut positions =
                    caller.blocks.iter().flat_map(|block| {
                        block.operations.iter().enumerate().filter_map(
                            move |(ordinal, candidate)| {
                                (candidate.id == operation.id && candidate == operation)
                                    .then_some((block.id, ordinal))
                            },
                        )
                    });
                let (block, ordinal) = positions
                    .next()
                    .ok_or("reference call has no current-body cut")?;
                if positions.next().is_some() {
                    return Err("reference call has ambiguous current-body cuts");
                }
                let position = (
                    block,
                    u32::try_from(ordinal).map_err(|_| "call ordinal overflows")?,
                );
                let mappings = super::bind_reference_call_results(
                    self.module.structural_types.iter(),
                    ReferenceCallContract::Machine {
                        parameters: &callee.structural_parameters,
                        sources: &signature.reference_sources,
                    },
                    structural_arguments,
                )?;
                preserve_call_memberships(
                    &self.module.structural_types,
                    &self.module.structural_domains,
                    || {
                        self.graph
                            .get_or_init(|| {
                                descriptors::terminal_borrowed_storage_graph(self.module)
                            })
                            .as_slice()
                    },
                    caller.id,
                    position,
                    result,
                    signature,
                    &mappings,
                    &|place| descriptors::terminal_payload_contract(caller, place),
                )
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub fn reference_call_preserved_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            position: DescriptorPosition,
            result: &StructuralOperationResult,
            declaration: &StructuralResultDeclaration,
            mappings: &[ReferenceCallResult],
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<Vec<StructuralPathQualification>, &'static str> {
            preserve_call_memberships(
                types,
                domains,
                || graph,
                identity,
                position,
                result,
                declaration,
                mappings,
                source,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn preserve_call_memberships<'input>(
            types: &'input [StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            graph: impl Fn() -> &'input [DescriptorMachine],
            identity: MachineId,
            position: DescriptorPosition,
            result: &StructuralOperationResult,
            declaration: &StructuralResultDeclaration,
            mappings: &[ReferenceCallResult],
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<Vec<StructuralPathQualification>, &'static str> {
            if result.structural_type != declaration.structural_type
                || result.multiplicity != declaration.multiplicity
            {
                return Err("preserved reference result has a different declared carrier");
            }
            let replay_source = |place| {
                if place == result.place {
                    Some(PayloadMembershipContract {
                        scalar_observations: Vec::new(),
                        scalar_requirements: Vec::new(),
                        carrier: declaration.structural_type,
                        qualifications: declaration.qualifications.clone(),
                        projected_qualifications: declaration.projected_qualifications.clone(),
                    })
                } else {
                    source(place)
                }
            };
            let mut replay = None;
            let mut retained = Vec::new();
            for (ordinal, mapping) in mappings.iter().enumerate() {
                if mappings[..ordinal]
                    .iter()
                    .any(|previous| previous.path == mapping.path)
                {
                    return Err("returned reference membership repeats an output leaf");
                }
                let Some(leaf) = crate::runtime_structural_path_tip(
                    types.iter(),
                    result.structural_type,
                    &mapping.path,
                ) else {
                    return Err("returned reference membership has no typed leaf");
                };
                let StructuralTypeShape::Reference {
                    referent,
                    access: StructuralAccess::SharedBorrow,
                } = leaf.shape
                else {
                    continue;
                };
                let mut common: Option<PayloadMembershipContract> = None;
                let mut complete = !mapping.sources.is_empty();
                for candidate in &mapping.sources {
                    let selected = (|| {
                        if candidate.extent != ReferenceOriginExtent::Projected {
                            return None;
                        }
                        let ReferenceCallOrigin::Parameter { source: actual, .. } =
                            &candidate.origin
                        else {
                            return None;
                        };
                        let root = replay_source(actual.place)?;
                        let mut path = actual.path.clone();
                        path.extend(candidate.suffix.iter().map(|projection| match projection {
                            ReferenceOriginProjection::Exact(segment) => segment.clone(),
                            ReferenceOriginProjection::Element => {
                                StructuralPathSegment::AllElements
                            }
                        }));
                        if let Some((StructuralPathSegment::Referent, prefix)) = path.split_last()
                            && crate::qualification_path_carrier(types.iter(), root.carrier, prefix)
                                .and_then(|carrier| {
                                    terminal_psi::find_structural_type(types, carrier)
                                })
                                .is_some_and(|row| {
                                    !matches!(row.shape, StructuralTypeShape::Reference { .. })
                                })
                        {
                            path.pop();
                        }
                        let query = descriptors::descriptor_contents_query(
                            types,
                            actual.place,
                            root.carrier,
                            &path,
                            None,
                        )?;
                        let (graph, analysis) = replay.get_or_insert_with(|| {
                            let graph = graph();
                            (
                                graph,
                                DescriptorAnalysis::with_borrowed_storage_slots(types, graph),
                            )
                        });
                        let targets = analysis.payload_targets(identity, position, &query)?;
                        descriptors::payload_memberships(types, domains, targets, |root| {
                            descriptors::completed_payload_contract(
                                types,
                                domains,
                                graph,
                                identity,
                                position,
                                root,
                                &replay_source,
                            )
                            .ok()
                        })?
                        .presented(types, domains, referent)
                    })();
                    let Some(selected) = selected else {
                        complete = false;
                        break;
                    };
                    if let Some(common) = &mut common {
                        common
                            .qualifications
                            .retain(|domain| selected.qualifications.contains(domain));
                        common
                            .projected_qualifications
                            .retain(|row| selected.projected_qualifications.contains(row));
                    } else {
                        common = Some(selected);
                    }
                }
                if complete && let Some(common) = common {
                    let mut destination = mapping.path.clone();
                    destination.push(StructuralPathSegment::Referent);
                    StructuralPathQualification::append_under(
                        &common.qualifications,
                        &common.projected_qualifications,
                        &destination,
                        &mut retained,
                    );
                }
            }
            retained.sort();
            retained.dedup();
            Ok(retained)
        }

        pub fn reference_result_memberships_weaken<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            carrier: StructuralTypeId,
            source: &[StructuralPathQualification],
            declared: &[StructuralPathQualification],
        ) -> bool {
            declared.iter().all(|row| source.contains(row))
                && source.iter().all(|row| {
                    if declared.contains(row) {
                        return true;
                    }
                    let mut crosses_shared = false;
                    for (index, segment) in row.path.iter().enumerate() {
                        if *segment != StructuralPathSegment::Referent {
                            continue;
                        }
                        let Some(declaration) =
                            crate::qualification_path_carrier(types.clone(), carrier, &row.path[..index])
                                .and_then(|carrier| crate::structural_declaration(types.clone(), carrier))
                        else {
                            return false;
                        };
                        match declaration.shape {
                            StructuralTypeShape::Reference {
                                access: StructuralAccess::SharedBorrow,
                                ..
                            } => {
                                crosses_shared = true;
                            }
                            _ => return false,
                        }
                    }
                    crosses_shared
                })
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use semantic_vocabulary::{BlockId, ScalarType};
            use std::cell::Cell;
            use terminal_psi::{StructuralArgument, StructuralMultiplicity, TerminalIdentityIndex};

            fn declarations(
                access: StructuralAccess,
            ) -> (
                Vec<StructuralTypeDeclaration>,
                StructuralOperationResult,
                StructuralResultDeclaration,
            ) {
                let scalar = StructuralTypeId::new(1).unwrap();
                let reference = StructuralTypeId::new(2).unwrap();
                let types = [
                    (
                        scalar,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    ),
                    (
                        reference,
                        StructuralTypeShape::Reference {
                            referent: scalar,
                            access,
                        },
                    ),
                ]
                .into_iter()
                .map(|(id, shape)| StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex(1),
                    owns_linear_claim: false,
                    copyable: false,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                    shape,
                    scalar_restriction: None,
                })
                .collect();
                let result = StructuralOperationResult {
                    place: PlaceId::new(99).unwrap(),
                    structural_type: reference,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                };
                let declaration = StructuralResultDeclaration {
                    place: result.place,
                    structural_type: reference,
                    multiplicity: result.multiplicity,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                };
                (types, result, declaration)
            }

            fn mapping(extent: ReferenceOriginExtent) -> ReferenceCallResult {
                ReferenceCallResult {
                    path: Vec::new(),
                    sources: vec![super::super::ReferenceCallSource {
                        origin: ReferenceCallOrigin::Parameter {
                            source: StructuralArgument {
                                place: PlaceId::new(3).unwrap(),
                                path: Vec::new(),
                                access: StructuralAccess::SharedBorrow,
                            },
                            transfers_existing: false,
                        },
                        suffix: Vec::new(),
                        endpoint: StructuralTypeId::new(1).unwrap(),
                        extent,
                    }],
                }
            }

            fn observe(
                types: &[StructuralTypeDeclaration],
                result: &StructuralOperationResult,
                declaration: &StructuralResultDeclaration,
                mappings: &[ReferenceCallResult],
            ) -> (
                Result<Vec<StructuralPathQualification>, &'static str>,
                usize,
            ) {
                let builds = Cell::new(0);
                let outcome = preserve_call_memberships(
                    types,
                    &[],
                    || {
                        builds.set(builds.get() + 1);
                        &[]
                    },
                    MachineId::new(1).unwrap(),
                    (BlockId::new(1).unwrap(), 0),
                    result,
                    declaration,
                    mappings,
                    &|_| {
                        Some(PayloadMembershipContract {
                            scalar_observations: Vec::new(),
                            scalar_requirements: Vec::new(),
                            carrier: StructuralTypeId::new(1).unwrap(),
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        })
                    },
                );
                (outcome, builds.get())
            }

            #[test]
            fn undemanded_rows_still_reject_malformed_mappings_without_a_graph() {
                let (types, mut result, declaration) =
                    declarations(StructuralAccess::MutableBorrow);
                let repeated = [
                    mapping(ReferenceOriginExtent::Projected),
                    mapping(ReferenceOriginExtent::Projected),
                ];
                assert_eq!(
                    observe(&types, &result, &declaration, &repeated),
                    (
                        Err("returned reference membership repeats an output leaf"),
                        0
                    )
                );
                let mut invalid = mapping(ReferenceOriginExtent::Projected);
                invalid.path.push(StructuralPathSegment::FixedIndex(0));
                assert_eq!(
                    observe(&types, &result, &declaration, &[invalid]),
                    (Err("returned reference membership has no typed leaf"), 0)
                );
                result.structural_type = StructuralTypeId::new(1).unwrap();
                assert_eq!(
                    observe(&types, &result, &declaration, &[]),
                    (
                        Err("preserved reference result has a different declared carrier"),
                        0
                    )
                );
            }

            #[test]
            fn only_a_projected_shared_referent_demands_the_graph() {
                for access in [
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::SharedBorrow,
                ] {
                    let (types, result, declaration) = declarations(access);
                    assert_eq!(
                        observe(&types, &result, &declaration, &[]),
                        (Ok(Vec::new()), 0)
                    );
                    assert_eq!(
                        observe(
                            &types,
                            &result,
                            &declaration,
                            &[mapping(ReferenceOriginExtent::WithinRegion)]
                        ),
                        (Ok(Vec::new()), 0)
                    );
                    assert_eq!(
                        observe(
                            &types,
                            &result,
                            &declaration,
                            &[mapping(ReferenceOriginExtent::Projected)]
                        ),
                        (
                            Ok(Vec::new()),
                            usize::from(access == StructuralAccess::SharedBorrow)
                        )
                    );
                }
            }
        }
    }
    pub use borrowed_leaves::{BorrowedResultLeaf, BorrowedResultLeafKind, borrowed_result_leaves};
    pub use memberships::{
        ReferenceResultMemberships, reference_call_preserved_memberships,
        reference_result_memberships_weaken,
    };
    pub(crate) mod descriptor_regions {

        use semantic_vocabulary::ReferenceOriginExtent;
        use terminal_psi::{
            ReferenceOriginProjection, ReferenceOriginSource, StructuralAccess,
            StructuralParameterDeclaration, StructuralPathSegment, StructuralResultDeclaration,
            StructuralTypeDeclaration, TerminalDynamicReferenceRegion,
        };

        use crate::descriptors::descriptor_lifetimes::{
            descriptor_location, descriptor_location_type, descriptor_path,
        };
        use crate::descriptors::descriptor_results::{
            DescriptorContentsStep, DescriptorResultSlot, DescriptorStoredOrigin,
        };
        use crate::storage::borrowed_storage::Projection;

        pub fn validate_descriptor_result_regions(
            types: &[StructuralTypeDeclaration],
            parameters: &[StructuralParameterDeclaration],
            result: &StructuralResultDeclaration,
            regions: &[TerminalDynamicReferenceRegion],
            slots: &[DescriptorResultSlot],
        ) -> Result<(), &'static str> {
            let leaves = super::borrowed_result_leaves(
                types.iter(),
                result.structural_type,
                result.multiplicity,
                regions.len(),
            )?;
            for leaf in leaves
                .into_iter()
                .filter(|leaf| matches!(leaf.kind, super::BorrowedResultLeafKind::View))
            {
                let location = descriptor_location(
                    types,
                    result.place,
                    result.structural_type,
                    &leaf.path,
                    None,
                )
                .ok_or("dynamic returned view has no typed descriptor location")?;
                let mut matching = slots.iter().filter(|slot| slot.path == location.path);
                let slot = matching
                    .next()
                    .ok_or("dynamic row has no complete returned view origins")?;
                if matching.next().is_some() || slot.origins.is_empty() {
                    return Err("dynamic row has ambiguous or unknown returned view origins");
                }
                for origin in &slot.origins {
                    if matches!(origin, DescriptorStoredOrigin::Empty) {
                        continue;
                    }
                    if matches!(origin, DescriptorStoredOrigin::Literal { .. }) {
                        if leaf.access != StructuralAccess::SharedBorrow {
                            return Err("dynamic literal view exceeds immutable image access");
                        }
                        let mut advertised = false;
                        for region in regions.iter().filter(|region| region.path == leaf.path) {
                            let terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                                argument_index,
                            } = region.origin
                            else {
                                return Err(
                                    "dynamic literal view has no shared input-region contract",
                                );
                            };
                            if region.access != StructuralAccess::SharedBorrow {
                                return Err("dynamic literal view exceeds its public access");
                            }
                            let parameter = usize::try_from(argument_index)
                                .ok()
                                .and_then(|position| parameters.get(position))
                                .ok_or("dynamic literal view names no formal input")?;
                            super::result_regions::validate_input_region(
                                types.iter(),
                                parameter.structural_type,
                                parameter.access,
                                &region.source_path,
                                region.access,
                            )?;
                            advertised = true;
                        }
                        if !advertised {
                            return Err("dynamic literal view has no public input-region contract");
                        }
                        continue;
                    }
                    let (position, path) = input_origin_path(types, parameters, origin)?;
                    let parameter = parameters
                        .get(position)
                        .ok_or("dynamic returned view names no formal input")?;
                    let actual = ReferenceOriginSource {
                        place: parameter.place,
                        path,
                        extent: ReferenceOriginExtent::WithinRegion,
                        access: leaf.access,
                    };
                    let allowed = regions
                        .iter()
                        .filter(|region| {
                            region.path == leaf.path
                                && matches!(region.origin, terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } if usize::try_from(argument_index).ok() == Some(position))
                        })
                        .map(|region| ReferenceOriginSource {
                            place: parameter.place,
                            path: region
                                .source_path
                                .iter()
                                .cloned()
                                .map(ReferenceOriginProjection::Exact)
                                .collect(),
                            extent: ReferenceOriginExtent::WithinRegion,
                            access: region.access,
                        })
                        .collect::<Vec<_>>();
                    if !super::reference_origin_is_covered(
                        types.iter(),
                        parameter.structural_type,
                        &actual,
                        &allowed,
                    ) && !inline_view_is_covered(
                        types,
                        parameter.structural_type,
                        &actual,
                        &allowed,
                    ) {
                        return Err("dynamic row returned view exceeds its public input regions");
                    }
                }
            }
            Ok(())
        }

        fn input_origin_path(
            types: &[StructuralTypeDeclaration],
            parameters: &[StructuralParameterDeclaration],
            origin: &DescriptorStoredOrigin,
        ) -> Result<(usize, Vec<ReferenceOriginProjection>), &'static str> {
            let (position, mut path, projection) = match origin {
                DescriptorStoredOrigin::Parameter {
                    position,
                    projection,
                } => (*position, Vec::new(), Some(projection)),
                DescriptorStoredOrigin::IncomingSlot { position, path } => {
                    let parameter = parameters
                        .get(*position)
                        .ok_or("returned view slot names no formal input")?;
                    let path = descriptor_path(types, parameter.structural_type, path)
                        .ok_or("returned view slot has no complete typed input path")?;
                    (
                        *position,
                        path.into_iter()
                            .map(ReferenceOriginProjection::Exact)
                            .collect(),
                        None,
                    )
                }
                DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps,
                    projection,
                } => {
                    let parameter = parameters
                        .get(*position)
                        .ok_or("returned view contents name no formal input")?;
                    (
                        *position,
                        contents_path(types, parameter.structural_type, steps)?,
                        Some(projection),
                    )
                }
                _ => return Err("dynamic returned view has no public input-relative origin"),
            };
            if let Some(projection) = projection {
                append_projection(&mut path, projection)?;
            }
            Ok((position, path))
        }

        fn append_projection(
            path: &mut Vec<ReferenceOriginProjection>,
            projection: &Projection,
        ) -> Result<(), &'static str> {
            if projection.indirect {
                return Err("dynamic returned view has unresolved indirect backing");
            }
            for segment in &projection.path {
                match segment {
                    StructuralPathSegment::FixedRange { .. } => break,
                    StructuralPathSegment::RuntimeIndex { .. } => {
                        path.push(ReferenceOriginProjection::Element)
                    }
                    StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Referent => {
                        path.push(ReferenceOriginProjection::Exact(segment.clone()))
                    }
                    _ => return Err("dynamic returned view has an unsupported backing projection"),
                }
            }
            Ok(())
        }

        fn contents_path(
            types: &[StructuralTypeDeclaration],
            mut root: semantic_vocabulary::StructuralTypeId,
            steps: &[DescriptorContentsStep],
        ) -> Result<Vec<ReferenceOriginProjection>, &'static str> {
            let mut path = Vec::new();
            let mut exact = Vec::new();
            for step in steps {
                match step {
                    DescriptorContentsStep::Address(segment) => exact.push(segment.clone()),
                    DescriptorContentsStep::AnyElement { element } => {
                        let collection = descriptor_location_type(types, root, &exact)
                            .ok_or("returned view contents have no typed collection")?;
                        if super::collection_element_type(types.iter(), collection)
                            != Some(*element)
                        {
                            return Err(
                                "returned view contents disagree with the input element type",
                            );
                        }
                        path.extend(
                            descriptor_path(types, root, &exact)
                                .ok_or("returned view contents have no typed prefix")?
                                .into_iter()
                                .map(ReferenceOriginProjection::Exact),
                        );
                        path.push(ReferenceOriginProjection::Element);
                        exact.clear();
                        root = *element;
                    }
                }
            }
            path.extend(
                descriptor_path(types, root, &exact)
                    .ok_or("returned view contents have no typed endpoint")?
                    .into_iter()
                    .map(ReferenceOriginProjection::Exact),
            );
            Ok(path)
        }

        fn inline_view_is_covered(
            types: &[StructuralTypeDeclaration],
            root: semantic_vocabulary::StructuralTypeId,
            actual: &ReferenceOriginSource,
            allowed: &[ReferenceOriginSource],
        ) -> bool {
            let Some(path) = actual
                .path
                .iter()
                .map(|segment| match segment {
                    ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                    ReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let inline_view = crate::runtime_structural_field(types.iter(), root, &path)
                .and_then(|field| field.field_type.byte_sequence_carrier())
                .is_some_and(|carrier| {
                    matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    )
                });
            inline_view
                && allowed.iter().any(|allowed| {
                    allowed.place == actual.place
                        && allowed.access == actual.access
                        && allowed.path == actual.path
                })
        }
    }
    pub(crate) mod result_regions {

        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::TerminalDynamicReferenceOrigin;
        use terminal_psi::{
            BoundaryMachineResult, ReferenceOriginProjection, StructuralAccess,
            StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
            TerminalDynamicSlot,
        };

        pub fn validate_dynamic_reference_regions<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            slot: &TerminalDynamicSlot,
        ) -> Result<(), &'static str> {
            validate_reference_regions(
                types,
                &slot.result,
                &slot.result_reference_regions,
                |index| {
                    slot.structural_parameters
                        .get(index)
                        .map(|p| (p.structural_type, p.access))
                },
                true,
            )
        }

        pub fn validate_boundary_reference_regions<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            boundary: &terminal_psi::BoundaryMachineDeclaration,
        ) -> Result<(), &'static str> {
            validate_reference_regions(
                types,
                &boundary.result,
                &boundary.result_reference_regions,
                |index| {
                    boundary
                        .structural_parameters
                        .get(index)
                        .map(|p| (p.structural_type, p.access))
                },
                false,
            )
        }

        fn validate_reference_regions<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            result: &BoundaryMachineResult,
            regions: &[terminal_psi::TerminalDynamicReferenceRegion],
            parameter: impl Fn(usize) -> Option<(StructuralTypeId, StructuralAccess)>,
            include_views: bool,
        ) -> Result<(), &'static str> {
            let BoundaryMachineResult::Structural(result) = result else {
                return if regions.is_empty() {
                    Ok(())
                } else {
                    Err("nonstructural dynamic result carries reference regions")
                };
            };
            if regions.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("dynamic reference regions are not unique and ordered");
            }
            let leaves = super::borrowed_result_leaves(
                types.clone(),
                result.structural_type,
                result.multiplicity,
                regions.len(),
            )?;
            if leaves
                .iter()
                .filter(|leaf| {
                    include_views
                        || matches!(leaf.kind, super::BorrowedResultLeafKind::Reference(_))
                })
                .any(|leaf| !regions.iter().any(|region| region.path == leaf.path))
            {
                return Err("dynamic reference regions omit a result leaf");
            }
            for region in regions {
                let leaf = leaves
                    .iter()
                    .find(|leaf| leaf.path == region.path)
                    .ok_or("dynamic reference region does not name an owned result leaf")?;
                if leaf.access != region.access {
                    return Err("dynamic reference region disagrees with result access");
                }
                let index = match region.origin {
                    TerminalDynamicReferenceOrigin::Parameter { argument_index } => argument_index,
                    TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                        let path = region
                            .source_path
                            .iter()
                            .cloned()
                            .map(ReferenceOriginProjection::Exact)
                            .collect::<Vec<_>>();
                        let endpoint =
                            super::reference_origin_type(types.clone(), carrier_type, &path)
                                .ok_or("dynamic invocation origin has no typed provider storage")?;
                        if let super::BorrowedResultLeafKind::Reference(referent) = leaf.kind
                            && referent != endpoint
                        {
                            return Err("dynamic invocation origin changes its reference referent");
                        }
                        validate_input_region(
                            types.clone(),
                            carrier_type,
                            StructuralAccess::Owned,
                            &region.source_path,
                            region.access,
                        )?;
                        continue;
                    }
                };
                let (parameter_type, parameter_access) = usize::try_from(index)
                    .ok()
                    .and_then(&parameter)
                    .ok_or("dynamic reference region has no structural input")?;
                validate_input_region(
                    types.clone(),
                    parameter_type,
                    parameter_access,
                    &region.source_path,
                    region.access,
                )?;
            }
            Ok(())
        }

        pub(crate) fn validate_input_region<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            access: StructuralAccess,
            path: &[StructuralPathSegment],
            required: StructuralAccess,
        ) -> Result<(), &'static str> {
            if !required.admits(access) {
                return Err("dynamic reference region exceeds input access");
            }
            let mut borrowed = access != StructuralAccess::Owned;
            for (position, segment) in path.iter().enumerate() {
                let field_view =
                    crate::runtime_structural_field(types.clone(), root, &path[..=position])
                        .and_then(|field| field.field_type.byte_sequence_carrier());
                let view_access = match field_view {
                    Some(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                    ) => Some(access.ok_or("stored byte view has no declared access")?),
                    _ => {
                        crate::runtime_structural_path_tip(types.clone(), root, &path[..=position])
                            .and_then(|row| match row.shape {
                                StructuralTypeShape::ElementView { .. } => {
                                    Some(StructuralAccess::SharedBorrow)
                                }
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                                    | terminal_psi::ByteSequenceCarrier::BoundedView {
                                        access, ..
                                    },
                                ) => Some(access.unwrap_or(StructuralAccess::SharedBorrow)),
                                _ => None,
                            })
                    }
                };
                if let Some(view_access) = view_access {
                    if !required.admits_referent(view_access) {
                        return Err("dynamic reference region exceeds stored view access");
                    }
                    borrowed = true;
                }
                match segment {
                    StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_) => {}
                    StructuralPathSegment::Referent => {
                        let prefix = crate::runtime_structural_path_tip(
                            types.clone(),
                            root,
                            &path[..position],
                        )
                        .ok_or("dynamic input region crossing has no typed carrier")?;
                        if let StructuralTypeShape::Reference { access, .. } = prefix.shape {
                            if !required.admits_referent(access) {
                                return Err(
                                    "dynamic reference region exceeds stored reference access",
                                );
                            }
                            borrowed = true;
                        } else if position + 1 != path.len() {
                            return Err("dynamic input region has a nonterminal storage marker");
                        }
                    }
                    _ => return Err("dynamic input region is not an exact public storage path"),
                }
            }
            let origin = path
                .iter()
                .cloned()
                .map(ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>();
            let storage_path = crate::reference_source_storage_path(types.clone(), root, path);
            let inline_view = crate::runtime_structural_field(types.clone(), root, storage_path)
                .and_then(|field| field.field_type.byte_sequence_carrier())
                .is_some_and(|carrier| {
                    matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    )
                });
            if !inline_view {
                super::reference_origin_type(types, root, &origin)
                    .ok_or("dynamic input region path has no typed endpoint")?;
            }
            if !borrowed {
                return Err("dynamic result cannot borrow private owned parameter storage");
            }
            Ok(())
        }

        pub fn reference_leaves<'types>(
            types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            maximum_leaves: usize,
        ) -> Option<Vec<ReferenceLeaf>> {
            let contains_reference = |root| {
                terminal_psi::structural_type_contains_reference_by(
                    |id| types.clone().find(|row| row.id == id),
                    root,
                )
            };
            let mut output = Vec::new();
            let mut pending = vec![(root, Vec::new())];
            while let Some((current, path)) = pending.pop() {
                if !contains_reference(current) {
                    continue;
                }
                let declaration = types.clone().find(|row| row.id == current)?;
                let fields: Vec<_> = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Reference { .. } => {
                        if output.len() == maximum_leaves {
                            return None;
                        }
                        output.push(ReferenceLeaf {
                            path,
                            structural_type: current,
                        });
                        continue;
                    }
                    terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                        fields.iter().map(|field| (None, field)).collect()
                    }
                    terminal_psi::StructuralTypeShape::Sum { cases } => cases
                        .iter()
                        .flat_map(|case| {
                            case.fields.iter().map(move |field| (Some(case.id), field))
                        })
                        .collect(),
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => cases
                        .iter()
                        .flat_map(|case| {
                            case.fields.iter().map(move |field| (Some(case.id), field))
                        })
                        .chain(fields.iter().map(|field| (None, field)))
                        .collect(),
                    terminal_psi::StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        let count = usize::try_from(*length).ok()?;
                        if pending
                            .len()
                            .checked_add(output.len())?
                            .checked_add(count)?
                            > maximum_leaves
                        {
                            return None;
                        }
                        for element_index in (0..*length).rev() {
                            let mut child_path = path.clone();
                            child_path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                                element_index,
                            ));
                            pending.push((*element, child_path));
                        }
                        continue;
                    }
                    _ => return None,
                };
                for (case, field) in fields.into_iter().rev() {
                    if let Some(child) = field.field_type.as_structural() {
                        if !contains_reference(child) {
                            continue;
                        }
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        if let Some(case) = case {
                            child_path.push(StructuralPathSegment::Case(case));
                        }
                        child_path.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                        pending.push((child, child_path));
                    }
                }
            }

            output.sort_by(|left, right| left.path.cmp(&right.path));
            if output.windows(2).any(|pair| {
                pair[0].path == pair[1].path && pair[0].structural_type != pair[1].structural_type
            }) {
                return None;
            }
            output.dedup_by(|left, right| left.path == right.path);
            Some(output)
        }

        pub struct ReferenceLeaf {
            pub path: Vec<StructuralPathSegment>,
            pub structural_type: StructuralTypeId,
        }
    }
    pub use call_results::{
        ReferenceCallContract, ReferenceCallOrigin, ReferenceCallResult, ReferenceCallSource,
        bind_public_reference_call_results, bind_reference_call_results,
    };
    pub use descriptor_regions::validate_descriptor_result_regions;
    pub use result_regions::{
        ReferenceLeaf, reference_leaves, validate_boundary_reference_regions,
        validate_dynamic_reference_regions,
    };
    use std::slice::from_ref;

    pub struct ReferenceOriginBinding<'origin> {
        pub source: terminal_psi::StructuralArgument,
        pub suffix: &'origin [ReferenceOriginProjection],
        pub endpoint: StructuralTypeId,
        pub extent: semantic_vocabulary::ReferenceOriginExtent,
    }

    impl ReferenceOriginBinding<'_> {
        pub fn transfers_existing<'types>(
            &self,
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            access: terminal_psi::StructuralAccess,
        ) -> bool {
            self.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                && self.suffix.is_empty()
                && matches!(
                    access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::SharedBorrow
                )
                && self.source.path.split_last().is_some_and(|(last, prefix)| {
                    *last == StructuralPathSegment::Referent
                        && crate::runtime_structural_path_tip(types, root, prefix).is_some_and(
                            |row| matches!(row.shape, StructuralTypeShape::Reference { .. }),
                        )
                })
        }
    }

    pub fn bind_reference_origin<'types, 'origin>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        origin: &'origin terminal_psi::ReferenceOriginSource,
    ) -> Result<ReferenceOriginBinding<'origin>, &'static str> {
        use ReferenceOriginProjection::{Element, Exact};
        use StructuralPathSegment::{FixedIndex, Referent};
        let endpoint = reference_origin_type(types.clone(), root, &origin.path)
            .ok_or("returned origin has no typed endpoint")?;
        let selection = origin
            .path
            .iter()
            .position(|projection| matches!(projection, Element | Exact(FixedIndex(_))));
        let last_crossing = origin
            .path
            .iter()
            .rposition(|projection| *projection == Exact(Referent));
        let split = selection.map_or(origin.path.len(), |selection| {
            last_crossing.map_or(selection, |crossing| selection.max(crossing + 1))
        });
        let prefix = origin.path[..split]
            .iter()
            .map(|projection| match projection {
                Exact(segment) => Ok(segment.clone()),
                Element => Err("returned origin needs a captured stored-descriptor identity"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = terminal_psi::StructuralArgument {
            place: origin.place,
            path: crate::reference_source_storage_path(types, root, &prefix).to_vec(),
            access: origin.access,
        };
        Ok(ReferenceOriginBinding {
            source,
            suffix: &origin.path[split..],
            endpoint,
            extent: origin.extent,
        })
    }

    fn reference_origin_refines<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        actual: &terminal_psi::ReferenceOriginSource,
        allowed: &terminal_psi::ReferenceOriginSource,
    ) -> bool {
        use ReferenceOriginProjection::{Element, Exact};
        use semantic_vocabulary::ReferenceOriginExtent::{Projected, WithinRegion};
        if actual.place != allowed.place
            || actual.access != allowed.access
            || captured_reference_origin_type(types.clone(), root, &actual.path).is_none()
            || reference_origin_type(types.clone(), root, &allowed.path).is_none()
        {
            return false;
        }
        let prefix_matches = actual.path.len() >= allowed.path.len()
            && allowed
                .path
                .iter()
                .zip(&actual.path)
                .all(|(expected, observed)| {
                    expected == observed
                        || matches!(
                            (expected, observed),
                            (Element, Exact(StructuralPathSegment::FixedIndex(_)))
                        )
                });
        if !prefix_matches {
            return false;
        }
        match allowed.extent {
            Projected => actual.extent == Projected && actual.path.len() == allowed.path.len(),
            WithinRegion => {
                for (position, projection) in
                    actual.path.iter().enumerate().skip(allowed.path.len())
                {
                    if !matches!(
                        projection,
                        Exact(
                            StructuralPathSegment::Referent | StructuralPathSegment::FixedIndex(_)
                        ) | Element
                    ) {
                        continue;
                    }
                    let Some(endpoint) = captured_reference_origin_type(
                        types.clone(),
                        root,
                        &actual.path[..position],
                    ) else {
                        return false;
                    };
                    let Some(declaration) = types.clone().find(|row| row.id == endpoint) else {
                        return false;
                    };
                    match projection {
                        Exact(StructuralPathSegment::Referent) => {
                            if position + 1 != actual.path.len()
                                || matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                            {
                                return false;
                            }
                        }
                        _ if descriptor_region(&declaration.shape)
                            && position != allowed.path.len() =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                }
                !(actual.extent == WithinRegion
                    && actual.path.len() > allowed.path.len()
                    && captured_reference_origin_type(types.clone(), root, &actual.path)
                        .and_then(|endpoint| crate::structural_declaration(types.clone(), endpoint))
                        .is_some_and(|row| descriptor_region(&row.shape)))
            }
        }
    }

    pub fn reference_origin_is_covered<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        actual: &terminal_psi::ReferenceOriginSource,
        allowed: &[terminal_psi::ReferenceOriginSource],
    ) -> bool {
        if allowed
            .iter()
            .any(|source| reference_origin_refines(types.clone(), root, actual, source))
        {
            return true;
        }
        for (position, projection) in actual.path.iter().enumerate() {
            if *projection != ReferenceOriginProjection::Element {
                continue;
            }
            let Some(collection) =
                captured_reference_origin_type(types.clone(), root, &actual.path[..position])
                    .and_then(|identity| crate::structural_declaration(types.clone(), identity))
            else {
                return false;
            };
            let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = collection.shape
            else {
                continue;
            };
            if length == 0 {
                return false;
            }
            let mut indices = allowed
                .iter()
                .filter(|source| source.place == actual.place && source.access == actual.access)
                .filter_map(|source| match source.path.get(position) {
                    Some(ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(
                        index,
                    ))) if *index < length => Some(*index),
                    _ => None,
                })
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            if u64::try_from(indices.len()).ok() != Some(length) {
                continue;
            }
            let mut selected = actual.clone();
            return indices.into_iter().all(|index| {
                selected.path[position] =
                    ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(index));
                reference_origin_is_covered(types.clone(), root, &selected, allowed)
            });
        }
        false
    }

    fn descriptor_region(shape: &StructuralTypeShape) -> bool {
        matches!(
            shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                )
        )
    }

    pub fn reference_result_refines_regions<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        result: &terminal_psi::StructuralResultDeclaration,
        regions: &[terminal_psi::TerminalDynamicReferenceRegion],
    ) -> bool {
        result.reference_sources.iter().all(|mapping| {
            !mapping.sources.is_empty()
                && mapping.sources.iter().all(|source| {
                    let source = match source {
                        terminal_psi::StructuralReferenceOrigin::Parameter(source) => source,
                        terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, path, extent, access } => {
                            let Some(endpoint) = reference_origin_type(types.clone(), *carrier_type, path) else {
                                return false;
                            };
                            return *extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                                && regions.iter().any(|region| {
                                    let terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type: allowed } = region.origin else { return false; };
                                    region.path == mapping.path && region.access == *access
                                        && reference_origin_type(types.clone(), allowed, &region.source_path.iter().cloned().map(ReferenceOriginProjection::Exact).collect::<Vec<_>>()) == Some(endpoint)
                                });
                        }
                    };
                    let Some((argument_index, parameter)) = parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.place == source.place)
                    else {
                        return false;
                    };
                    let allowed = regions
                        .iter()
                        .filter(|region| {
                            matches!(region.origin, terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index: index } if index as usize == argument_index)
                                && region.path == mapping.path
                        })
                        .map(|region| terminal_psi::ReferenceOriginSource {
                            place: parameter.place,
                            path: region
                                .source_path
                                .iter()
                                .cloned()
                                .map(ReferenceOriginProjection::Exact)
                                .collect(),
                            extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                            access: region.access,
                        })
                        .collect::<Vec<_>>();
                    reference_origin_is_covered(
                        types.clone(),
                        parameter.structural_type,
                        source,
                        &allowed,
                    )
                })
        })
    }

    pub fn reference_origin_paths_may_overlap(
        left: &[ReferenceOriginProjection],
        right: &[ReferenceOriginProjection],
    ) -> bool {
        use ReferenceOriginProjection::{Element, Exact};
        for (left, right) in left.iter().zip(right) {
            match (left, right) {
                (Exact(StructuralPathSegment::RepresentationView { .. }), _)
                | (_, Exact(StructuralPathSegment::RepresentationView { .. })) => return true,
                (
                    Exact(StructuralPathSegment::Case(left)),
                    Exact(StructuralPathSegment::Case(right)),
                ) if left != right => return true,
                (Element, _) | (_, Element) => {}
                (Exact(left), Exact(right))
                    if !crate::structural_paths_may_overlap(from_ref(left), from_ref(right)) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    pub fn captured_reference_origin_type<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[ReferenceOriginProjection],
    ) -> Option<StructuralTypeId> {
        let geometry = path
            .iter()
            .map(|projection| match projection {
                ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex {
                    ..
                }) => ReferenceOriginProjection::Element,
                _ => projection.clone(),
            })
            .collect::<Vec<_>>();
        resolve_reference_origin_type(types, root, &geometry)
    }

    pub fn reference_origin_type<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[ReferenceOriginProjection],
    ) -> Option<StructuralTypeId> {
        if path.iter().any(|projection| {
            matches!(
                projection,
                ReferenceOriginProjection::Exact(
                    StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::FixedRange { .. }
                )
            )
        }) {
            return None;
        }
        resolve_reference_origin_type(types, root, path)
    }

    fn resolve_reference_origin_type<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        mut root: StructuralTypeId,
        path: &[ReferenceOriginProjection],
    ) -> Option<StructuralTypeId> {
        let mut exact = Vec::new();
        for projection in path {
            match projection {
                ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(index)) => {
                    root = exact_origin_type(types.clone(), root, &exact)?;
                    exact.clear();
                    let collection = crate::structural_declaration(types.clone(), root)?;
                    if matches!(collection.shape, StructuralTypeShape::FixedArray { length: terminal_psi::ArrayExtent::Literal(length), .. } if *index >= length)
                    {
                        return None;
                    }
                    root = collection_element_type(types.clone(), root)?;
                }
                ReferenceOriginProjection::Exact(segment) => {
                    exact.push(segment.clone());
                }
                ReferenceOriginProjection::Element => {
                    root = exact_origin_type(types.clone(), root, &exact)?;
                    exact.clear();
                    root = collection_element_type(types.clone(), root)?;
                }
            }
        }
        exact_origin_type(types, root, &exact)
    }

    fn exact_origin_type<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let path = crate::reference_source_storage_path(types.clone(), root, path);
        crate::runtime_structural_path_tip(types.clone(), root, path)
            .map(|declaration| declaration.id)
            .or_else(|| crate::primitive_projection_declaration(types, root, path))
    }

    pub fn collection_element_type<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        collection: StructuralTypeId,
    ) -> Option<StructuralTypeId> {
        let declaration = crate::structural_declaration(types.clone(), collection)?;
        match declaration.shape {
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element, .. } => Some(element),
            StructuralTypeShape::ByteSequence(_) => {
                let mut matching = types.filter(|declaration| declaration.scalar_restriction.is_none()
                    && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                        if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address()));
                let element = matching.next()?;
                matching.next().is_none().then_some(element.id)
            }
            _ => None,
        }
    }
}
pub(crate) mod reference_storage_geometry {

    use semantic_vocabulary::{
        AuthoredFieldStorage, ScalarApplicationId, ScalarType, StructuralTypeId,
        TargetCarrierLayout,
    };
    use terminal_psi::{
        LayoutApplicationId, StructuralFieldDeclaration, StructuralPathSegment,
        StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
        TargetLayoutRealization,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReferenceStorageProjection {
        pub byte_offset: u64,
        pub logical_scalar: ScalarType,
        pub stored_scalar: ScalarType,
    }

    pub enum ReferenceScalarStorage<'recipe> {
        Contiguous(ReferenceStorageProjection),
        Fragmented {
            byte_offset: u64,
            logical_scalar: ScalarType,
            fragments: &'recipe semantic_vocabulary::AuthoredBitFragments,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReferenceStorageRegion {
        pub byte_offset: u64,
        pub byte_length: u64,
        pub scalar: Option<ReferenceStorageProjection>,
        pub array: Option<ReferenceArrayWindow>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReferenceArrayWindow {
        pub declaration: StructuralTypeId,
        pub byte_offset: u64,
        pub byte_length: u64,
    }

    fn align(offset: u64, alignment: u64) -> Option<u64> {
        offset
            .checked_add(alignment.checked_sub(1)?)
            .map(|value| value / alignment * alignment)
    }

    fn scalar_geometry(scalar: ScalarType) -> Option<(u64, u64)> {
        let width = u64::from(crate::reference_storage_byte_width(scalar)?);
        Some((width, width))
    }

    fn field_geometry<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        field: &StructuralFieldDeclaration,
        storage: Option<&AuthoredFieldStorage>,
        active: &mut Vec<StructuralTypeId>,
        carriers: Option<TargetCarrierLayout>,
        records: &impl RecordStorageResolver,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
    ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
        if let Some(scalar) = field.field_type.scalar_type() {
            return scalar_geometry(
                storage
                    .and_then(AuthoredFieldStorage::stored_scalar)
                    .unwrap_or(scalar),
            )
            .map(Some)
            .ok_or(StorageGeometryError::UnsupportedScalar(scalar));
        }
        if let Some(carrier) = field.field_type.byte_sequence_carrier() {
            return byte_sequence_geometry(carrier, carriers).map(Some);
        }
        let child = field
            .field_type
            .as_structural()
            .ok_or(StorageGeometryError::UnsupportedField)?;
        geometry(types, child, active, carriers, records, extents)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StorageGeometryError {
        MissingType(StructuralTypeId),
        RecursiveType(StructuralTypeId),
        NestingLimit,
        UnsupportedType(StructuralTypeId),
        UnsupportedScalar(ScalarType),
        UnsupportedField,
        UnsupportedByteSequence(terminal_psi::ByteSequenceCarrier),
        UnresolvedExtent(ScalarApplicationId),
        UnresolvedLayout(StructuralTypeId),
        MissingFieldStorage(StructuralTypeId, semantic_vocabulary::StructuralFieldId),
        InvalidRecordStorage(StructuralTypeId),
        Overflow,
        MissingCarrierLayout,
        InvalidCarrierLayout,
        AggregateLayout(layout_plans::AggregateGeometryError),
    }

    impl std::fmt::Display for StorageGeometryError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "storage geometry: {self:?}")
        }
    }
    impl std::error::Error for StorageGeometryError {}

    pub trait RecordStorageResolver {
        fn realization(
            &self,
            record: StructuralTypeId,
            application: LayoutApplicationId,
        ) -> Result<&TargetLayoutRealization, StorageGeometryError>;
    }

    pub struct UnselectedRecordStorage;

    impl RecordStorageResolver for UnselectedRecordStorage {
        fn realization(
            &self,
            record: StructuralTypeId,
            _application: LayoutApplicationId,
        ) -> Result<&TargetLayoutRealization, StorageGeometryError> {
            Err(StorageGeometryError::UnresolvedLayout(record))
        }
    }

    pub struct ReferenceRecordField<'storage> {
        pub declaration: &'storage StructuralFieldDeclaration,
        pub storage: Option<&'storage AuthoredFieldStorage>,
        pub byte_offset: Option<u64>,
        pub byte_size: u64,
        pub byte_alignment: u64,
        pub repeated: Option<ReferenceRepeatedField>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReferenceRepeatedField {
        pub element_size: u64,
        pub element_count: u64,
        pub element_stride: u64,
    }

    pub struct ReferenceStorageGeometry<Types, Extents, Records = UnselectedRecordStorage> {
        types: Types,
        extents: Extents,
        carriers: Option<TargetCarrierLayout>,
        records: Records,
    }

    impl<'types, Types, Extents> ReferenceStorageGeometry<Types, Extents, UnselectedRecordStorage>
    where
        Types: Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    {
        pub fn new(types: Types, extents: Extents) -> Self {
            Self {
                types,
                extents,
                carriers: None,
                records: UnselectedRecordStorage,
            }
        }
    }

    impl<'types, Types, Extents, Records> ReferenceStorageGeometry<Types, Extents, Records>
    where
        Types: Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
        Records: RecordStorageResolver,
    {
        pub fn with_record_storage<Selected: RecordStorageResolver>(
            self,
            records: Selected,
        ) -> ReferenceStorageGeometry<Types, Extents, Selected> {
            ReferenceStorageGeometry {
                types: self.types,
                extents: self.extents,
                carriers: self.carriers,
                records,
            }
        }

        pub fn with_target_carriers(mut self, carriers: Option<TargetCarrierLayout>) -> Self {
            self.carriers = carriers;
            self
        }

        pub fn dependencies(
            &self,
            root: StructuralTypeId,
        ) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
            let mut dependencies = Vec::new();
            geometry(
                self.types.clone(),
                root,
                &mut Vec::new(),
                self.carriers,
                &self.records,
                &mut |application| {
                    if !dependencies.contains(&application) {
                        dependencies.push(application);
                    }
                    Ok(None)
                },
            )?;
            Ok(dependencies)
        }

        pub fn layout(
            &mut self,
            root: StructuralTypeId,
        ) -> Result<(u64, u64), StorageGeometryError> {
            geometry(
                self.types.clone(),
                root,
                &mut Vec::new(),
                self.carriers,
                &self.records,
                &mut |application| (self.extents)(application).map(Some),
            )?
            .ok_or(StorageGeometryError::UnresolvedLayout(root))
        }

        pub fn sum_layout(
            &mut self,
            root: StructuralTypeId,
        ) -> Result<layout_plans::ConventionalSumGeometry, StorageGeometryError> {
            let declaration = crate::structural_declaration(self.types.clone(), root)
                .ok_or(StorageGeometryError::MissingType(root))?;
            let (fields, cases) = match &declaration.shape {
                StructuralTypeShape::Sum { cases } => (&[][..], cases),
                StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
                _ => return Err(StorageGeometryError::UnsupportedType(root)),
            };
            sum_layout(
                self.types.clone(),
                fields,
                cases,
                &mut vec![root],
                self.carriers,
                &self.records,
                &mut |application| (self.extents)(application).map(Some),
            )?
            .ok_or(StorageGeometryError::UnresolvedLayout(root))
        }

        pub fn region(
            &mut self,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<ReferenceStorageRegion> {
            storage_region(
                self.types.clone(),
                root,
                path,
                &self.records,
                self.carriers,
                &mut self.extents,
            )
        }

        pub fn region_source_compatible(
            &mut self,
            source: StructuralTypeId,
            path: &[StructuralPathSegment],
            reference: StructuralTypeId,
            stored: Option<ScalarType>,
        ) -> bool {
            if stored.is_some() {
                return false;
            }
            let Some(region) = self.region(source, path) else {
                return false;
            };
            let Some(array) = region.array else {
                return false;
            };
            let Some(StructuralTypeShape::Reference { referent, .. }) =
                crate::structural_shape(self.types.clone(), reference)
            else {
                return false;
            };
            if !crate::references::reference_views::plain_storage(
                self.types.clone(),
                array.declaration,
                &mut Vec::new(),
            ) || !crate::references::reference_views::plain_storage(
                self.types.clone(),
                *referent,
                &mut Vec::new(),
            ) {
                return false;
            }
            let (Ok((source_size, source_align)), Ok((target_size, target_align))) =
                (self.layout(source), self.layout(*referent))
            else {
                return false;
            };
            target_align != 0
                && source_align.is_multiple_of(target_align)
                && region.byte_offset.is_multiple_of(target_align)
                && region.byte_length == target_size
                && region
                    .byte_offset
                    .checked_add(region.byte_length)
                    .is_some_and(|end| end <= source_size)
        }

        pub fn record_fields<'storage>(
            &'storage mut self,
            root: StructuralTypeId,
        ) -> Result<Vec<ReferenceRecordField<'storage>>, StorageGeometryError>
        where
            'types: 'storage,
        {
            let declaration = crate::structural_declaration(self.types.clone(), root)
                .ok_or(StorageGeometryError::MissingType(root))?;
            let StructuralTypeShape::Record { fields, storage } = &declaration.shape else {
                return Err(StorageGeometryError::UnsupportedType(root));
            };
            let selected = match storage {
                StructuralRecordStorage::TargetLayout(application) => {
                    Some(self.records.realization(root, *application)?)
                }
                StructuralRecordStorage::Packed | StructuralRecordStorage::Authored(_) => None,
                _ => return Err(StorageGeometryError::UnsupportedType(root)),
            };
            let mut output = Vec::with_capacity(fields.len());
            let mut packed_offset = 0;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let field_storage = match selected {
                    Some(realization) => Some(
                        &realization
                            .fields
                            .iter()
                            .find(|row| row.field == field.id)
                            .ok_or(StorageGeometryError::MissingFieldStorage(root, field.id))?
                            .storage,
                    ),
                    None => field.storage.as_ref(),
                };
                let (byte_size, byte_alignment) = field_geometry(
                    self.types.clone(),
                    field,
                    field_storage,
                    &mut Vec::new(),
                    self.carriers,
                    &self.records,
                    &mut |application| (self.extents)(application).map(Some),
                )?
                .ok_or(StorageGeometryError::UnresolvedLayout(root))?;
                let repeated = match field_storage {
                    Some(AuthoredFieldStorage::Repeated { element_stride, .. }) => {
                        let array = field
                            .field_type
                            .as_structural()
                            .ok_or(StorageGeometryError::InvalidRecordStorage(root))?;
                        let StructuralTypeShape::FixedArray { element, length } =
                            &crate::structural_declaration(self.types.clone(), array)
                                .ok_or(StorageGeometryError::MissingType(array))?
                                .shape
                        else {
                            return Err(StorageGeometryError::InvalidRecordStorage(root));
                        };
                        let element_count = match length {
                            terminal_psi::ArrayExtent::Literal(count) => *count,
                            terminal_psi::ArrayExtent::Application(application) => {
                                (self.extents)(*application)?
                            }
                        };
                        let element_size = geometry(
                            self.types.clone(),
                            *element,
                            &mut Vec::new(),
                            self.carriers,
                            &self.records,
                            &mut |application| (self.extents)(application).map(Some),
                        )?
                        .ok_or(StorageGeometryError::UnresolvedLayout(*element))?
                        .0;
                        Some(ReferenceRepeatedField {
                            element_size,
                            element_count,
                            element_stride: *element_stride,
                        })
                    }
                    _ => None,
                };
                let byte_offset = match storage {
                    StructuralRecordStorage::Packed => {
                        packed_offset = align(packed_offset, byte_alignment)
                            .ok_or(StorageGeometryError::Overflow)?;
                        let offset = packed_offset;
                        packed_offset = packed_offset
                            .checked_add(byte_size)
                            .ok_or(StorageGeometryError::Overflow)?;
                        Some(offset)
                    }
                    _ => field_storage
                        .ok_or(StorageGeometryError::MissingFieldStorage(root, field.id))?
                        .byte_offset(),
                };
                output.push(ReferenceRecordField {
                    declaration: field,
                    storage: field_storage,
                    byte_offset,
                    byte_size,
                    byte_alignment,
                    repeated,
                });
            }
            Ok(output)
        }

        pub fn record_field_dependencies(
            &self,
            root: StructuralTypeId,
        ) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
            let declaration = crate::structural_declaration(self.types.clone(), root)
                .ok_or(StorageGeometryError::MissingType(root))?;
            let StructuralTypeShape::Record { fields, storage } = &declaration.shape else {
                return Err(StorageGeometryError::UnsupportedType(root));
            };
            let selected = match storage {
                StructuralRecordStorage::TargetLayout(application) => {
                    Some(self.records.realization(root, *application)?)
                }
                StructuralRecordStorage::Packed | StructuralRecordStorage::Authored(_) => None,
                _ => return Err(StorageGeometryError::UnsupportedType(root)),
            };
            let mut dependencies = Vec::new();
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let field_storage = match selected {
                    Some(realization) => Some(
                        &realization
                            .fields
                            .iter()
                            .find(|row| row.field == field.id)
                            .ok_or(StorageGeometryError::MissingFieldStorage(root, field.id))?
                            .storage,
                    ),
                    None => field.storage.as_ref(),
                };
                field_geometry(
                    self.types.clone(),
                    field,
                    field_storage,
                    &mut Vec::new(),
                    self.carriers,
                    &self.records,
                    &mut |application| {
                        if !dependencies.contains(&application) {
                            dependencies.push(application);
                        }
                        Ok(None)
                    },
                )?;
            }
            Ok(dependencies)
        }
    }

    pub fn reference_storage_dependencies<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Result<Vec<ScalarApplicationId>, StorageGeometryError> {
        ReferenceStorageGeometry::new(types, |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .dependencies(root)
    }

    fn geometry<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        carriers: Option<TargetCarrierLayout>,
        records: &impl RecordStorageResolver,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
    ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
        if active.len() >= 128 {
            return Err(StorageGeometryError::NestingLimit);
        }
        if active.contains(&root) {
            return Err(StorageGeometryError::RecursiveType(root));
        }
        let shape = &crate::structural_declaration(types.clone(), root)
            .ok_or(StorageGeometryError::MissingType(root))?
            .shape;
        active.push(root);
        let result = (|| match shape {
            StructuralTypeShape::Reference { .. } => {
                let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
                carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
                    .map(Some)
            }
            StructuralTypeShape::ByteSequence(carrier) => {
                byte_sequence_geometry(*carrier, carriers).map(Some)
            }
            StructuralTypeShape::ElementView { .. } => {
                let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
                carrier_geometry(
                    carriers.descriptor_byte_size,
                    carriers.descriptor_byte_alignment,
                )
                .map(Some)
            }
            StructuralTypeShape::AtomicCell(cell) => scalar_geometry(cell.scalar_type())
                .map(Some)
                .ok_or(StorageGeometryError::UnsupportedScalar(cell.scalar_type())),
            StructuralTypeShape::PrimitiveScalar(scalar) => scalar_geometry(*scalar)
                .map(Some)
                .ok_or(StorageGeometryError::UnsupportedScalar(*scalar)),
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(application),
                ..
            } => {
                let selected = records.realization(root, *application)?;
                Ok(Some((
                    selected.storage.byte_size,
                    selected.storage.byte_alignment,
                )))
            }
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(storage),
                ..
            } => Ok(Some((storage.byte_size, storage.byte_alignment))),
            StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            } => {
                let mut size = Some(0);
                let mut alignment = 1;
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let child = field_geometry(
                        types.clone(),
                        field,
                        field.storage.as_ref(),
                        active,
                        carriers,
                        records,
                        extents,
                    )?;
                    let Some((child_size, child_alignment)) = child else {
                        size = None;
                        continue;
                    };
                    if let Some(current) = size {
                        size = Some(
                            align(current, child_alignment)
                                .and_then(|offset| offset.checked_add(child_size))
                                .ok_or(StorageGeometryError::Overflow)?,
                        );
                    }
                    alignment = alignment.max(child_alignment);
                }
                size.map(|size| {
                    Ok((
                        align(size, alignment).ok_or(StorageGeometryError::Overflow)?,
                        alignment,
                    ))
                })
                .transpose()
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = match length {
                    terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                    terminal_psi::ArrayExtent::Application(application) => extents(*application)?,
                };
                let element =
                    geometry(types.clone(), *element, active, carriers, records, extents)?;
                let (Some(length), Some((size, alignment))) = (length, element) else {
                    return Ok(None);
                };
                let size = align(size, alignment)
                    .and_then(|stride| stride.checked_mul(length))
                    .ok_or(StorageGeometryError::Overflow)?;
                Ok(Some((size, alignment)))
            }
            StructuralTypeShape::Sum { cases } => {
                sum_geometry(types, &[], cases, active, carriers, records, extents)
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                sum_geometry(types, fields, cases, active, carriers, records, extents)
            }
            _ => Err(StorageGeometryError::UnsupportedType(root)),
        })();
        active.pop();
        result
    }

    fn sum_geometry<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        common_fields: &[StructuralFieldDeclaration],
        cases: &[terminal_psi::StructuralCaseDeclaration],
        active: &mut Vec<StructuralTypeId>,
        carriers: Option<TargetCarrierLayout>,
        records: &impl RecordStorageResolver,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
    ) -> Result<Option<(u64, u64)>, StorageGeometryError> {
        Ok(sum_layout(
            types,
            common_fields,
            cases,
            active,
            carriers,
            records,
            extents,
        )?
        .map(|geometry| (geometry.storage.byte_size, geometry.storage.byte_alignment)))
    }

    fn sum_layout<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        common_fields: &[StructuralFieldDeclaration],
        cases: &[terminal_psi::StructuralCaseDeclaration],
        active: &mut Vec<StructuralTypeId>,
        carriers: Option<TargetCarrierLayout>,
        records: &impl RecordStorageResolver,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<Option<u64>, StorageGeometryError>,
    ) -> Result<Option<layout_plans::ConventionalSumGeometry>, StorageGeometryError> {
        let mut fields = |fields: &[StructuralFieldDeclaration]| {
            fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    field_geometry(
                        types.clone(),
                        field,
                        field.storage.as_ref(),
                        active,
                        carriers,
                        records,
                        extents,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let common = fields(common_fields)?;
        let payloads = cases
            .iter()
            .map(|case| fields(&case.fields))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(common) = common.into_iter().collect::<Option<Vec<_>>>() else {
            return Ok(None);
        };
        let Some(payloads) = payloads
            .into_iter()
            .map(|fields| fields.into_iter().collect::<Option<Vec<_>>>())
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
        let tag = carrier_geometry(
            carriers.case_tag_byte_size,
            carriers.case_tag_byte_alignment,
        )?;
        let shape = |(byte_size, byte_alignment)| layout_plans::StorageGeometry {
            byte_size,
            byte_alignment,
        };
        let geometry = layout_plans::conventional_sum_geometry(
            shape(tag),
            common.into_iter().map(shape),
            payloads
                .into_iter()
                .map(|fields| fields.into_iter().map(shape)),
        )
        .map_err(StorageGeometryError::AggregateLayout)?;
        Ok(Some(geometry))
    }

    fn carrier_geometry(size: u64, alignment: u64) -> Result<(u64, u64), StorageGeometryError> {
        if size == 0 || !alignment.is_power_of_two() || !size.is_multiple_of(alignment) {
            return Err(StorageGeometryError::InvalidCarrierLayout);
        }
        Ok((size, alignment))
    }

    fn byte_sequence_geometry(
        carrier: terminal_psi::ByteSequenceCarrier,
        carriers: Option<TargetCarrierLayout>,
    ) -> Result<(u64, u64), StorageGeometryError> {
        use terminal_psi::ByteSequenceCarrier;
        let carriers = carriers.ok_or(StorageGeometryError::MissingCarrierLayout)?;
        match carrier {
            ByteSequenceCarrier::BorrowedView { .. } => carrier_geometry(
                carriers.descriptor_byte_size,
                carriers.descriptor_byte_alignment,
            ),
            ByteSequenceCarrier::BoundedView { .. } => {
                carrier_geometry(carriers.pointer_byte_size, carriers.pointer_byte_alignment)
            }
            ByteSequenceCarrier::BoundedOwned { .. } => {
                Err(StorageGeometryError::UnsupportedByteSequence(carrier))
            }
        }
    }

    pub fn reference_storage_size<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<u64> {
        reference_storage_layout(types, root).map(|(size, _)| size)
    }

    pub fn reference_storage_layout<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<(u64, u64)> {
        ReferenceStorageGeometry::new(types, |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .layout(root)
        .ok()
    }

    pub fn reference_storage_projection<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorageProjection> {
        reference_storage_region(types, root, path)?.scalar
    }

    pub fn reference_storage_region<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorageRegion> {
        crate::ReferenceStorageGeometry::new(types, |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .region(root, path)
    }

    fn storage_region<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        records: &impl RecordStorageResolver,
        carriers: Option<TargetCarrierLayout>,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    ) -> Option<ReferenceStorageRegion> {
        let projection = storage_path(types.clone(), root, path, records, carriers, extents)?;
        if projection.fragments.is_some() {
            return None;
        }
        let size = match (projection.window, projection.scalar) {
            (Some(window), _) => window.byte_length,
            (None, Some((_, stored))) => scalar_geometry(stored)?.0,
            (None, None) => {
                selected_storage_layout(types, projection.carrier, records, carriers, extents)?.0
            }
        };
        Some(ReferenceStorageRegion {
            byte_offset: projection.byte_offset,
            byte_length: size,
            scalar: projection.scalar.map(|(logical_scalar, stored_scalar)| {
                ReferenceStorageProjection {
                    byte_offset: projection.byte_offset,
                    logical_scalar,
                    stored_scalar,
                }
            }),
            array: projection.window,
        })
    }

    fn selected_storage_layout<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        records: &impl RecordStorageResolver,
        carriers: Option<TargetCarrierLayout>,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    ) -> Option<(u64, u64)> {
        geometry(
            types,
            root,
            &mut Vec::new(),
            carriers,
            records,
            &mut |application| extents(application).map(Some),
        )
        .ok()?
    }

    pub fn reference_scalar_storage<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceScalarStorage<'types>> {
        let projection = storage_path(
            types,
            root,
            path,
            &UnselectedRecordStorage,
            None,
            &mut |application| Err(StorageGeometryError::UnresolvedExtent(application)),
        )?;
        let (logical_scalar, stored_scalar) = projection.scalar?;
        Some(match projection.fragments {
            Some(fragments) => ReferenceScalarStorage::Fragmented {
                byte_offset: projection.byte_offset,
                logical_scalar,
                fragments,
            },
            None => ReferenceScalarStorage::Contiguous(ReferenceStorageProjection {
                byte_offset: projection.byte_offset,
                logical_scalar,
                stored_scalar,
            }),
        })
    }

    struct StoragePathProjection<'recipe> {
        byte_offset: u64,
        carrier: StructuralTypeId,
        scalar: Option<(ScalarType, ScalarType)>,
        fragments: Option<&'recipe semantic_vocabulary::AuthoredBitFragments>,
        window: Option<ReferenceArrayWindow>,
    }

    fn storage_path<'types, 'storage>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        records: &'storage impl RecordStorageResolver,
        carriers: Option<TargetCarrierLayout>,
        extents: &mut impl FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    ) -> Option<StoragePathProjection<'storage>>
    where
        'types: 'storage,
    {
        let mut carrier = root;
        let mut byte_offset = 0_u64;
        let mut repeated_stride = None;
        let mut selected_storage = None;
        let mut selected_scalar = None;
        let mut fragments = None;
        let mut window: Option<ReferenceArrayWindow> = None;
        let mut segments = path.iter();
        while let Some(segment) = segments.next() {
            if let StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } = segment
            {
                if fragments.is_some() {
                    return None;
                }
                let source_size = if let Some(window) = window {
                    window.byte_length
                } else {
                    match selected_scalar {
                        Some(logical) => scalar_geometry(selected_storage.unwrap_or(logical))?.0,
                        None => match *crate::structural_shape(types.clone(), carrier)? {
                            StructuralTypeShape::PrimitiveScalar(logical) => {
                                scalar_geometry(selected_storage.unwrap_or(logical))?.0
                            }
                            _ => {
                                selected_storage_layout(
                                    types.clone(),
                                    carrier,
                                    records,
                                    carriers,
                                    extents,
                                )?
                                .0
                            }
                        },
                    }
                };
                let view = crate::representation_view_declaration(
                    types.clone(),
                    *structural_type,
                    *stored_scalar,
                )?;
                let view_size = match stored_scalar {
                    Some(scalar) => scalar_geometry(*scalar)?.0,
                    None => {
                        selected_storage_layout(types.clone(), view.id, records, carriers, extents)?
                            .0
                    }
                };
                if source_size != view_size {
                    return None;
                }
                carrier = view.id;
                selected_storage = *stored_scalar;
                selected_scalar = None;
                repeated_stride = None;
                window = None;
                continue;
            }
            if window.is_some() {
                return None;
            }
            if selected_scalar.is_some() {
                return None;
            }
            let shape = &crate::structural_shape(types.clone(), carrier)?;
            match (shape, segment) {
                (
                    StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. },
                    StructuralPathSegment::Field(_) | StructuralPathSegment::Case(_),
                ) => {
                    let (common, cases) = match shape {
                        StructuralTypeShape::Sum { cases } => (&[][..], cases),
                        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
                        _ => return None,
                    };
                    let layout = sum_layout(
                        types.clone(),
                        common,
                        cases,
                        &mut vec![carrier],
                        carriers,
                        records,
                        &mut |application| extents(application).map(Some),
                    )
                    .ok()??;
                    let (field, offset) = match segment {
                        StructuralPathSegment::Field(identity) => {
                            let (ordinal, field) =
                                terminal_psi::live_field_entry(common, *identity)?;
                            (field, *layout.common_field_offsets.get(ordinal)?)
                        }
                        StructuralPathSegment::Case(case) => {
                            let (ordinal, selected) =
                                cases.iter().enumerate().find(|(_, row)| row.id == *case)?;
                            let StructuralPathSegment::Field(identity) = segments.next()? else {
                                return None;
                            };
                            let (field_ordinal, field) =
                                terminal_psi::live_field_entry(&selected.fields, *identity)?;
                            (
                                field,
                                *layout.case_field_offsets.get(ordinal)?.get(field_ordinal)?,
                            )
                        }
                        _ => return None,
                    };
                    let recipe = field.storage.as_ref();
                    if matches!(recipe, Some(AuthoredFieldStorage::Fragmented(_))) {
                        return None;
                    }
                    byte_offset = byte_offset.checked_add(offset)?;
                    selected_storage = recipe.and_then(AuthoredFieldStorage::stored_scalar);
                    repeated_stride = recipe.and_then(AuthoredFieldStorage::element_stride);
                    if let Some(logical_scalar) = field.field_type.scalar_type() {
                        selected_scalar = Some(logical_scalar);
                        continue;
                    }
                    carrier = field.field_type.as_structural()?;
                }
                (
                    StructuralTypeShape::Record { fields, storage },
                    StructuralPathSegment::Field(identity),
                ) => {
                    let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                    let recipe = match storage {
                        StructuralRecordStorage::TargetLayout(application) => Some(
                            &records
                                .realization(carrier, *application)
                                .ok()?
                                .fields
                                .iter()
                                .find(|row| row.field == field.id)?
                                .storage,
                        ),
                        _ => field.storage.as_ref(),
                    };
                    let offset = match storage {
                        terminal_psi::StructuralRecordStorage::AccessView(_)
                        | terminal_psi::StructuralRecordStorage::Accessor(_) => return None,
                        StructuralRecordStorage::TargetLayout(_)
                        | StructuralRecordStorage::Authored(_) => match recipe? {
                            AuthoredFieldStorage::Fragmented(recipe) => {
                                fragments = Some(recipe);
                                0
                            }
                            storage => storage.byte_offset()?,
                        },
                        StructuralRecordStorage::Packed => {
                            let mut offset = 0;
                            for candidate in
                                fields.iter().filter(|field| !field.relevance.is_erased())
                            {
                                let (size, alignment) = field_geometry(
                                    types.clone(),
                                    candidate,
                                    candidate.storage.as_ref(),
                                    &mut Vec::new(),
                                    carriers,
                                    records,
                                    &mut |application| extents(application).map(Some),
                                )
                                .ok()??;
                                offset = align(offset, alignment)?;
                                if candidate.id == field.id {
                                    break;
                                }
                                offset = offset.checked_add(size)?;
                            }
                            offset
                        }
                    };
                    byte_offset = byte_offset.checked_add(offset)?;
                    selected_storage = recipe.and_then(AuthoredFieldStorage::stored_scalar);
                    repeated_stride = recipe.and_then(AuthoredFieldStorage::element_stride);
                    if let Some(logical_scalar) = field.field_type.scalar_type() {
                        selected_scalar = Some(logical_scalar);
                        continue;
                    }
                    let child = field.field_type.as_structural()?;
                    carrier = child;
                }
                (
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                    StructuralPathSegment::FixedIndex(index),
                ) if index < length => {
                    let (size, alignment) = selected_storage_layout(
                        types.clone(),
                        *element,
                        records,
                        carriers,
                        extents,
                    )?;
                    let stride = repeated_stride.take().unwrap_or(align(size, alignment)?);
                    byte_offset = byte_offset.checked_add(index.checked_mul(stride)?)?;
                    carrier = *element;
                    selected_storage = None;
                }
                (
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                    StructuralPathSegment::FixedRange { start, end },
                ) if start <= end && end <= length => {
                    let (size, alignment) = selected_storage_layout(
                        types.clone(),
                        *element,
                        records,
                        carriers,
                        extents,
                    )?;
                    let stride = repeated_stride.take().unwrap_or(align(size, alignment)?);
                    if stride < size {
                        return None;
                    }
                    let array_offset = start.checked_mul(stride)?;
                    byte_offset = byte_offset.checked_add(array_offset)?;
                    let count = end.checked_sub(*start)?;
                    let byte_length = if count == 0 {
                        0
                    } else {
                        count
                            .checked_sub(1)?
                            .checked_mul(stride)?
                            .checked_add(size)?
                    };
                    window = Some(ReferenceArrayWindow {
                        declaration: carrier,
                        byte_offset: array_offset,
                        byte_length,
                    });
                    selected_storage = None;
                }
                _ => return None,
            }
        }
        let scalar = match selected_scalar {
            Some(logical) => Some((logical, selected_storage.unwrap_or(logical))),
            None => match *crate::structural_shape(types.clone(), carrier)? {
                StructuralTypeShape::PrimitiveScalar(logical) => {
                    Some((logical, selected_storage.unwrap_or(logical)))
                }
                _ => None,
            },
        };
        Some(StoragePathProjection {
            byte_offset,
            carrier,
            scalar,
            fragments,
            window,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            AuthoredRecordStorage, IntegerSign, IntegerType, StructuralFieldId,
        };
        use terminal_psi::{BindingRelevance, StructuralFieldType, TerminalIdentityIndex};

        fn identity(value: u64) -> StructuralTypeId {
            StructuralTypeId::new(value).expect("type identity")
        }
        fn scalar(bits: u16) -> ScalarType {
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).expect("signed scalar"))
        }
        fn field(
            value: u64,
            field_type: StructuralFieldType,
            storage: Option<AuthoredFieldStorage>,
        ) -> StructuralFieldDeclaration {
            StructuralFieldDeclaration {
                inline_carrier: None,
                id: StructuralFieldId::new(value).expect("field identity"),
                identity: TerminalIdentityIndex(4),
                relevance: BindingRelevance::Relevant,
                field_type,
                storage,
            }
        }

        #[test]
        fn sum_image_offsets_and_array_extent_share_the_selected_tag_geometry() {
            let declaration = |value, shape| StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(value),
                identity: TerminalIdentityIndex(value as u32),
                shape,
            };
            let types = [
                declaration(
                    1,
                    StructuralTypeShape::Mixed {
                        fields: vec![field(1, StructuralFieldType::Scalar(scalar(16)), None)],
                        cases: vec![
                            terminal_psi::StructuralCaseDeclaration {
                                id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                                identity: TerminalIdentityIndex(10),
                                fields: vec![],
                            },
                            terminal_psi::StructuralCaseDeclaration {
                                id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                                identity: TerminalIdentityIndex(11),
                                fields: vec![
                                    field(1, StructuralFieldType::Scalar(scalar(8)), None),
                                    StructuralFieldDeclaration {
                                        identity: TerminalIdentityIndex(5),
                                        ..field(2, StructuralFieldType::Scalar(scalar(64)), None)
                                    },
                                ],
                            },
                        ],
                    },
                ),
                declaration(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: identity(1),
                        length: terminal_psi::ArrayExtent::Literal(3),
                    },
                ),
                declaration(
                    3,
                    StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::Packed,
                        fields: vec![
                            field(1, StructuralFieldType::Scalar(scalar(32)), None),
                            StructuralFieldDeclaration {
                                identity: TerminalIdentityIndex(5),
                                ..field(2, StructuralFieldType::Structural(identity(1)), None)
                            },
                        ],
                    },
                ),
                declaration(
                    4,
                    StructuralTypeShape::Sum {
                        cases: vec![terminal_psi::StructuralCaseDeclaration {
                            id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                            identity: TerminalIdentityIndex(11),
                            fields: vec![StructuralFieldDeclaration {
                                identity: TerminalIdentityIndex(5),
                                ..field(
                                    2,
                                    StructuralFieldType::Scalar(scalar(64)),
                                    Some(AuthoredFieldStorage::Scalar {
                                        byte_offset: 0,
                                        stored_scalar: Some(scalar(32)),
                                    }),
                                )
                            }],
                        }],
                    },
                ),
            ];
            let unresolved = |application| Err(StorageGeometryError::UnresolvedExtent(application));
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), unresolved).sum_layout(identity(1)),
                Err(StorageGeometryError::MissingCarrierLayout)
            );
            for (tag_size, common, payload, extent) in [(4, 4, 8, 24), (8, 8, 16, 32)] {
                let mut geometry = ReferenceStorageGeometry::new(types.iter(), unresolved)
                    .with_target_carriers(Some(TargetCarrierLayout {
                        pointer_byte_size: 8,
                        pointer_byte_alignment: 8,
                        descriptor_byte_size: 16,
                        descriptor_byte_alignment: 8,
                        case_tag_byte_size: tag_size,
                        case_tag_byte_alignment: tag_size,
                    }));
                let sum = geometry.sum_layout(identity(1)).unwrap();
                assert_eq!(sum.tag_byte_offset, 0);
                assert_eq!(sum.tag.byte_size, tag_size);
                assert_eq!(sum.tag.byte_alignment, tag_size);
                assert_eq!(sum.common_field_offsets, vec![common]);
                assert_eq!(
                    sum.case_field_offsets,
                    vec![vec![], vec![payload, payload + 8]]
                );
                assert_eq!(geometry.layout(identity(1)), Ok((extent, 8)));
                assert_eq!(sum.storage.byte_size, extent);
                assert_eq!(geometry.layout(identity(2)), Ok((3 * extent, 8)));
                let common_region = geometry
                    .region(
                        identity(1),
                        &[StructuralPathSegment::Field(TerminalIdentityIndex(4))],
                    )
                    .unwrap();
                assert_eq!(
                    (common_region.byte_offset, common_region.byte_length),
                    (common, 2)
                );
                let selected = [
                    StructuralPathSegment::Case(
                        semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                    ),
                    StructuralPathSegment::Field(TerminalIdentityIndex(5)),
                ];
                let region = geometry.region(identity(1), &selected).unwrap();
                assert_eq!((region.byte_offset, region.byte_length), (payload + 8, 8));
                assert_eq!(region.scalar.unwrap().logical_scalar, scalar(64));
                let narrow = geometry.region(identity(4), &selected).unwrap();
                assert_eq!((narrow.byte_offset, narrow.byte_length), (tag_size, 4));
                assert_eq!(narrow.scalar.unwrap().logical_scalar, scalar(64));
                assert_eq!(narrow.scalar.unwrap().stored_scalar, scalar(32));
                let mut nested = vec![StructuralPathSegment::Field(TerminalIdentityIndex(5))];
                nested.extend_from_slice(&selected);
                let region = geometry.region(identity(3), &nested).unwrap();
                assert_eq!(
                    (region.byte_offset, region.byte_length),
                    (8 + payload + 8, 8)
                );
                let mut indexed = vec![StructuralPathSegment::FixedIndex(2)];
                indexed.extend_from_slice(&selected);
                let region = geometry.region(identity(2), &indexed).unwrap();
                assert_eq!(
                    (region.byte_offset, region.byte_length),
                    (2 * extent + payload + 8, 8)
                );
                assert!(geometry.region(identity(1), &selected[..1]).is_none());
                assert!(
                    geometry
                        .region(
                            identity(1),
                            &[
                                StructuralPathSegment::Case(
                                    semantic_vocabulary::StructuralCaseId::new(1).unwrap()
                                ),
                                StructuralPathSegment::Field(TerminalIdentityIndex(5)),
                            ]
                        )
                        .is_none()
                );
            }
        }

        #[test]
        fn extent_discovery_visits_unknown_siblings_but_respects_storage_and_erasure() {
            let application = |value| ScalarApplicationId::new(value).unwrap();
            let declaration = |value, shape| StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(value),
                identity: TerminalIdentityIndex(value as u32),
                shape,
            };
            let mut types = [
                declaration(1, StructuralTypeShape::PrimitiveScalar(scalar(64))),
                declaration(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: identity(1),
                        length: terminal_psi::ArrayExtent::Application(application(1)),
                    },
                ),
                declaration(
                    3,
                    StructuralTypeShape::FixedArray {
                        element: identity(1),
                        length: terminal_psi::ArrayExtent::Application(application(2)),
                    },
                ),
                declaration(
                    4,
                    StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::Packed,
                        fields: vec![
                            field(1, StructuralFieldType::Structural(identity(2)), None),
                            field(2, StructuralFieldType::Structural(identity(3)), None),
                        ],
                    },
                ),
            ];
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(4)),
                Ok(vec![application(1), application(2)])
            );
            assert_eq!(
                reference_storage_layout(types.iter(), identity(4)),
                None,
                "discovery cannot publish unknown geometry"
            );
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |dependency| Ok(dependency.get()))
                    .layout(identity(4)),
                Ok((24, 8))
            );
            let StructuralTypeShape::Record { fields, .. } = &mut types[3].shape else {
                panic!("record");
            };
            fields[0].relevance = BindingRelevance::Erased;
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(4)),
                Ok(vec![application(2)])
            );
            let StructuralTypeShape::Record { storage, .. } = &mut types[3].shape else {
                panic!("record");
            };
            *storage = StructuralRecordStorage::Authored(AuthoredRecordStorage {
                byte_size: 3,
                byte_alignment: 1,
            });
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(4)),
                Ok(Vec::new())
            );
            assert_eq!(
                reference_storage_layout(types.iter(), identity(4)),
                Some((3, 1))
            );

            types[1].shape = StructuralTypeShape::FixedArray {
                element: identity(3),
                length: terminal_psi::ArrayExtent::Literal(0),
            };
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(2)),
                Ok(vec![application(2)])
            );
            types[2].shape = StructuralTypeShape::FixedArray {
                element: identity(2),
                length: terminal_psi::ArrayExtent::Literal(0),
            };
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(2)),
                Err(StorageGeometryError::RecursiveType(identity(2)))
            );
            types[2].shape = StructuralTypeShape::FixedArray {
                element: identity(99),
                length: terminal_psi::ArrayExtent::Literal(0),
            };
            assert_eq!(
                reference_storage_dependencies(types.iter(), identity(2)),
                Err(StorageGeometryError::MissingType(identity(99)))
            );
        }

        #[test]
        fn resolved_array_extent_uses_the_same_geometry_and_reports_missing_or_overflow() {
            let dependency = ScalarApplicationId::new(1).unwrap();
            let mut types = [
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    copyable: true,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: identity(1),
                    identity: TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::PrimitiveScalar(scalar(64)),
                },
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    copyable: true,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: identity(2),
                    identity: TerminalIdentityIndex(2),
                    shape: StructuralTypeShape::FixedArray {
                        element: identity(1),
                        length: terminal_psi::ArrayExtent::Application(dependency),
                    },
                },
            ];
            assert_eq!(reference_storage_layout(types.iter(), identity(2)), None);
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |application| Err(
                    StorageGeometryError::UnresolvedExtent(application)
                ))
                .layout(identity(2)),
                Err(StorageGeometryError::UnresolvedExtent(dependency))
            );
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
                Ok((24, 8))
            );
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |_| Ok(0)).layout(identity(2)),
                Ok((0, 8))
            );
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |_| Ok(u64::MAX)).layout(identity(2)),
                Err(StorageGeometryError::Overflow)
            );
            types[1].shape = StructuralTypeShape::FixedArray {
                element: identity(2),
                length: terminal_psi::ArrayExtent::Application(dependency),
            };
            assert_eq!(
                ReferenceStorageGeometry::new(types.iter(), |_| Ok(3)).layout(identity(2)),
                Err(StorageGeometryError::RecursiveType(identity(2)))
            );
        }

        #[test]
        fn canonical_parent_keeps_authored_child_width_and_codec() {
            let types = [
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: identity(1),
                    identity: TerminalIdentityIndex(3),
                    shape: StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                            byte_size: 1,
                            byte_alignment: 1,
                        }),
                        fields: vec![field(
                            1,
                            StructuralFieldType::Scalar(scalar(64)),
                            Some(AuthoredFieldStorage::Scalar {
                                byte_offset: 0,
                                stored_scalar: Some(scalar(8)),
                            }),
                        )],
                    },
                },
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: identity(2),
                    identity: TerminalIdentityIndex(2),
                    shape: StructuralTypeShape::Record {
                        storage: StructuralRecordStorage::Packed,
                        fields: vec![field(2, StructuralFieldType::Structural(identity(1)), None)],
                    },
                },
            ];
            assert_eq!(reference_storage_size(types.iter(), identity(2)), Some(1));
            assert_eq!(
                reference_storage_projection(
                    types.iter(),
                    identity(2),
                    &[
                        StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                        StructuralPathSegment::Field(TerminalIdentityIndex(4))
                    ]
                ),
                Some(ReferenceStorageProjection {
                    byte_offset: 0,
                    logical_scalar: scalar(64),
                    stored_scalar: scalar(8)
                })
            );
        }

        #[test]
        fn fragmented_fields_do_not_hide_the_record_extent_or_invent_a_scalar_address() {
            let types = [StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: identity(1),
                identity: TerminalIdentityIndex(1),
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 1,
                        byte_alignment: 1,
                    }),
                    fields: vec![field(
                        1,
                        StructuralFieldType::Scalar(scalar(8)),
                        Some(AuthoredFieldStorage::Fragmented(
                            semantic_vocabulary::AuthoredBitFragments::new([
                                semantic_vocabulary::AuthoredBitFragment {
                                    container_byte_offset: 0,
                                    container_width_bits: 8,
                                    destination_lsb: 4,
                                    source_lsb: 0,
                                    width: 4,
                                },
                                semantic_vocabulary::AuthoredBitFragment {
                                    container_byte_offset: 0,
                                    container_width_bits: 8,
                                    destination_lsb: 0,
                                    source_lsb: 4,
                                    width: 4,
                                },
                            ]),
                        )),
                    )],
                },
            }];
            assert_eq!(reference_storage_size(types.iter(), identity(1)), Some(1));
            assert_eq!(
                reference_storage_projection(
                    types.iter(),
                    identity(1),
                    &[StructuralPathSegment::Field(TerminalIdentityIndex(4))]
                ),
                None
            );
        }
    }
}
pub(crate) mod reference_views {

    use crate::descriptors::descriptor_results::descriptor_value_access;
    use crate::exactly_one::exactly_one;
    use crate::reference_storage_layout;
    use crate::runtime_structural_path_tip;
    use semantic_vocabulary::IntegerSign;
    use semantic_vocabulary::{ScalarType, StructuralTypeId};

    pub fn shared_descriptor_copy_matches<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        occurrence_access: Option<StructuralAccess>,
        destination: StructuralTypeId,
        destination_multiplicity: terminal_psi::StructuralMultiplicity,
    ) -> bool {
        let Some(source) = crate::structural_declaration(types.clone(), source) else {
            return false;
        };
        let Some(destination) = crate::structural_declaration(types.clone(), destination) else {
            return false;
        };
        descriptor_geometry_matches(&source.shape, &destination.shape)
            && descriptor_declared_access(&source.shape)
                .flatten()
                .or(occurrence_access)
                == Some(StructuralAccess::SharedBorrow)
            && descriptor_value_access(types, destination.id, destination_multiplicity)
                == Some(StructuralAccess::SharedBorrow)
    }

    pub fn descriptor_value_transfer_matches<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        destination: StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
    ) -> bool {
        let Some(source_access) = descriptor_value_access(types.clone(), source, multiplicity)
        else {
            return false;
        };
        descriptor_value_access(types.clone(), destination, multiplicity) == Some(source_access)
            && crate::borrowed_referent_presentation(types, source, destination)
                == Some(crate::BorrowedReferentPresentation::Unchanged)
    }

    pub fn descriptor_reborrow_matches<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        source_access: StructuralAccess,
        source_occurrence_access: Option<StructuralAccess>,
        destination: StructuralTypeId,
        requested_access: StructuralAccess,
    ) -> bool {
        let Some(source) = crate::structural_declaration(types.clone(), source) else {
            return false;
        };
        let Some(destination) = crate::structural_declaration(types.clone(), destination) else {
            return false;
        };
        descriptor_reborrow_shapes_match(
            &source.shape,
            source_access,
            source_occurrence_access,
            &destination.shape,
            requested_access,
        )
    }

    pub fn descriptor_reborrow_shapes_match(
        source: &StructuralTypeShape,
        source_access: StructuralAccess,
        source_occurrence_access: Option<StructuralAccess>,
        destination: &StructuralTypeShape,
        requested_access: StructuralAccess,
    ) -> bool {
        let source_ceiling = descriptor_declared_access(source).flatten();
        let destination_ceiling = descriptor_declared_access(destination).flatten();
        descriptor_geometry_matches(source, destination)
            && requested_access.admits(source_access)
            && source_occurrence_access
                .or(source_ceiling)
                .is_some_and(|access| requested_access.admits_referent(access))
            && source_ceiling.is_none_or(|access| requested_access.admits_referent(access))
            && destination_ceiling.is_none_or(|access| requested_access.admits_referent(access))
    }

    pub fn stored_reference_access(shape: &StructuralTypeShape) -> Option<StructuralAccess> {
        match shape {
            StructuralTypeShape::Reference { access, .. } => Some(*access),
            _ => descriptor_declared_access(shape).flatten(),
        }
    }

    pub(crate) fn descriptor_declared_access(
        shape: &StructuralTypeShape,
    ) -> Option<Option<StructuralAccess>> {
        match shape {
            StructuralTypeShape::ElementView { access, .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
            ) => Some(*access),
            _ => None,
        }
    }

    pub fn descriptor_geometry_matches(
        source: &StructuralTypeShape,
        destination: &StructuralTypeShape,
    ) -> bool {
        use terminal_psi::ByteSequenceCarrier;
        match (source, destination) {
            (
                StructuralTypeShape::ElementView {
                    element: source, ..
                },
                StructuralTypeShape::ElementView {
                    element: destination,
                    ..
                },
            ) => source == destination,
            (
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
            ) => true,
            (
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity: source,
                    ..
                }),
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity: destination,
                    ..
                }),
            ) => source == destination,
            _ => false,
        }
    }

    pub fn is_copyable_carrier<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        structural_type: StructuralTypeId,
    ) -> bool {
        let mut declarations = types.clone().filter(|row| row.id == structural_type);
        let declared_copyable = declarations
            .next()
            .is_some_and(|row| row.copyable && !row.owns_linear_claim)
            && declarations.next().is_none();
        declared_copyable || is_shared_reference_carrier(types, structural_type)
    }

    pub fn is_shared_reference_carrier<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        mut structural_type: StructuralTypeId,
    ) -> bool {
        for _ in 0..types.clone().count() {
            let mut matching = types.clone().filter(|row| row.id == structural_type);
            let Some(declaration) = matching.next() else {
                return false;
            };
            if matching.next().is_some() || declaration.quotient_carrier.is_some() {
                return false;
            }
            match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => {
                    return access == StructuralAccess::SharedBorrow;
                }
                StructuralTypeShape::ElementView {
                    access: Some(access),
                    ..
                }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(access),
                    }
                    | terminal_psi::ByteSequenceCarrier::BoundedView {
                        access: Some(access),
                        ..
                    },
                ) => {
                    return access == StructuralAccess::SharedBorrow;
                }
                StructuralTypeShape::FixedArray { element, .. } => structural_type = element,
                _ => return false,
            }
        }
        false
    }

    pub fn representation_view_declaration<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        view: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> Option<&'types StructuralTypeDeclaration> {
        let declaration = exactly_one(types, |declaration| declaration.id == view)?;
        match (stored, &declaration.shape) {
            (Some(stored), StructuralTypeShape::PrimitiveScalar(logical))
                if reference_storage_view_is_compatible(stored, *logical) => {}
            (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
            }
            _ => return None,
        }
        Some(declaration)
    }

    pub fn reference_source_storage_path<'types, 'path>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &'path [terminal_psi::StructuralPathSegment],
    ) -> &'path [terminal_psi::StructuralPathSegment] {
        if let Some((terminal_psi::StructuralPathSegment::Referent, prefix)) = path.split_last()
            && (runtime_structural_path_tip(types.clone(), root, prefix).is_some_and(|tip| {
                !matches!(
                    tip.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                )
            }) || crate::runtime_structural_field(types, root, prefix)
                .is_some_and(|field| field.field_type.byte_sequence_carrier().is_some()))
        {
            prefix
        } else {
            path
        }
    }

    pub fn reference_source_projection_compatible<'types>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        reference: StructuralTypeId,
    ) -> bool {
        let path = reference_source_storage_path(types.clone(), root, path);
        if let Some(StructuralTypeShape::Reference {
            access: StructuralAccess::SharedBorrow,
            referent,
        }) = crate::structural_shape(types.clone(), reference)
            && let Some(target) = crate::structural_declaration(types.clone(), *referent)
            && matches!(target.shape, StructuralTypeShape::PrimitiveScalar(_))
        {
            let Some(source) = crate::borrowed_primitive_projection_declaration(
                types.clone(),
                root,
                path,
                Some(*referent),
            ) else {
                return false;
            };
            return (source == *referent || target.scalar_restriction.is_none())
                && reference_source_compatible(types, source, reference);
        }
        if let Some(field) = crate::runtime_structural_field(types.clone(), root, path)
            && let Some(carrier) = field.field_type.byte_sequence_carrier()
            && let Some(StructuralTypeShape::Reference { referent, .. }) =
                crate::structural_shape(types.clone(), reference)
            && crate::structural_declaration(types.clone(), *referent)
                .is_some_and(|row| row.shape == StructuralTypeShape::ByteSequence(carrier))
        {
            return true;
        }
        if let Some(source) = crate::runtime_structural_path_tip(types.clone(), root, path) {
            return reference_source_compatible(types, source.id, reference);
        }
        if let Some(source) = crate::primitive_projection_declaration(types.clone(), root, path) {
            return reference_source_compatible(types, source, reference);
        }
        let Some(field) = crate::runtime_structural_field(types.clone(), root, path) else {
            return false;
        };
        let Some(carrier) = field.field_type.byte_sequence_carrier() else {
            return false;
        };
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(types.clone(), reference)
        else {
            return false;
        };
        crate::structural_declaration(types.clone(), *referent).is_some_and(|row| {
            row.shape == terminal_psi::StructuralTypeShape::ByteSequence(carrier)
        })
    }

    pub fn reference_source_projection_accessible<'types>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
        mut authority: terminal_psi::StructuralAccess,
        path: &[terminal_psi::StructuralPathSegment],
        requested: terminal_psi::StructuralAccess,
    ) -> bool {
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralPathSegment, StructuralTypeShape,
        };
        if authority == StructuralAccess::Owned
            && multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && crate::descriptors::descriptor_results::is_shared_descriptor(types.clone(), root)
        {
            authority = StructuralAccess::SharedBorrow;
        }
        if !requested.admits(authority) {
            return false;
        }
        let path = reference_source_storage_path(types.clone(), root, path);
        let mut region_start = 0;
        for (position, segment) in path.iter().enumerate() {
            let prefix = &path[..position];
            match segment {
                StructuralPathSegment::Referent => {
                    let Some(StructuralTypeShape::Reference { access, .. }) =
                        runtime_structural_path_tip(types.clone(), root, prefix)
                            .map(|declaration| &declaration.shape)
                    else {
                        return false;
                    };
                    if !StructuralAccess::SharedBorrow.admits(authority)
                        || !requested.admits_referent(*access)
                    {
                        return false;
                    }
                    authority = *access;
                    region_start = position + 1;
                }
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. } => {
                    let declaration = runtime_structural_path_tip(types.clone(), root, prefix);
                    if let Some(StructuralTypeShape::ElementView { access, .. }) =
                        declaration.map(|row| &row.shape)
                    {
                        match access {
                            Some(access) if !requested.admits_referent(*access) => return false,
                            None if position != region_start
                                && requested != StructuralAccess::SharedBorrow =>
                            {
                                return false;
                            }
                            _ => {}
                        }
                    }
                    let carrier = crate::runtime_structural_field(types.clone(), root, prefix)
                        .and_then(|field| field.field_type.byte_sequence_carrier())
                        .or_else(|| {
                            declaration.and_then(|row| match row.shape {
                                StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                                _ => None,
                            })
                        });
                    if let Some(
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(access),
                        }
                        | ByteSequenceCarrier::BoundedView {
                            access: Some(access),
                            ..
                        },
                    ) = carrier
                        && !requested.admits_referent(access)
                    {
                        return false;
                    }
                }
                _ => {}
            }
        }
        true
    }

    pub fn descriptor_source_projection_accessible<'types>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
        authority: terminal_psi::StructuralAccess,
        path: &[terminal_psi::StructuralPathSegment],
        requested: terminal_psi::StructuralAccess,
    ) -> bool {
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralPathSegment, StructuralTypeShape,
        };
        if !reference_source_projection_accessible(
            types.clone(),
            root,
            multiplicity,
            authority,
            path,
            requested,
        ) {
            return false;
        }
        let path = reference_source_storage_path(types.clone(), root, path);
        let region_start = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
            .map_or(0, |position| position + 1);
        if let Some(StructuralTypeShape::ElementView { access, .. }) =
            runtime_structural_path_tip(types.clone(), root, path).map(|row| &row.shape)
        {
            match access {
                Some(access) if !requested.admits_referent(*access) => return false,
                None if path.len() != region_start
                    && requested != StructuralAccess::SharedBorrow =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let carrier = crate::runtime_structural_field(types.clone(), root, path)
            .and_then(|field| field.field_type.byte_sequence_carrier())
            .or_else(|| {
                runtime_structural_path_tip(types.clone(), root, path).and_then(|row| {
                    match row.shape {
                        StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                        _ => None,
                    }
                })
            });
        if let Some(
            ByteSequenceCarrier::BorrowedView { access }
            | ByteSequenceCarrier::BoundedView { access, .. },
        ) = carrier
        {
            match access {
                Some(access) if !requested.admits_referent(access) => return false,
                None if path.len() != region_start
                    && requested != StructuralAccess::SharedBorrow =>
                {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    pub fn reference_byte_field_compatible<'types, 'domains>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domains terminal_psi::StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        reference: StructuralTypeId,
        memberships: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) =
            crate::structural_shape(types.clone(), reference)
        else {
            return false;
        };
        let Some(terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
        )) = crate::structural_shape(types.clone(), *referent)
        else {
            return false;
        };
        let Some(field) = crate::runtime_structural_field(types, root, path) else {
            return false;
        };
        if field.field_type.byte_sequence_carrier()
            != Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: *capacity,
            })
            || memberships
                .iter()
                .any(|row| row.path != [terminal_psi::StructuralPathSegment::Referent])
        {
            return false;
        }
        let qualifications = memberships.iter().map(|row| row.domain).collect::<Vec<_>>();
        let Some(required) =
            crate::byte_qualification_predicates_in(domains, *referent, &qualifications)
        else {
            return false;
        };
        let predicates = field.field_type.byte_predicates();
        predicates.implies(required)
            && (*access == terminal_psi::StructuralAccess::SharedBorrow
                || required.implies(predicates))
    }
    pub fn reference_storage_byte_width(stored: ScalarType) -> Option<u8> {
        match stored {
            ScalarType::Integer(integer) if !integer.is_address() && integer.bits() % 8 == 0 => {
                u8::try_from(integer.bits() / 8).ok()
            }
            ScalarType::Boolean => Some(1),
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
            _ => None,
        }
    }

    pub fn reference_storage_view_is_compatible(stored: ScalarType, logical: ScalarType) -> bool {
        if reference_storage_byte_width(stored).is_none() {
            return false;
        }
        if scalar_reference_view_is_total(stored, logical) {
            return true;
        }
        let (ScalarType::Integer(stored), ScalarType::Integer(logical)) = (stored, logical) else {
            return false;
        };
        if logical.is_address() || logical.bits() < stored.bits() {
            return false;
        }
        match (stored.sign(), logical.sign()) {
            (IntegerSign::Signed, IntegerSign::Unsigned) => false,
            (IntegerSign::Unsigned, IntegerSign::Signed) => logical.bits() > stored.bits(),
            _ => true,
        }
    }

    pub fn reference_storage_export_byte_width(
        logical: ScalarType,
        access: StructuralAccess,
    ) -> Option<u8> {
        let width = reference_storage_byte_width(logical)?;
        if access == StructuralAccess::MutableBorrow {
            return Some(width);
        }
        let same_width = |stored| {
            !reference_storage_view_is_compatible(stored, logical)
                || reference_storage_byte_width(stored) == Some(width)
        };
        for stored in [
            ScalarType::Boolean,
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
        ] {
            if !same_width(stored) {
                return None;
            }
        }
        for bits in (8..=128).step_by(8) {
            for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                let integer = semantic_vocabulary::IntegerType::new(sign, bits).ok()?;
                if !same_width(ScalarType::Integer(integer)) {
                    return None;
                }
            }
        }
        Some(width)
    }

    pub fn reference_storage_region_source_compatible<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        crate::ReferenceStorageGeometry::new(types, |application| {
            Err(crate::StorageGeometryError::UnresolvedExtent(application))
        })
        .region_source_compatible(source, path, reference, stored)
    }

    pub fn reference_storage_source_compatible<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        reference: StructuralTypeId,
        stored: Option<ScalarType>,
    ) -> bool {
        let Some(StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(types.clone(), reference)
        else {
            return false;
        };
        let Some(source_shape) = crate::structural_shape(types.clone(), source) else {
            return false;
        };
        let byte_endpoint = matches!(source_shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
            if !integer.is_address() && integer.bits() == 8
                && integer.sign() == IntegerSign::Unsigned);
        let Some(target) = crate::structural_shape(types.clone(), *referent) else {
            return false;
        };
        match (stored, target) {
            (Some(stored), StructuralTypeShape::PrimitiveScalar(logical)) => {
                let StructuralTypeShape::PrimitiveScalar(source) = source_shape else {
                    return false;
                };
                (byte_endpoint
                    || source == logical
                    || scalar_reference_view_is_total(*source, *logical))
                    && reference_storage_view_is_compatible(stored, *logical)
            }
            (None, terminal_psi::record_or_fixed_array_shape_pattern!()) => {
                plain_storage(types.clone(), *referent, &mut Vec::new())
                    && (byte_endpoint
                        || plain_storage(types.clone(), source, &mut Vec::new())
                            && reference_storage_layout(types.clone(), source).is_some_and(
                                |layout| {
                                    let target_layout =
                                        reference_storage_layout(types.clone(), *referent);
                                    Some(layout) == target_layout
                                        || storage_scalars_satisfy(
                                            types.clone(),
                                            *referent,
                                            &mut Vec::new(),
                                            &is_byte_scalar,
                                        ) && target_layout.is_some_and(|target| {
                                            layout.0 == target.0
                                                && target.1 != 0
                                                && layout.1 % target.1 == 0
                                        })
                                },
                            ))
            }
            _ => false,
        }
    }

    pub(crate) fn plain_storage<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> bool {
        storage_scalars_satisfy(types, root, active, &|scalar| {
            reference_storage_byte_width(scalar).is_some()
        })
    }

    fn is_byte_scalar(scalar: ScalarType) -> bool {
        matches!(scalar, ScalarType::Integer(integer) if !integer.is_address()
            && integer.bits() == 8 && integer.sign() == IntegerSign::Unsigned)
    }

    fn storage_scalars_satisfy<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        accepts: &impl Fn(ScalarType) -> bool,
    ) -> bool {
        if active.len() >= 128 || active.contains(&root) {
            return false;
        }
        let Some(shape) = crate::structural_shape(types.clone(), root) else {
            return false;
        };
        active.push(root);
        let valid = match shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => accepts(*scalar),
            StructuralTypeShape::FixedArray { element, .. } => {
                storage_scalars_satisfy(types.clone(), *element, active, accepts)
            }
            StructuralTypeShape::Record { fields, .. } => fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .all(|field| match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => {
                        storage_scalars_satisfy(types.clone(), child, active, accepts)
                    }
                    terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_) => false,
                    _ => field.field_type.scalar_type().is_some_and(accepts),
                }),
            _ => false,
        };
        active.pop();
        valid
    }
    use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape};

    use crate::scalars::scalar_representation_view::{integer_bits, integer_from_bits};
    use crate::{ScalarRepresentationViewPair, ScalarViewValue};

    pub fn scalar_reference_view_is_total(source: ScalarType, target: ScalarType) -> bool {
        if source == target {
            return true;
        }
        if let (ScalarType::Integer(source), ScalarType::Integer(target)) = (source, target) {
            return !source.is_address()
                && !target.is_address()
                && source.bits() == target.bits()
                && source.bits() <= 64;
        }
        ScalarRepresentationViewPair::classify(source, target)
            .is_some_and(|pair| !pair.requires_landing())
    }

    /// Whether a reference with `access` may present `stored` scalar storage as
    /// `presented` (wiki/spec/layouts/recasts.md#shared-and-mutable-views).
    ///
    /// A recast keeps the same backing, so the judgment follows the operations
    /// the view enables: a shared reader needs every stored value to read as a
    /// valid presented value, a write-only view needs every presented value to
    /// leave valid stored bits, and a mutable view needs both. Owned values are
    /// never reinterpreted in place. Domain handback obligations on the lender
    /// stay with the checker; this judges only representation validity.
    pub fn scalar_reference_view_admits(
        access: StructuralAccess,
        stored: ScalarType,
        presented: ScalarType,
    ) -> bool {
        let reads = || scalar_reference_view_is_total(stored, presented);
        let writes = || scalar_reference_view_is_total(presented, stored);
        match access {
            StructuralAccess::Owned => stored == presented,
            StructuralAccess::SharedBorrow => reads(),
            StructuralAccess::WriteOnlyBorrow => writes(),
            StructuralAccess::MutableBorrow => reads() && writes(),
        }
    }

    /// The leaf contract an inline field presents to a borrow with `access`
    /// that expects `expected`. Every Terminal consumer (native target lowering
    /// and the Psi interpreter) resolves borrowed field leaves here, so a
    /// cross-carrier scalar view of a field has one admission rule.
    pub fn borrowed_field_presentation(
        field: &terminal_psi::StructuralFieldType,
        access: StructuralAccess,
        expected: Option<&StructuralTypeDeclaration>,
    ) -> Option<terminal_psi::StructuralFieldType> {
        let presentation = field.borrowed_presentation_type(access, expected)?;
        if let Some(expected) = expected
            && presentation != terminal_psi::StructuralFieldType::Structural(expected.id)
            && expected.scalar_restriction.is_none()
            && let StructuralTypeShape::PrimitiveScalar(presented) = expected.shape
            && let Some(StructuralTypeShape::PrimitiveScalar(stored)) =
                presentation.canonical_leaf_shape()
            && scalar_reference_view_admits(access, stored, presented)
        {
            return Some(terminal_psi::StructuralFieldType::Structural(expected.id));
        }
        Some(presentation)
    }

    pub fn scalar_reference_view_value(
        source: ScalarType,
        target: ScalarType,
        value: ScalarViewValue,
    ) -> Option<ScalarViewValue> {
        if !scalar_reference_view_is_total(source, target) {
            return None;
        }
        if source == target {
            return match (source, value) {
                (ScalarType::Boolean, ScalarViewValue::Boolean(_)) => Some(value),
                (ScalarType::Integer(integer), ScalarViewValue::Integer(value))
                    if integer.admits(value) =>
                {
                    Some(ScalarViewValue::Integer(value))
                }
                (ScalarType::IeeeFloat(format), ScalarViewValue::IeeeFloat(value))
                    if value.format() == format =>
                {
                    Some(ScalarViewValue::IeeeFloat(value))
                }
                _ => None,
            };
        }
        if let (
            ScalarType::Integer(source),
            ScalarType::Integer(target),
            ScalarViewValue::Integer(value),
        ) = (source, target, value)
        {
            return Some(ScalarViewValue::Integer(integer_from_bits(
                target,
                integer_bits(source, value)?,
            )));
        }
        ScalarRepresentationViewPair::classify(source, target)?.view(value)
    }

    pub fn reference_source_compatible<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source: StructuralTypeId,
        reference: StructuralTypeId,
    ) -> bool {
        let Some(StructuralTypeShape::Reference { referent, access }) =
            crate::structural_shape(types.clone(), reference)
        else {
            return false;
        };
        let Some(source_shape) = crate::structural_shape(types.clone(), source) else {
            return false;
        };
        if source == *referent {
            return true;
        }
        if matches!(
            source_shape,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        ) && crate::structural_declaration(types.clone(), *referent)
            .is_some_and(|row| row.shape == *source_shape)
        {
            return true;
        }
        let Some(StructuralTypeShape::PrimitiveScalar(target)) =
            crate::structural_shape(types, *referent)
        else {
            return false;
        };
        matches!(source_shape, StructuralTypeShape::PrimitiveScalar(source)
            if scalar_reference_view_admits(*access, *source, *target))
    }
    #[cfg(test)]
    mod scalar_view_admission_tests {
        use super::*;
        use semantic_vocabulary::{IntegerSign, IntegerType, StructuralTypeId};
        use terminal_psi::{StructuralFieldType, TerminalIdentityIndex};

        fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
            ScalarType::Integer(IntegerType::new(sign, bits).expect("fixed integer"))
        }

        fn primitive(raw: u64, scalar: ScalarType) -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                requires: Vec::new(),
                id: StructuralTypeId::new(raw).expect("type identity"),
                identity: TerminalIdentityIndex(raw as u32),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                quotient_carrier: None,
                scalar_restriction: None,
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
            }
        }

        // `&mut pair.start as &mut u32` over an i32 field: every bit pattern is
        // valid in both carriers, so every borrow access admits the view, but an
        // owned value is never reinterpreted in place.
        #[test]
        fn equal_width_integer_views_admit_every_borrow() {
            let signed = integer(IntegerSign::Signed, 32);
            let unsigned = integer(IntegerSign::Unsigned, 32);
            for access in [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
                StructuralAccess::WriteOnlyBorrow,
            ] {
                assert!(scalar_reference_view_admits(access, signed, unsigned));
                assert!(scalar_reference_view_admits(access, unsigned, signed));
            }
            assert!(!scalar_reference_view_admits(
                StructuralAccess::Owned,
                signed,
                unsigned
            ));
        }

        // Every bool reads as a valid u8, but a u8 write of 2 would leave invalid
        // bool storage: only the direction the view's operations need is admitted.
        #[test]
        fn one_way_views_admit_only_the_operations_they_keep_valid() {
            let byte = integer(IntegerSign::Unsigned, 8);
            let boolean = ScalarType::Boolean;
            assert!(scalar_reference_view_admits(
                StructuralAccess::SharedBorrow,
                boolean,
                byte
            ));
            assert!(!scalar_reference_view_admits(
                StructuralAccess::MutableBorrow,
                boolean,
                byte
            ));
            assert!(!scalar_reference_view_admits(
                StructuralAccess::WriteOnlyBorrow,
                boolean,
                byte
            ));
            assert!(scalar_reference_view_admits(
                StructuralAccess::WriteOnlyBorrow,
                byte,
                boolean
            ));
            assert!(!scalar_reference_view_admits(
                StructuralAccess::SharedBorrow,
                byte,
                boolean
            ));
        }

        #[test]
        fn borrowed_fields_present_only_admitted_scalar_views() {
            let signed = StructuralFieldType::Scalar(integer(IntegerSign::Signed, 32));
            let unsigned = primitive(7, integer(IntegerSign::Unsigned, 32));
            assert_eq!(
                borrowed_field_presentation(
                    &signed,
                    StructuralAccess::MutableBorrow,
                    Some(&unsigned)
                ),
                Some(StructuralFieldType::Structural(unsigned.id))
            );
            let boolean = StructuralFieldType::Scalar(ScalarType::Boolean);
            let byte = primitive(8, integer(IntegerSign::Unsigned, 8));
            assert_eq!(
                borrowed_field_presentation(&boolean, StructuralAccess::SharedBorrow, Some(&byte)),
                Some(StructuralFieldType::Structural(byte.id))
            );
            assert_eq!(
                borrowed_field_presentation(&boolean, StructuralAccess::MutableBorrow, Some(&byte)),
                Some(boolean)
            );
        }
    }
}
pub(crate) mod resident_references {

    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{
        ResidentReferenceSource, StructuralAccess, StructuralTypeShape, TerminalAccessViewPlan,
        TerminalModule,
    };

    pub fn resident_reference_plan(
        module: &TerminalModule,
        source_type: StructuralTypeId,
        result_type: StructuralTypeId,
        access: StructuralAccess,
        resident: ResidentReferenceSource,
    ) -> Result<&TerminalAccessViewPlan, &'static str> {
        validate_resident_owner_geometry(module, source_type, resident.base, resident.length)?;
        let Some(StructuralTypeShape::Reference {
            referent,
            access: requested,
        }) = module.structural_type(result_type).map(|row| &row.shape)
        else {
            return Err("resident interpretation requires an ordinary reference result");
        };
        if !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || !matches!(
            requested,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || !requested.admits(access)
        {
            return Err("resident interpretation cannot increase source borrow access");
        }
        if resident.granted == resident.resident
            || [resident.granted, resident.resident]
                .iter()
                .any(|identity| {
                    module
                        .structural_domain(*identity)
                        .is_none_or(|domain| domain.carrier != source_type)
                })
        {
            return Err("resident interpretation lost distinct source-owner qualification roles");
        }
        let mut plans = module
            .access_view_plans
            .iter()
            .filter(|plan| plan.carrier == *referent);
        let (Some(plan), None) = (plans.next(), plans.next()) else {
            return Err("resident interpretation requires one exact retained placement");
        };
        Ok(plan)
    }

    pub(crate) fn validate_resident_owner_geometry(
        module: &TerminalModule,
        carrier: StructuralTypeId,
        base: semantic_vocabulary::StructuralFieldId,
        length: semantic_vocabulary::StructuralFieldId,
    ) -> Result<(), &'static str> {
        let declaration = module
            .structural_type(carrier)
            .ok_or("placement extent carrier is absent")?;
        validate_resident_owner_fields(declaration, base, length)
    }

    pub fn validate_resident_owner_fields(
        declaration: &terminal_psi::StructuralTypeDeclaration,
        base: semantic_vocabulary::StructuralFieldId,
        length: semantic_vocabulary::StructuralFieldId,
    ) -> Result<(), &'static str> {
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return Err("placement extent carrier is not a record");
        };
        if !declaration.owns_linear_claim || base == length {
            return Err("placement extent lost linear custody or distinct geometry fields");
        }
        let field_type = |identity| {
            fields
                .iter()
                .find(|field| field.id == identity && !field.relevance.is_erased())
                .and_then(|field| field.field_type.scalar_type())
                .and_then(|scalar| scalar.as_integer())
        };
        if field_type(base).is_none_or(|integer| !integer.is_address())
            || field_type(length).is_none_or(|integer| {
                integer.is_address()
                    || integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                    || integer.bits() != 64
            })
        {
            return Err("placement extent geometry roles have incorrect scalar types");
        }
        Ok(())
    }
}

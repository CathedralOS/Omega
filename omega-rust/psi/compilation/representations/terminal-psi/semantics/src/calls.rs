/*
Semantic judgments for direct, indirect, and boundary call sites, argument/result binding, crash
continuation routing, and call-graph reachability.
*/

pub(crate) mod call_composition {

    pub(crate) mod bounded_byte_owner {
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
            StructuralParameterDeclaration, StructuralTypeShape, TerminalModule,
        };

        pub fn bounded_byte_owner_capacity(
            module: &TerminalModule,
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<u64> {
            bounded_byte_owner_capacity_from_catalogs(
                &module.structural_types,
                &module.structural_domains,
                actual,
                argument,
                expected,
            )
        }

        pub fn bounded_byte_owner_capacity_from_catalogs(
            types: &[terminal_psi::StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<u64> {
            bounded_byte_owner_capacity_in(types.iter(), domains.iter(), actual, argument, expected)
        }

        pub fn bounded_byte_owner_capacity_in<'type_declaration, 'domain>(
            types: impl Iterator<Item = &'type_declaration terminal_psi::StructuralTypeDeclaration>
            + Clone,
            domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<u64> {
            if actual.place != argument.place
                || argument.access != expected.access
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    actual.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                || argument.access == StructuralAccess::Owned
                || !crate::reference_source_projection_accessible(
                    types.clone(),
                    actual.structural_type,
                    actual.multiplicity,
                    actual.access,
                    &argument.path,
                    argument.access,
                )
            {
                return None;
            }
            let expected_type =
                crate::structural_declaration(types.clone(), expected.structural_type)?;
            let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
                expected_type.shape
            else {
                return None;
            };
            let required = crate::byte_parameter_predicates_in(domains.clone(), expected)?;
            let (actual_capacity, predicates) = if argument.path.is_empty() {
                let declaration =
                    crate::structural_declaration(types.clone(), actual.structural_type)?;
                let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity,
                }) = declaration.shape
                else {
                    return None;
                };
                (
                    capacity,
                    crate::byte_parameter_predicates_in(domains.clone(), actual)?,
                )
            } else {
                if !actual.qualifications.is_empty() || !actual.projected_qualifications.is_empty()
                {
                    return None;
                }
                let field =
                    crate::runtime_structural_field(types, actual.structural_type, &argument.path)?;
                let ByteSequenceCarrier::BoundedOwned { capacity } =
                    field.field_type.byte_sequence_carrier()?
                else {
                    return None;
                };
                (capacity, field.field_type.byte_predicates())
            };
            if actual_capacity != capacity
                || !predicates.implies(required)
                || (argument.access != StructuralAccess::SharedBorrow
                    && !required.implies(predicates))
            {
                return None;
            }
            Some(capacity)
        }
    }
    pub(crate) mod fixed_byte_view {

        use crate::runtime_structural_path_tip;
        use semantic_vocabulary::{IntegerSign, ScalarType};
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
            StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
            StructuralTypeShape,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct FixedCollectionWindow<'path> {
            pub backing_path: &'path [StructuralPathSegment],
            pub backing_length: u64,
            pub offset: u64,
            pub length: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum BorrowedReferentPresentation {
            Unchanged,
            AddressOfBacking { byte_count: u64 },
        }

        pub fn borrowed_referent_matches<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            actual: semantic_vocabulary::StructuralTypeId,
            expected: semantic_vocabulary::StructuralTypeId,
        ) -> bool {
            borrowed_referent_presentation(types, actual, expected).is_some()
        }

        pub fn borrowed_referent_presentation<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            actual: semantic_vocabulary::StructuralTypeId,
            expected: semantic_vocabulary::StructuralTypeId,
        ) -> Option<BorrowedReferentPresentation> {
            let expected_type = types.clone().find(|row| row.id == expected)?;
            if actual == expected {
                return Some(BorrowedReferentPresentation::Unchanged);
            }
            if crate::references::reference_views::descriptor_geometry_matches(
                &types.clone().find(|row| row.id == actual)?.shape,
                &expected_type.shape,
            ) {
                return Some(BorrowedReferentPresentation::Unchanged);
            }
            let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity,
                ..
            }) = expected_type.shape
            else {
                return None;
            };
            if fixed_byte_extent(types.clone(), actual) != Some(capacity) {
                return None;
            }
            match types.clone().find(|row| row.id == actual)?.shape {
                StructuralTypeShape::FixedArray { .. } => {
                    Some(BorrowedReferentPresentation::AddressOfBacking {
                        byte_count: capacity,
                    })
                }
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => {
                    Some(BorrowedReferentPresentation::Unchanged)
                }
                _ => None,
            }
        }

        pub fn fixed_byte_extent<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            identity: semantic_vocabulary::StructuralTypeId,
        ) -> Option<u64> {
            match types.clone().find(|row| row.id == identity)?.shape {
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity,
                    ..
                }) => Some(capacity),
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    let element = types.clone().find(|row| row.id == element)?;
                    matches!(element.shape,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                            if integer.sign() == IntegerSign::Unsigned
                                && integer.bits() == 8 && !integer.is_address())
                    .then_some(length)
                }
                _ => None,
            }
        }

        pub fn fixed_byte_array_extent<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<u64> {
            fixed_byte_array_window(types, actual, argument, expected).map(|window| window.length)
        }

        pub fn fixed_byte_array_window<'types, 'path>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            actual: &StructuralParameterDeclaration,
            argument: &'path StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<FixedCollectionWindow<'path>> {
            if !readable_collection_loan(actual, argument, expected)
                || !types.clone().any(|declaration| {
                    declaration.id == expected.structural_type
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::byte_sequence_view_carrier_pattern!()
                            )
                        )
                })
            {
                return None;
            }
            let bounded_capacity = types.clone().find_map(|declaration| {
                match (
                    declaration.id == expected.structural_type,
                    &declaration.shape,
                ) {
                    (
                        true,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                            capacity,
                            ..
                        }),
                    ) => Some(*capacity),
                    _ => None,
                }
            });
            if bounded_capacity.is_some()
                && matches!(
                    argument.path.last(),
                    Some(StructuralPathSegment::FixedRange { .. })
                )
            {
                return None;
            }
            let window = fixed_byte_storage_window(types, actual.structural_type, &argument.path)?;
            if bounded_capacity.is_some_and(|capacity| capacity != window.length) {
                return None;
            }
            Some(window)
        }

        pub fn fixed_byte_storage_window<'types, 'path>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: semantic_vocabulary::StructuralTypeId,
            path: &'path [StructuralPathSegment],
        ) -> Option<FixedCollectionWindow<'path>> {
            let window = fixed_collection_storage_window(types.clone(), root, path)?;
            let backing = runtime_structural_path_tip(types.clone(), root, window.backing_path)?;
            (fixed_byte_extent(types, backing.id)? == window.backing_length).then_some(window)
        }

        pub fn fixed_collection_storage_window<'types, 'path>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: semantic_vocabulary::StructuralTypeId,
            path: &'path [StructuralPathSegment],
        ) -> Option<FixedCollectionWindow<'path>> {
            let (backing_path, range) = match path.split_last() {
                Some((StructuralPathSegment::FixedRange { start, end }, backing)) => {
                    (backing, Some((*start, *end)))
                }
                _ => (path, None),
            };
            let declaration = runtime_structural_path_tip(types.clone(), root, backing_path)?;
            let length = match declaration.shape {
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                } => length,
                _ => fixed_byte_extent(types, declaration.id)?,
            };
            let (start, end) = range.unwrap_or((0, length));
            if start > end || end > length {
                return None;
            }
            Some(FixedCollectionWindow {
                backing_path,
                backing_length: length,
                offset: start,
                length: end - start,
            })
        }

        pub fn fixed_element_array_extent<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<u64> {
            if !readable_collection_loan(actual, argument, expected)
                || [actual, expected].iter().any(|parameter| {
                    !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                })
                || !argument
                    .path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
            {
                return None;
            }
            let expected_element = types.clone().find_map(|declaration| {
                match (
                    declaration.id == expected.structural_type,
                    &declaration.shape,
                ) {
                    (true, StructuralTypeShape::ElementView { element, .. }) => Some(*element),
                    _ => None,
                }
            })?;
            let declaration =
                runtime_structural_path_tip(types.clone(), actual.structural_type, &argument.path)?;
            let StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } = declaration.shape
            else {
                return None;
            };
            if element != expected_element {
                return None;
            }
            Some(length)
        }

        fn readable_collection_loan(
            actual: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> bool {
            actual.place == argument.place
                && argument.access == expected.access
                && matches!(
                    (actual.access, argument.access),
                    (
                        terminal_psi::mutating_access_pair_pattern!(),
                        terminal_psi::reading_borrow_pair_pattern!()
                    ) | (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    )
                )
                && matches!(
                    actual.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                && expected.multiplicity == StructuralMultiplicity::Unrestricted
        }

        pub fn fixed_byte_array_loan<'types, 'domain, 'path>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
            actual: &StructuralParameterDeclaration,
            argument: &'path StructuralArgument,
            expected: &StructuralParameterDeclaration,
        ) -> Option<FixedCollectionWindow<'path>> {
            let window = fixed_byte_array_window(types.clone(), actual, argument, expected)?;
            if !window.backing_path.is_empty() && !actual.qualifications.is_empty() {
                return None;
            }
            let carrier =
                runtime_structural_path_tip(types, actual.structural_type, window.backing_path)?.id;
            let (whole, projected) = terminal_psi::StructuralPathQualification::select(
                &actual.qualifications,
                &actual.projected_qualifications,
                window.backing_path,
            );
            if !projected.is_empty() {
                return None;
            }
            let original =
                crate::byte_qualification_predicates_in(domains.clone(), carrier, &whole)?;
            let required = crate::byte_parameter_predicates_in(domains, expected)?;
            let mut offered = original;
            if window.offset != 0 || window.length != window.backing_length {
                offered = terminal_psi::ByteSequencePredicateSet::EMPTY;
                for predicate in original.predicates() {
                    if matches!(
                        predicate,
                        terminal_psi::ByteSequencePredicate::AsciiOnly
                            | terminal_psi::ByteSequencePredicate::NoNul
                    ) {
                        offered.insert(predicate);
                    }
                }
            }
            (offered.implies(required)
                && (argument.access == StructuralAccess::SharedBorrow
                    || required.implies(original)))
            .then_some(window)
        }
    }
    pub use bounded_byte_owner::{
        bounded_byte_owner_capacity, bounded_byte_owner_capacity_from_catalogs,
        bounded_byte_owner_capacity_in,
    };
    pub use fixed_byte_view::{
        BorrowedReferentPresentation, FixedCollectionWindow, borrowed_referent_matches,
        borrowed_referent_presentation, fixed_byte_array_extent, fixed_byte_array_loan,
        fixed_byte_array_window, fixed_byte_extent, fixed_byte_storage_window,
        fixed_collection_storage_window, fixed_element_array_extent,
    };

    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{
        ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralArgument,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };

    use crate::OperationSemanticError;
    use crate::runtime_structural_field;
    use crate::semantic_rows::OperationSemanticTag;

    pub fn boundary_buffer_capacity<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
        expected: &StructuralParameterDeclaration,
    ) -> Option<u64> {
        inline_byte_field_capacity(
            types,
            root_type,
            argument,
            expected,
            StructuralAccess::MutableBorrow,
        )
    }

    pub fn shared_boundary_buffer_capacity<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
        expected: &StructuralParameterDeclaration,
    ) -> Option<u64> {
        inline_byte_field_capacity(
            types,
            root_type,
            argument,
            expected,
            StructuralAccess::SharedBorrow,
        )
    }

    pub fn stored_view_field_presentation<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
        expected: &StructuralParameterDeclaration,
    ) -> Option<terminal_psi::ByteSequencePredicateSet> {
        if argument.access != StructuralAccess::SharedBorrow
            || expected.access != StructuralAccess::SharedBorrow
            || !expected.projected_qualifications.is_empty()
            || !types.clone().any(|declaration| {
                declaration.id == expected.structural_type
                    && declaration.shape.is_shared_readable_byte_view()
            })
        {
            return None;
        }
        let field = runtime_structural_field(types, root_type, &argument.path)?;
        (field.field_type.byte_sequence_carrier()?
            == ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            })
        .then(|| field.field_type.byte_predicates())
    }

    fn inline_byte_field_capacity<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
        expected: &StructuralParameterDeclaration,
        access: StructuralAccess,
    ) -> Option<u64> {
        if argument.access != access
            || expected.access != access
            || !expected.qualifications.is_empty()
            || !expected.projected_qualifications.is_empty()
            || !types.clone().any(|declaration| {
                declaration.id == expected.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                    )
            })
        {
            return None;
        }
        if access == StructuralAccess::SharedBorrow && argument.path.is_empty() {
            return types
                .clone()
                .find_map(|declaration| match declaration.shape {
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity,
                    }) if declaration.id == root_type => Some(capacity),
                    _ => None,
                });
        }
        let field = runtime_structural_field(types, root_type, &argument.path)?;
        if access != StructuralAccess::SharedBorrow
            && !field.field_type.byte_predicates().is_empty()
        {
            return None;
        }
        match field.field_type.byte_sequence_carrier()? {
            ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
            _ => None,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallTargetRule {
        ExactModuleMachine,
        ExactBoundaryMachine,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallResultRule {
        ScalarCalleeResult,
        UnitCalleeResult,
        StructuralCalleeResult,
        BoundaryDeclaredResult,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallArgumentRule {
        ScalarPositionalArguments,
        StructuralPositionalArguments,
        ScalarAndStructuralPositionalArguments,
        BoundaryScalarAndStructuralPositionalArguments,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallRequirementRule {
        EnumerateScalarRequires,
        EnumerateStructuralRequires,
        ValidateBoundaryRequirements,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallTransferRule {
        NoStructuralTransfer,
        ExactClaimTransfers,
        ExactClaimAndStructuralResultTransfers,
        ExactCompletionReceipts,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallOutcomeRule {
        ImportScalarEnsures,
        ImportStructuralEnsures,
        BoundaryDeclaredCompletion,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallCrashRule {
        ComposeScalarCrashRoutes,
        ComposeStructuralCrashRoutes,
        BoundaryHasNoCallRoutes,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallEvidenceRule {
        SameUnitOrCertifiedScalarEvidence,
        SameUnitOrCertifiedStructuralEvidence,
        ExactBoundaryDeclarationEvidence,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallWorkPolicy {
        ConsumeOne,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CallFrontierRule {
        KeepsScalarFrontier,
        TransfersStructuralFrontier,
        SettlesBoundaryFrontier,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CallCompositionSchema {
        target: CallTargetRule,
        result: CallResultRule,
        arguments: CallArgumentRule,
        requirements: CallRequirementRule,
        transfers: CallTransferRule,
        outcomes: CallOutcomeRule,
        crash: CallCrashRule,
        evidence: CallEvidenceRule,
        work: CallWorkPolicy,
        frontier: CallFrontierRule,
    }

    impl CallCompositionSchema {
        pub const fn target(self) -> CallTargetRule {
            self.target
        }

        pub const fn result(self) -> CallResultRule {
            self.result
        }

        pub const fn arguments(self) -> CallArgumentRule {
            self.arguments
        }

        pub const fn requirements(self) -> CallRequirementRule {
            self.requirements
        }

        pub const fn transfers(self) -> CallTransferRule {
            self.transfers
        }

        pub const fn outcomes(self) -> CallOutcomeRule {
            self.outcomes
        }

        pub const fn crash(self) -> CallCrashRule {
            self.crash
        }

        pub const fn evidence(self) -> CallEvidenceRule {
            self.evidence
        }

        pub const fn work(self) -> CallWorkPolicy {
            self.work
        }

        pub const fn frontier(self) -> CallFrontierRule {
            self.frontier
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CallCompositionSemanticRow {
        tag: OperationSemanticTag,
        schema: CallCompositionSchema,
    }

    impl CallCompositionSemanticRow {
        pub const fn tag(self) -> OperationSemanticTag {
            self.tag
        }

        pub const fn schema(self) -> CallCompositionSchema {
            self.schema
        }
    }

    macro_rules! call_composition_rows {
        ($( $tag:ident => ($target:ident, $result:ident, $arguments:ident,
            $requirements:ident, $transfers:ident, $outcomes:ident,
            $crash:ident, $evidence:ident, $frontier:ident) ),+ $(,)?) => {
            impl CallCompositionSemanticRow {
                pub const ALL: [Self; call_composition_rows!(@count $( $tag )+)] = [
                    $( Self {
                        tag: OperationSemanticTag::$tag,
                        schema: CallCompositionSchema {
                            target: CallTargetRule::$target,
                            result: CallResultRule::$result,
                            arguments: CallArgumentRule::$arguments,
                            requirements: CallRequirementRule::$requirements,
                            transfers: CallTransferRule::$transfers,
                            outcomes: CallOutcomeRule::$outcomes,
                            crash: CallCrashRule::$crash,
                            evidence: CallEvidenceRule::$evidence,
                            work: CallWorkPolicy::ConsumeOne,
                            frontier: CallFrontierRule::$frontier,
                        },
                    } ),+
                ];
            }

            crate::semantic_rows::operation_semantic_cohort! {
                rows: CallCompositionSemanticRow,
                tags: CALL_COMPOSITION_TAGS,
                is_tag: is_call_composition_tag,
                exact_row: exact_call_composition_semantic_row_in,
                reject_unexpected: reject_unexpected_call_composition_rows,
                missing: MissingCallCompositionRow,
                duplicate: DuplicateCallCompositionRow,
                unexpected: UnexpectedCallCompositionRow,
                $( $tag ),+
            }
        };
        (@count $( $variant:ident )+) => {
            <[()]>::len(&[$(call_composition_rows!(@unit $variant)),+])
        };
        (@unit $variant:ident) => { () };
    }

    call_composition_rows! {
        Call => (ExactModuleMachine, ScalarCalleeResult, ScalarPositionalArguments, EnumerateScalarRequires, NoStructuralTransfer, ImportScalarEnsures, ComposeScalarCrashRoutes, SameUnitOrCertifiedScalarEvidence, KeepsScalarFrontier),
        CallUnit => (ExactModuleMachine, UnitCalleeResult, ScalarAndStructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
        CallStructuralScalar => (ExactModuleMachine, ScalarCalleeResult, StructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
        CallStructural => (ExactModuleMachine, StructuralCalleeResult, StructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimAndStructuralResultTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
        CallStructuralWithScalarArguments => (ExactModuleMachine, StructuralCalleeResult, ScalarAndStructuralPositionalArguments, EnumerateStructuralRequires, ExactClaimAndStructuralResultTransfers, ImportStructuralEnsures, ComposeStructuralCrashRoutes, SameUnitOrCertifiedStructuralEvidence, TransfersStructuralFrontier),
        BoundaryCall => (ExactBoundaryMachine, BoundaryDeclaredResult, BoundaryScalarAndStructuralPositionalArguments, ValidateBoundaryRequirements, ExactCompletionReceipts, BoundaryDeclaredCompletion, BoundaryHasNoCallRoutes, ExactBoundaryDeclarationEvidence, SettlesBoundaryFrontier),
    }

    fn canonical_schema(tag: OperationSemanticTag) -> Option<CallCompositionSchema> {
        CallCompositionSemanticRow::ALL
            .iter()
            .find(|row| row.tag == tag)
            .map(|row| row.schema)
    }

    pub fn validate_call_composition_semantic_rows(
        rows: &[CallCompositionSemanticRow],
    ) -> Result<(), OperationSemanticError> {
        reject_unexpected_call_composition_rows(rows)?;
        for tag in CALL_COMPOSITION_TAGS {
            let row = exact_call_composition_semantic_row_in(tag, rows)?
                .expect("the requested tag belongs to call composition");
            if Some(row.schema) != canonical_schema(tag) {
                return Err(OperationSemanticError::CallCompositionSchemaMismatch(tag));
            }
        }
        Ok(())
    }

    pub fn call_composition_semantic_row(
        operation: &OperationKind,
    ) -> Result<Option<&'static CallCompositionSemanticRow>, OperationSemanticError> {
        validate_call_composition_semantic_rows(&CallCompositionSemanticRow::ALL)?;
        exact_call_composition_semantic_row_in(
            OperationSemanticTag::for_operation(operation),
            &CallCompositionSemanticRow::ALL,
        )
    }
}
pub mod closed_conformances {

    use terminal_psi::{
        ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedReachArgument,
        ClosedReachParameter, TargetMachineCallable, TerminalIdentityIndex, TerminalModule,
    };

    pub fn is_checked_body_family_contract(
        module: &TerminalModule,
        boundary: &terminal_psi::BoundaryMachineDeclaration,
    ) -> bool {
        let head = TargetMachineCallable::Boundary(boundary.id);
        let Some(family) = module
            .target_machine_families
            .iter()
            .find(|family| family.canonical == head)
        else {
            return false;
        };
        boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Machine
            && boundary.role == terminal_psi::BoundaryDeclarationRole::Callable
            && family.head_contract != [0; 32]
            && boundary
                .declaration
                .as_ref()
                .is_some_and(|declaration| declaration.source.contract == family.head_contract)
            && validate_boundary_callable(module, boundary).is_ok()
            && !family.members.is_empty()
            && family.members.iter().all(|member| match member.callable {
                TargetMachineCallable::Machine(machine) => {
                    matches!(
                        member.supply,
                        terminal_psi::TargetMachineSupply::CheckedBody
                            | terminal_psi::TargetMachineSupply::Boundary
                    ) && module.machine(machine).is_some()
                }
                TargetMachineCallable::Boundary(_) => false,
            })
    }

    pub fn validate_boundary_callable(
        module: &TerminalModule,
        boundary: &terminal_psi::BoundaryMachineDeclaration,
    ) -> Result<(), &'static str> {
        let Some(checked) = &boundary.declaration else {
            return Ok(());
        };
        if boundary.role != terminal_psi::BoundaryDeclarationRole::Callable {
            return Err("contract evidence cannot carry an executable boundary declaration");
        }
        let declaration = &checked.source;
        for coordinate in [
            declaration.machine.identity(),
            declaration.state.identity(),
            declaration.semantic_declaration,
            declaration.callable,
            declaration.result_type,
        ]
        .into_iter()
        .chain(declaration.parameter_types.iter().copied())
        {
            identity(module, coordinate)?;
        }
        if declaration.contract == [0; 32]
            || !declaration.machine.same_identity_as(
                &module.identities,
                &boundary.owner,
                &module.identities,
            )
        {
            return Err("checked boundary declaration differs from its exact declaration owner");
        }
        match &declaration.specialization {
            Some(specialization) => {
                identity(module, specialization.template.identity())?;
                if let Some(capture) = &specialization.capture {
                    identity(module, capture.template_identity)?;
                    if capture.template_commitment == [0; 32]
                        || capture.specialization_commitment == [0; 32]
                    {
                        return Err("checked boundary capture has an empty commitment");
                    }
                }
                if specialization.commitment == [0; 32] {
                    return Err("checked boundary specialization has an empty commitment");
                }
            }
            None if !checked.method_arguments.is_empty() => {
                return Err("unspecialized boundary carries method arguments");
            }
            None => {}
        }
        for argument in &checked.method_arguments {
            let coordinate = match argument {
                ClosedReachArgument::Type(coordinate)
                | ClosedReachArgument::Const(coordinate)
                | ClosedReachArgument::Proposition(coordinate) => *coordinate,
                ClosedReachArgument::Machine {
                    identity,
                    contract_commitment,
                } => {
                    if *contract_commitment == [0; 32] {
                        return Err("boundary method argument has an empty contract commitment");
                    }
                    *identity
                }
            };
            identity(module, coordinate)?;
        }
        if let Some(family) = module
            .target_machine_families
            .iter()
            .find(|family| family.canonical == TargetMachineCallable::Boundary(boundary.id))
        {
            if family.head_contract != declaration.contract {
                return Err(
                    "checked boundary declaration differs from its published family contract",
                );
            }
            if !family
                .members
                .iter()
                .any(|member| member.callable == family.canonical)
                && identity(module, declaration.callable)? != identity(module, boundary.identity)?
            {
                return Err(
                    "checked boundary declaration differs from its published family overload",
                );
            }
        }
        Ok(())
    }

    fn callable_declaration(
        module: &TerminalModule,
        callable: TargetMachineCallable,
    ) -> Result<&terminal_psi::CheckedMachineDeclaration, &'static str> {
        match callable {
            TargetMachineCallable::Machine(machine) => module
                .machine(machine)
                .and_then(|machine| machine.declaration.as_ref()),
            TargetMachineCallable::Boundary(boundary) => module
                .boundary_machine(boundary)
                .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
                .and_then(|boundary| boundary.declaration.as_ref())
                .map(|declaration| &declaration.source),
        }
        .ok_or("static conformance target has no checked declaration")
    }

    fn callable_result(
        module: &TerminalModule,
        callable: TargetMachineCallable,
    ) -> Result<terminal_psi::ClosedConformanceCallableResult, &'static str> {
        use terminal_psi::{
            BoundaryMachineResult, ClosedConformanceCallableResult as ResultClass,
            TerminalMachineResult,
        };
        match callable {
            TargetMachineCallable::Machine(machine) => match &module
                .machine(machine)
                .ok_or("conformance callable registry names an absent machine")?
                .result
            {
                TerminalMachineResult::Unit => Ok(ResultClass::Unit),
                TerminalMachineResult::Scalar(result) => {
                    Ok(ResultClass::Scalar(result.scalar_type))
                }
                TerminalMachineResult::Structural(result) => {
                    Ok(ResultClass::Structural(result.structural_type))
                }
            },
            TargetMachineCallable::Boundary(boundary) => match &module
                .boundary_machine(boundary)
                .ok_or("conformance callable registry names an absent boundary")?
                .result
            {
                BoundaryMachineResult::Unit => Ok(ResultClass::Unit),
                BoundaryMachineResult::Scalar(result) => {
                    Ok(ResultClass::Scalar(result.scalar_type))
                }
                BoundaryMachineResult::Structural(result) => {
                    Ok(ResultClass::Structural(result.structural_type))
                }
            },
        }
    }

    fn declared_callable_identity(
        module: &TerminalModule,
        declaration: &terminal_psi::CheckedMachineDeclaration,
    ) -> Result<String, &'static str> {
        let callable = identity(module, declaration.callable)?;
        Ok(match &declaration.specialization {
            None => callable.to_owned(),
            Some(specialization) => {
                let commitment = specialization
                    .commitment
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!(
                    "specialized-machine|callable={}:{}|application={commitment}",
                    callable.len(),
                    callable
                )
            }
        })
    }

    fn identity(
        module: &TerminalModule,
        identity: TerminalIdentityIndex,
    ) -> Result<&str, &'static str> {
        module
            .identities
            .text(identity)
            .filter(|text| !text.is_empty())
            .ok_or("closed conformance contains an invalid or empty identity")
    }

    fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], &'static str> {
        let value = bytes
            .get(..count)
            .ok_or("conformance schema definition is truncated")?;
        *bytes = &bytes[count..];
        Ok(value)
    }

    fn length(bytes: &mut &[u8]) -> Result<usize, &'static str> {
        let raw: [u8; 8] = take(bytes, 8)?
            .try_into()
            .map_err(|_| "invalid schema length")?;
        u64::from_le_bytes(raw)
            .try_into()
            .map_err(|_| "schema length exceeds host positions")
    }

    fn validate_definition(schema: &ClosedConformanceSchema) -> Result<(), &'static str> {
        let mut bytes = schema.definition.as_slice();
        let domain = b"omega.requirement-schema.v3\0";
        if take(&mut bytes, domain.len())? != domain {
            return Err("conformance schema has an unknown definition domain");
        }
        let signature = length(&mut bytes)?;
        if signature == 0 {
            return Err("conformance schema has no original signature");
        }
        take(&mut bytes, signature)?;
        if length(&mut bytes)? != schema.lifetime_parameter_count as usize
            || length(&mut bytes)? != schema.parameter_kinds.len()
        {
            return Err("conformance schema telescope differs from its original definition");
        }
        for expected in &schema.parameter_kinds {
            let kind = take(&mut bytes, 1)?[0];
            if !(1..=6).contains(&kind) || kind != *expected {
                return Err("conformance schema binder category differs from its definition");
            }
            if !(1..=3).contains(&take(&mut bytes, 1)?[0]) {
                return Err("conformance schema has an invalid multiplicity");
            }
            let carry = bytes
                .iter()
                .position(|byte| *byte == 0)
                .ok_or("schema carry demand is unterminated")?;
            std::str::from_utf8(take(&mut bytes, carry)?)
                .map_err(|_| "schema carry demand is not canonical text")?;
            take(&mut bytes, 1)?;
            let contract = length(&mut bytes)?;
            take(&mut bytes, contract)?;
        }
        if !bytes.is_empty() {
            return Err("conformance schema definition has trailing bytes");
        }
        let generic = schema.lifetime_parameter_count != 0 || !schema.parameter_kinds.is_empty();
        match (schema.kind, generic, &schema.finite_tuples) {
            (ClosedConformanceSchemaKind::Nongeneric, false, None)
            | (ClosedConformanceSchemaKind::Open, true, None) => Ok(()),
            (ClosedConformanceSchemaKind::Finite, true, Some(tuples))
                if schema.lifetime_parameter_count == 0
                    && schema
                        .parameter_kinds
                        .iter()
                        .all(|kind| matches!(kind, 2 | 3))
                    && tuples
                        .iter()
                        .all(|tuple| tuple.len() == schema.parameter_kinds.len()) =>
            {
                Ok(())
            }
            _ => Err(
                "conformance schema classification contradicts its retained telescope or roster",
            ),
        }
    }

    pub(crate) fn argument_matches(
        module: &TerminalModule,
        expected: &ClosedReachArgument,
        actual: &ClosedReachParameter,
    ) -> bool {
        match (expected, actual) {
            (ClosedReachArgument::Type(identity), ClosedReachParameter::Type { argument })
            | (ClosedReachArgument::Const(identity), ClosedReachParameter::Const { argument })
            | (
                ClosedReachArgument::Proposition(identity),
                ClosedReachParameter::Proposition { argument },
            ) => module.identities.text(*identity) == Some(argument.as_str()),
            (
                ClosedReachArgument::Machine {
                    identity,
                    contract_commitment,
                },
                ClosedReachParameter::Machine(binding),
            ) => {
                *identity == binding.selected_identity
                    && *contract_commitment == binding.selected_contract_commitment
            }
            _ => false,
        }
    }

    fn application_key(
        application: &terminal_psi::ClosedConformanceApplication,
    ) -> (
        semantic_vocabulary::MachineId,
        terminal_psi::ClosedConformanceApplicationCommitment,
        u64,
    ) {
        (
            application.owner,
            application.commitment,
            application.report_fingerprint,
        )
    }

    fn dispatch_application<'module>(
        applications: &[&'module terminal_psi::ClosedConformanceApplication],
        caller: semantic_vocabulary::MachineId,
        dispatch: &terminal_psi::StaticRequirementDispatch,
        callee: semantic_vocabulary::MachineId,
    ) -> Result<&'module terminal_psi::ClosedConformanceApplication, &'static str> {
        if callee != dispatch.realization {
            return Err("static requirement dispatch differs from its ordinary runtime call");
        }
        let key = (
            caller,
            dispatch.conformance_application_commitment,
            dispatch.conformance_application_report_fingerprint,
        );
        let start = applications.partition_point(|application| application_key(application) < key);
        let end = applications.partition_point(|application| application_key(application) <= key);
        let [application] = &applications[start..end] else {
            return Err("static requirement dispatch has no unique closed conformance application");
        };
        if application
            .schemas
            .iter()
            .filter(|schema| {
                schema.declaring_trait_identity == dispatch.declaring_trait_identity
                    && schema.public_requirement_identity == dispatch.public_requirement_identity
                    && schema.requirement_identity == dispatch.requirement_identity
                    && schema.realization_identity == dispatch.realization_identity
            })
            .count()
            != 1
        {
            return Err("static requirement dispatch has no exact original schema");
        }
        if application
            .static_applications
            .iter()
            .filter(|applied| {
                applied.public_requirement_identity == dispatch.public_requirement_identity
                    && applied.realization_identity == dispatch.realization_identity
                    && applied.source_callable_identity == dispatch.realization_callable_identity
                    && applied.callable == TargetMachineCallable::Machine(dispatch.realization)
            })
            .count()
            != 1
        {
            return Err("static requirement dispatch has no exact executable method application");
        }
        Ok(*application)
    }

    pub fn validate_closed_conformances(module: &TerminalModule) -> Result<(), &'static str> {
        for boundary in &module.boundary_machines {
            validate_boundary_callable(module, boundary)?;
        }
        for application in &module.closed_conformance_applications {
            identity(module, application.declaration_identity)?;
            identity(module, application.trait_identity)?;
            if module.machine(application.owner).is_none() {
                return Err("closed conformance owner is absent");
            }
            let mut previous = None;
            for schema in &application.schemas {
                let key = (
                    identity(module, schema.declaring_trait_identity)?,
                    identity(module, schema.public_requirement_identity)?,
                );
                identity(module, schema.requirement_identity)?;
                identity(module, schema.requirement_declaration.identity())?;
                identity(module, schema.realization_identity)?;
                if previous.is_some_and(|previous| previous >= key) {
                    return Err("conformance schemas are repeated or not canonical");
                }
                previous = Some(key);
                validate_definition(schema)?;
                let rows = application
                    .rows
                    .iter()
                    .filter(|row| {
                        row.declaring_trait_identity == schema.declaring_trait_identity
                            && row.public_requirement_identity == schema.public_requirement_identity
                            && row.requirement_identity == schema.requirement_identity
                            && row.requirement_declaration == schema.requirement_declaration
                    })
                    .collect::<Vec<_>>();
                match schema.kind {
                    ClosedConformanceSchemaKind::Open if !rows.is_empty() => {
                        return Err("open conformance schema advertises finite execution coverage");
                    }
                    ClosedConformanceSchemaKind::Nongeneric
                        if rows.len() != 1
                            || !rows[0].family_tuple.is_empty()
                            || rows[0].realization_identity != schema.realization_identity =>
                    {
                        return Err("nongeneric conformance schema lost its original row");
                    }
                    ClosedConformanceSchemaKind::Finite => {
                        let tuples = schema
                            .finite_tuples
                            .as_ref()
                            .ok_or("finite schema lost its declared roster")?;
                        for tuple in tuples {
                            for argument in tuple {
                                identity(module, *argument)?;
                            }
                        }
                        let tuple_keys = tuples
                            .iter()
                            .map(|tuple| {
                                tuple
                                    .iter()
                                    .map(|argument| identity(module, *argument))
                                    .collect::<Result<Vec<_>, _>>()
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        if tuple_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
                            return Err(
                                "finite conformance schema tuples are repeated or not canonical",
                            );
                        }
                        if tuples
                            .iter()
                            .enumerate()
                            .any(|(index, tuple)| tuples[..index].contains(tuple))
                            || rows.len() != tuples.len()
                            || tuples.iter().any(|tuple| {
                                rows.iter().filter(|row| &row.family_tuple == tuple).count() != 1
                            })
                        {
                            return Err(
                                "finite conformance schema has partial or duplicate tuple coverage",
                            );
                        }
                    }
                    _ => {}
                }
            }
            for row in &application.rows {
                identity(module, row.realization_identity)?;
                if application
                    .schemas
                    .iter()
                    .filter(|schema| {
                        schema.declaring_trait_identity == row.declaring_trait_identity
                            && schema.public_requirement_identity == row.public_requirement_identity
                            && schema.requirement_identity == row.requirement_identity
                            && schema.requirement_declaration == row.requirement_declaration
                    })
                    .count()
                    != 1
                {
                    return Err("conformance row has no unique original schema");
                }
            }
            let mut registry_identities = Vec::new();
            let mut registry_machines = Vec::new();
            for callable in &application.realization_callables {
                let callable_identity = identity(module, callable.source_callable_identity)?;
                if registry_identities
                    .last()
                    .is_some_and(|previous| *previous >= callable_identity)
                    || registry_machines.contains(&callable.callable)
                {
                    return Err(
                        "conformance callable registry has duplicate identities or machine bindings, or is not canonical",
                    );
                }
                registry_identities.push(callable_identity);
                registry_machines.push(callable.callable);
                if callable.result != callable_result(module, callable.callable)? {
                    return Err(
                        "conformance callable registry result differs from its emitted declaration",
                    );
                }
                let declaration = callable_declaration(module, callable.callable)?;
                if declared_callable_identity(module, declaration)? != callable_identity {
                    return Err(
                        "conformance callable registry differs from its checked declaration identity",
                    );
                }
            }
            let application_keys = application
                .static_applications
                .iter()
                .map(|row| static_application_order_key(&module.identities, row))
                .collect::<Result<Vec<_>, _>>()?;
            if application_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("conformance static applications are repeated or not canonical");
            }
            for applied in &application.static_applications {
                identity(module, applied.public_requirement_identity)?;
                identity(module, applied.realization_identity)?;
                identity(module, applied.source_callable_identity)?;
                let schemas = application
                    .schemas
                    .iter()
                    .filter(|schema| {
                        schema.public_requirement_identity == applied.public_requirement_identity
                            && schema.realization_identity == applied.realization_identity
                    })
                    .collect::<Vec<_>>();
                let [schema] = schemas.as_slice() else {
                    return Err("static conformance application has no unique original schema");
                };
                if applied.method_arguments.len() != schema.parameter_kinds.len() {
                    return Err(
                        "static conformance method tuple differs from its independent telescope",
                    );
                }
                for (kind, argument) in schema.parameter_kinds.iter().zip(&applied.method_arguments)
                {
                    let (actual, argument_identity) = match argument {
                        ClosedReachArgument::Type(identity) => (1, *identity),
                        ClosedReachArgument::Const(identity) => (2, *identity),
                        ClosedReachArgument::Proposition(identity) => (6, *identity),
                        ClosedReachArgument::Machine { identity, .. } => (4, *identity),
                    };
                    identity(module, argument_identity)?;
                    if actual != *kind && !(actual == 2 && *kind == 3) {
                        return Err(
                            "static conformance method argument has the wrong binder category",
                        );
                    }
                }
                if let Some(tuples) = &schema.finite_tuples {
                    let values = applied
                        .method_arguments
                        .iter()
                        .map(|argument| match argument {
                            ClosedReachArgument::Const(identity) => Some(*identity),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(
                            "finite conformance method application contains a non-value argument",
                        )?;
                    if !tuples.contains(&values) {
                        return Err(
                            "static conformance method tuple is outside its original finite roster",
                        );
                    }
                }
                let declaration = callable_declaration(module, applied.callable)?;
                if declaration
                    .specialization
                    .as_ref()
                    .map(|specialization| specialization.commitment)
                    != applied.application_commitment
                {
                    return Err(
                        "static conformance application differs from its checked declaration commitment",
                    );
                }
                if application
                    .realization_callables
                    .iter()
                    .filter(|callable| {
                        callable.source_callable_identity == applied.source_callable_identity
                            && callable.callable == applied.callable
                    })
                    .count()
                    != 1
                {
                    return Err(
                        "static conformance application has no unique callable registry entry",
                    );
                }
                match applied.callable {
                    TargetMachineCallable::Boundary(boundary) => {
                        let arguments = module
                            .boundary_machine(boundary)
                            .and_then(|boundary| boundary.declaration.as_ref())
                            .ok_or("static conformance boundary has no independent invocation")?;
                        if arguments.method_arguments != applied.method_arguments {
                            return Err(
                                "static conformance method tuple differs from its retained boundary invocation",
                            );
                        }
                    }
                    TargetMachineCallable::Machine(machine)
                        if !applied.method_arguments.is_empty() =>
                    {
                        let machine = module
                            .machine(machine)
                            .ok_or("static conformance application target is absent")?;
                        let invocation = machine.closed_reach_application.as_ref().ok_or(
                            "static conformance method tuple has no independent retained invocation",
                        )?;
                        if Some(invocation.specialization_commitment)
                            != applied.application_commitment
                            || invocation.telescope.len() != applied.method_arguments.len()
                            || !applied
                                .method_arguments
                                .iter()
                                .zip(&invocation.telescope)
                                .all(|(expected, actual)| {
                                    argument_matches(module, expected, actual)
                                })
                        {
                            return Err(
                                "static conformance method tuple differs from its retained executable invocation",
                            );
                        }
                    }
                    TargetMachineCallable::Machine(_) => {}
                }
            }
            if terminal_psi::closed_conformance_application_commitment(
                &module.identities,
                application,
            ) != application.commitment
                || terminal_psi::closed_conformance_application_report_fingerprint(
                    &module.identities,
                    application,
                ) != application.report_fingerprint
            {
                return Err(
                    "closed conformance application commitment differs from its semantic contents",
                );
            }
        }
        // Every selected call authenticates the same retained application key.
        // Sort borrowed rows once so ordinary calls do not each scan and
        // allocate a copy of the complete module roster. Equal keys remain in
        // the slice so a demanded ambiguous application still refuses.
        let mut applications = module
            .closed_conformance_applications
            .iter()
            .collect::<Vec<_>>();
        applications.sort_unstable_by_key(|application| application_key(application));
        let mut proof_calls = module
            .proof_output_calls
            .iter()
            .filter_map(|call| {
                call.runtime_call
                    .map(|runtime| ((call.caller, runtime.site), call, runtime.callee))
            })
            .collect::<Vec<_>>();
        proof_calls.sort_unstable_by_key(|(site, _, _)| *site);
        for invocation in &module.proof_output_calls {
            let Some(dispatch) = &invocation.static_requirement_dispatch else {
                continue;
            };
            let callee = invocation
                .runtime_call
                .as_ref()
                .ok_or("static requirement dispatch has no ordinary runtime call")?
                .callee;
            dispatch_application(&applications, invocation.caller, dispatch, callee)?;
        }
        for machine in &module.machines {
            for invocation in machine.module_invocations() {
                let Some(required) = invocation.inputs.required_contract else {
                    continue;
                };
                let contract = module
                    .invocation_machine_contract(invocation.inputs.callee, Some(required))
                    .ok_or("selected call has no exact public contract and body association")?;
                let body = module
                    .machine(invocation.inputs.callee)
                    .ok_or("selected public contract lost its executable body")?;
                if (contract.declared_service_reach.is_none()
                    && contract.published_service_ceiling != body.published_service_ceiling)
                    || (contract.declared_synchronous_entries.is_none()
                        && contract.published_synchronous_entries
                            != body.published_synchronous_entries)
                    || (contract.declared_crash_routes.is_none()
                        && contract.crash_routes != body.crash_routes)
                {
                    return Err("selected public effects differ from their exact implementation");
                }
                let dispatch = contract
                    .selected_application
                    .as_ref()
                    .ok_or("selected call public contract has no closed application")?;
                let application = dispatch_application(
                    &applications,
                    machine.id,
                    dispatch,
                    invocation.inputs.callee,
                )?;
                // A proof sidecar on this ordinary call keeps the same public
                // selection. It cannot describe the physical body as a direct
                // proof call while the operation names a public requirement.
                let site = (machine.id, invocation.site);
                let start = proof_calls.partition_point(|(key, _, _)| *key < site);
                let end = proof_calls.partition_point(|(key, _, _)| *key <= site);
                if end - start > 1 {
                    return Err("selected public call has ambiguous proof-call correspondence");
                }
                for (_, proof, callee) in &proof_calls[start..end] {
                    if *callee != invocation.inputs.callee
                        || proof.static_requirement_dispatch.as_ref() != Some(dispatch)
                    {
                        return Err(
                            "selected proof call differs from its operation's public contract",
                        );
                    }
                }
                let declaration = contract
                    .declaration
                    .as_ref()
                    .ok_or("selected call public contract has no source declaration")?;
                let schema = application
                    .schemas
                    .iter()
                    .find(|schema| {
                        schema.declaring_trait_identity == dispatch.declaring_trait_identity
                            && schema.public_requirement_identity
                                == dispatch.public_requirement_identity
                            && schema.requirement_identity == dispatch.requirement_identity
                            && schema.realization_identity == dispatch.realization_identity
                    })
                    .ok_or("selected public contract has no original requirement schema")?;
                match &declaration.specialization {
                    None => {
                        if declaration.state != schema.requirement_declaration
                            || declaration.callable != schema.requirement_identity
                        {
                            return Err(
                                "selected public contract differs from its complete requirement application",
                            );
                        }
                    }
                    Some(specialization) => {
                        if specialization.capture.is_some()
                            || specialization.template != schema.requirement_declaration
                            || specialization.template_callable
                                != schema.public_requirement_identity
                        {
                            return Err("selected public member has another source template");
                        }
                        let mut methods = application.static_applications.iter().filter(|method| {
                            method.public_requirement_identity
                                == dispatch.public_requirement_identity
                                && method.realization_identity == dispatch.realization_identity
                                && method.source_callable_identity
                                    == dispatch.realization_callable_identity
                                && method.callable
                                    == TargetMachineCallable::Machine(dispatch.realization)
                        });
                        let method = methods
                            .next()
                            .ok_or("selected public member lost its exact method application")?;
                        if methods.next().is_some() || method.method_arguments.is_empty() {
                            return Err(
                                "selected public member has no unique complete method tuple",
                            );
                        }
                        let arguments = method.method_arguments.iter().map(|argument| {
                            use language_semantics::conformance_identity::MachineOperandSelectionIdentity;
                            match argument {
                                ClosedReachArgument::Type(index) => {
                                    Ok(MachineOperandSelectionIdentity::Type(identity(module, *index)?.to_owned()))
                                }
                                ClosedReachArgument::Const(index) => {
                                    Ok(MachineOperandSelectionIdentity::Const(identity(module, *index)?.to_owned()))
                                }
                                _ => Err("selected public member needs a complete type or value tuple"),
                            }
                        }).collect::<Result<Vec<_>, _>>()?;
                        let commitment = language_semantics::conformance_identity::requirement_application_commitment(
                            identity(module, specialization.template_callable)?, &arguments,
                        )?;
                        if commitment != specialization.commitment {
                            return Err(
                                "selected public member differs from the invoked method tuple",
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum MethodArgumentOrderKey<'a> {
        Type(&'a str),
        Const(&'a str),
        Proposition(&'a str),
        Machine(&'a str, [u8; 32]),
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct StaticApplicationOrderKey<'a> {
        requirement: &'a str,
        realization: &'a str,
        callable: &'a str,
        arguments: Vec<MethodArgumentOrderKey<'a>>,
        commitment: Option<[u8; 32]>,
    }

    pub fn static_application_order_key<'a>(
        identities: &'a terminal_psi::TerminalIdentityTable,
        application: &terminal_psi::ClosedConformanceStaticApplication,
    ) -> Result<StaticApplicationOrderKey<'a>, &'static str> {
        let text = |identity| {
            identities
                .text(identity)
                .filter(|value| !value.is_empty())
                .ok_or("static application order contains an invalid identity")
        };
        let arguments = application
            .method_arguments
            .iter()
            .map(|argument| {
                Ok(match argument {
                    ClosedReachArgument::Type(identity) => {
                        MethodArgumentOrderKey::Type(text(*identity)?)
                    }
                    ClosedReachArgument::Const(identity) => {
                        MethodArgumentOrderKey::Const(text(*identity)?)
                    }
                    ClosedReachArgument::Proposition(identity) => {
                        MethodArgumentOrderKey::Proposition(text(*identity)?)
                    }
                    ClosedReachArgument::Machine {
                        identity,
                        contract_commitment,
                    } => MethodArgumentOrderKey::Machine(text(*identity)?, *contract_commitment),
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        Ok(StaticApplicationOrderKey {
            requirement: text(application.public_requirement_identity)?,
            realization: text(application.realization_identity)?,
            callable: text(application.source_callable_identity)?,
            arguments,
            commitment: application.application_commitment,
        })
    }
}
pub mod closed_reach {

    use terminal_psi::{ClosedReachParameter, TerminalModule};

    fn validate_telescope(
        kinds: &[u8],
        telescope: &[ClosedReachParameter],
    ) -> Result<(), &'static str> {
        if kinds.len() != telescope.len()
            || !kinds.iter().zip(telescope).all(|(kind, parameter)| {
                *kind
                    == match parameter {
                        ClosedReachParameter::Type { .. } => 0,
                        ClosedReachParameter::Const { .. } => 1,
                        ClosedReachParameter::Proposition { .. } => 2,
                        ClosedReachParameter::Machine(_) => 3,
                    }
            })
        {
            return Err("closed reach arguments differ from their complete declaration telescope");
        }
        Ok(())
    }

    fn validate_operational_contract(
        base: [u8; 32],
        commitment: [u8; 32],
        parameters: &[&ClosedReachParameter],
        suspension: &terminal_psi::ContractDependency<bool, Vec<u32>>,
        blocking: &terminal_psi::ContractDependency<bool, Vec<u32>>,
        carry: &[terminal_psi::ConditionalCarryRequirement],
    ) -> Result<(bool, bool), &'static str> {
        if base == [0; 32]
            || terminal_psi::template_contract_commitment(
                base,
                parameters.len(),
                suspension,
                blocking,
                carry,
            )? != commitment
        {
            return Err(
                "closed callback contract lost its dependency or conditional carry commitment",
            );
        }
        let evaluate = |term: &terminal_psi::ContractDependency<bool, Vec<u32>>,
                        suspension_axis: bool|
         -> Result<bool, &'static str> {
            let mut possible = term.concrete;
            for parameter in &term.parameters {
                let Some(ClosedReachParameter::Machine(binding)) =
                    parameters.get(*parameter as usize).copied()
                else {
                    return Err(
                        "closed callback dependency is outside its exact machine telescope",
                    );
                };
                possible |= if suspension_axis {
                    binding.selected_may_suspend
                } else {
                    binding.selected_may_block
                };
            }
            Ok(possible)
        };
        for requirement in carry {
            // Only the suspension permission is guarded. A true guard leaves
            // the ordinary closed-call carry plan responsible for the crossing;
            // this metadata cannot grant ownership or acquisition permission.
            evaluate(&requirement.condition, true)?;
        }
        Ok((evaluate(suspension, true)?, evaluate(blocking, false)?))
    }

    pub fn validate_closed_reach_applications(module: &TerminalModule) -> Result<(), &'static str> {
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{ClosedReachArgument, TerminalIdentityIndex};

        let machines = module
            .machines
            .iter()
            .map(|machine| (machine.id, machine))
            .collect::<BTreeMap<_, _>>();
        if machines.len() != module.machines.len() {
            return Err("closed reach machine identities are ambiguous");
        }
        let services = module
            .services
            .iter()
            .map(|service| (service.id, service))
            .collect::<BTreeMap<_, _>>();
        if services.len() != module.services.len() {
            return Err("closed reach service identities are ambiguous");
        }
        let identity = |coordinate: TerminalIdentityIndex| {
            module
                .identities
                .text(coordinate)
                .filter(|text| !text.is_empty())
                .ok_or("closed reach has an invalid or empty identity")
        };
        let row = |row: &[semantic_vocabulary::ServiceId]| -> Result<(), &'static str> {
            if row.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("closed reach service row is not strictly ordered");
            }
            for service in row {
                let declaration = services
                    .get(service)
                    .ok_or("closed reach has an unknown service")?;
                if declaration
                    .parents
                    .iter()
                    .any(|parent| row.binary_search(parent).is_err())
                {
                    return Err("closed reach service row lost its parent closure");
                }
            }
            Ok(())
        };
        for machine in &module.machines {
            let operations = machine
                .operations()
                .map(|operation| (operation.id, operation))
                .collect::<BTreeMap<_, _>>();
            if operations.len() != machine.operations().count() {
                return Err("closed reach owner operation identities are ambiguous");
            }
            if operations.values().any(|operation| {
                operation.static_reach_binding.is_some()
                    && operation.kind.module_invocation().is_none()
            }) {
                return Err("closed reach marker is not an ordinary module invocation");
            }
            let invocations = machine
                .module_invocations()
                .map(|invocation| (invocation.site, invocation))
                .collect::<BTreeMap<_, _>>();
            if invocations.len() != machine.module_invocations().count() {
                return Err("closed reach invocation identities are ambiguous");
            }
            let declaration = machine
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.specialization.as_ref());
            let Some(application) = &machine.closed_reach_application else {
                if declaration.is_some() {
                    return Err("checked specialization has no retained application");
                }
                if invocations
                    .values()
                    .any(|invocation| invocation.static_reach_binding.is_some())
                {
                    return Err("closed reach invocation marker has no retained application");
                }
                continue;
            };
            identity(application.template_identity)?;
            if application.template_commitment == [0; 32]
                || application.specialization_commitment == [0; 32]
                || machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| declaration.specialization.as_ref())
                    .map(|specialization| specialization.commitment)
                    != Some(application.specialization_commitment)
            {
                return Err(
                    "closed reach application differs from its checked declaration commitment",
                );
            }
            match (
                declaration.and_then(|declaration| declaration.capture.as_ref()),
                &application.capture,
            ) {
                (None, None) => {}
                (Some(expected), Some(capture))
                    if expected.template_identity == capture.template_identity
                        && expected.template_commitment == capture.template_commitment
                        && expected.specialization_commitment
                            == capture.specialization_commitment =>
                {
                    identity(expected.template_identity)?;
                }
                _ => {
                    return Err(
                        "closed reach capture differs from its independent checked declaration",
                    );
                }
            }
            validate_telescope(&application.parameter_kinds, &application.telescope)?;
            if let Some(capture) = &application.capture {
                validate_telescope(&capture.parameter_kinds, &capture.telescope)?;
                identity(capture.template_identity)?;
                if capture.template_commitment == [0; 32]
                    || capture.specialization_commitment == [0; 32]
                {
                    return Err("closed reach capture lost its checked application commitment");
                }
            }
            let parameters = application.parameters().collect::<Vec<_>>();
            let (may_suspend, may_block) = validate_operational_contract(
                application.template_base_commitment,
                application.template_commitment,
                &parameters,
                &application.suspension_dependency,
                &application.blocking_dependency,
                &application.conditional_carry,
            )?;
            let published = machine
                .declaration
                .as_ref()
                .ok_or("closed callback application lost its checked public declaration")?;
            if may_suspend != published.published_may_suspend
                || may_block != published.published_may_block
            {
                return Err(
                    "closed callback substitution differs from its published operational contract",
                );
            }
            if let Some(capture) = &application.capture {
                validate_operational_contract(
                    capture.template_base_commitment,
                    capture.template_commitment,
                    // The capture's template still owns the method binders.
                    // A closed method supplies those after its capture prefix.
                    &parameters,
                    &capture.suspension_dependency,
                    &capture.blocking_dependency,
                    &capture.conditional_carry,
                )?;
            }
            row(&application.fixed)?;
            row(&machine.published_service_ceiling)?;
            for parameter in application.parameters() {
                match parameter {
                    ClosedReachParameter::Type { argument }
                    | ClosedReachParameter::Const { argument }
                    | ClosedReachParameter::Proposition { argument }
                        if argument.is_empty() =>
                    {
                        return Err("closed reach telescope has an empty argument");
                    }
                    ClosedReachParameter::Machine(binding) => {
                        identity(binding.selected_identity)?;
                        if binding.selected_contract_commitment == [0; 32]
                            || binding
                                .nominal_requirement
                                .as_ref()
                                .is_some_and(String::is_empty)
                        {
                            return Err("closed reach machine binder lost its public contract");
                        }
                        if binding.requirement_parameter_kinds.len()
                            != binding.requirement_arguments.len()
                            || binding
                                .requirement_parameter_kinds
                                .iter()
                                .any(|kind| *kind > 5)
                            || (binding.nominal_requirement.is_none()
                                && !binding.requirement_arguments.is_empty())
                        {
                            return Err(
                                "closed reach requirement lost its full declaration telescope",
                            );
                        }
                        for argument in &binding.requirement_arguments {
                            identity(*argument)?;
                        }
                        if (binding.selected_may_suspend && !binding.upper_bound_may_suspend)
                            || (binding.selected_may_block && !binding.upper_bound_may_block)
                        {
                            return Err(
                                "selected callback operational contract exceeds its requirement bound",
                            );
                        }
                        row(&binding.upper_bound)?;
                        row(&binding.selected_reach)?;
                        if binding
                            .selected_reach
                            .iter()
                            .any(|service| binding.upper_bound.binary_search(service).is_err())
                        {
                            return Err(
                                "closed reach selected contract exceeds its requirement bound",
                            );
                        }
                        if let Some(callee) = binding.callee {
                            let callee = machines
                                .get(&callee)
                                .ok_or("closed reach selected callee is absent")?;
                            if callee
                                .declaration
                                .as_ref()
                                .map(|declaration| declaration.contract)
                                != Some(binding.selected_contract_commitment)
                                || callee.published_service_ceiling != binding.selected_reach
                                || callee.declaration.as_ref().is_none_or(|declaration| {
                                    declaration.published_may_suspend
                                        != binding.selected_may_suspend
                                        || declaration.published_may_block
                                            != binding.selected_may_block
                                })
                            {
                                return Err(
                                    "closed reach selection differs from its retained callee contract",
                                );
                            }
                        }
                        if let Some(schema) = &binding.schema {
                            identity(schema.template_identity)?;
                            if schema.template_commitment == [0; 32] {
                                return Err("closed reach schema has an empty template commitment");
                            }
                        }
                    }
                    _ => {}
                }
            }
            if application
                .dependencies
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err("closed reach dependency binders are not strictly ordered");
            }
            let mut union = application.fixed.iter().copied().collect::<BTreeSet<_>>();
            for dependency in &application.dependencies {
                let Some(ClosedReachParameter::Machine(binding)) =
                    application.parameter(*dependency)
                else {
                    return Err("closed reach dependency is outside its machine telescope");
                };
                if binding.nominal_requirement.is_none() {
                    return Err("closed reach dependency selects a structural fixed-row binder");
                }
                union.extend(binding.selected_reach.iter().copied());
            }
            if union.into_iter().collect::<Vec<_>>() != machine.published_service_ceiling {
                return Err("closed reach finite union differs from its published service row");
            }
            if application
                .calls
                .windows(2)
                .any(|pair| pair[0].site >= pair[1].site)
            {
                return Err("closed reach call occurrences are not strictly ordered");
            }
            for call in &application.calls {
                let invocation = invocations
                    .get(&call.site)
                    .ok_or("closed reach call has no exact owner invocation")?;
                if invocation.static_reach_binding != Some(call.binder) {
                    return Err("closed reach call differs from its invocation binder marker");
                }
                let Some(ClosedReachParameter::Machine(binding)) =
                    application.parameter(call.binder)
                else {
                    return Err("closed reach call is outside its machine telescope");
                };
                let callee = invocation.inputs.callee;
                match (&binding.schema, &call.application) {
                    (None, None) if binding.callee == Some(callee) => {}
                    (Some(schema), Some(expected)) if expected.callee == callee => {
                        let selected = machines
                            .get(&callee)
                            .ok_or("closed reach schema callee is absent")?;
                        let selected = selected
                            .closed_reach_application
                            .as_ref()
                            .ok_or("closed reach schema callee lost its retained application")?;
                        if identity(schema.template_identity)?
                            != identity(selected.template_identity)?
                            || schema.template_commitment != selected.template_commitment
                            || expected.specialization_commitment
                                != selected.specialization_commitment
                            || expected.arguments.len() != selected.telescope.len()
                        {
                            return Err(
                                "closed reach schema call differs from its exact retained application",
                            );
                        }
                        for (argument, parameter) in
                            expected.arguments.iter().zip(&selected.telescope)
                        {
                            let coordinate = match argument {
                                ClosedReachArgument::Type(identity)
                                | ClosedReachArgument::Const(identity)
                                | ClosedReachArgument::Proposition(identity) => *identity,
                                ClosedReachArgument::Machine {
                                    identity,
                                    contract_commitment,
                                } => {
                                    if *contract_commitment == [0; 32] {
                                        return Err(
                                            "closed reach call tuple has an empty machine contract",
                                        );
                                    }
                                    *identity
                                }
                            };
                            identity(coordinate)?;
                            if !crate::calls::closed_conformances::argument_matches(
                                module, argument, parameter,
                            ) {
                                return Err(
                                    "closed reach call tuple differs from its retained telescope",
                                );
                            }
                        }
                    }
                    _ => return Err("closed reach direct call lost its exact selection or schema"),
                }
            }
            for invocation in invocations.values() {
                if let Some(binder) = invocation.static_reach_binding {
                    let call = application
                        .calls
                        .binary_search_by_key(&invocation.site, |call| call.site)
                        .ok()
                        .and_then(|position| application.calls.get(position));
                    if call.is_none_or(|call| call.binder != binder) {
                        return Err("closed reach invocation marker has no exact call occurrence");
                    }
                }
            }
        }
        Ok(())
    }
}
pub mod evidence_calls {
    use crate::descriptors::descriptor_lifetimes::{DescriptorLocation, DescriptorPosition};
    use crate::descriptors::descriptor_results::{
        self, DescriptorAnalysis, DescriptorCasePredicate, DescriptorMachine,
        PayloadMembershipHistory, PayloadScalarObservation,
    };
    pub(crate) mod inputs {
        use crate::descriptors::descriptor_lifetimes::DescriptorPosition;
        use crate::descriptors::descriptor_results::{
            DescriptorMachine, PayloadMembershipContract,
        };
        use semantic_vocabulary::{
            BlockId, InvocationSite, MachineId, OperationId, PlaceId, ScalarTerm, ValueId,
        };
        use terminal_psi::{
            CheckedMachineDeclaration, DirectCallParts, DirectCallRole, EvidenceContractLane,
            EvidenceTermDeclaration, OutcomeSpecificCallEvidence, OutcomeSpecificEnsure,
            ProofOutputCall, PropositionApplicationIdentity, StructuralArgument,
            StructuralDomainDeclaration, StructuralOperationResult, StructuralParameterDeclaration,
            StructuralTypeDeclaration, TerminalIdentityTable,
        };

        pub enum RuntimeEvidenceResult {
            Unit,
            NoNormal,
            Scalar {
                id: ValueId,
                scalar_type: semantic_vocabulary::ScalarType,
            },
            Structural(StructuralOperationResult),
        }
        impl RuntimeEvidenceResult {
            pub fn scalar(&self) -> Option<(ValueId, semantic_vocabulary::ScalarType)> {
                match self {
                    Self::Scalar { id, scalar_type } => Some((*id, *scalar_type)),
                    _ => None,
                }
            }
            pub fn structural(&self) -> Option<&StructuralOperationResult> {
                match self {
                    Self::Structural(result) => Some(result),
                    _ => None,
                }
            }
        }

        pub struct RuntimeEvidenceOperation {
            pub site: InvocationSite,
            pub position: DescriptorPosition,
            pub callee: MachineId,
            pub role: DirectCallRole,
            pub arguments: Vec<ValueId>,
            pub structural_arguments: Vec<StructuralArgument>,
            pub result: RuntimeEvidenceResult,
            pub selected_evidence: Vec<OutcomeSpecificCallEvidence>,
        }
        impl RuntimeEvidenceOperation {
            pub fn parts(&self) -> DirectCallParts<'_> {
                DirectCallParts {
                    callee: self.callee,
                    role: self.role,
                    arguments: &self.arguments,
                    structural_arguments: &self.structural_arguments,
                    selected_evidence: &self.selected_evidence,
                }
            }
        }

        pub struct RuntimeEvidenceBody {
            pub id: MachineId,
            pub entry: BlockId,
            pub declaration: Option<CheckedMachineDeclaration>,
            pub parameters: Vec<ValueId>,
            pub structural_parameters: Vec<StructuralParameterDeclaration>,
            pub outcome_specific_ensures: Vec<OutcomeSpecificEnsure>,
            pub calls: Vec<RuntimeEvidenceOperation>,
            pub scalar_expressions: Vec<(ValueId, ScalarTerm)>,
            pub payload_contracts: Vec<(PlaceId, PayloadMembershipContract)>,
        }
        impl RuntimeEvidenceBody {
            pub fn operations(&self) -> impl Iterator<Item = &RuntimeEvidenceOperation> {
                self.calls.iter()
            }
            pub fn operation(&self, id: OperationId) -> Option<&RuntimeEvidenceOperation> {
                self.invocation(InvocationSite::Operation(id))
            }
            pub fn invocation(&self, site: InvocationSite) -> Option<&RuntimeEvidenceOperation> {
                let mut rows = self.calls.iter().filter(|row| row.site == site);
                let row = rows.next()?;
                rows.next().is_none().then_some(row)
            }
            pub fn scalar_expression(&self, id: ValueId) -> Option<&ScalarTerm> {
                let mut rows = self
                    .scalar_expressions
                    .iter()
                    .filter(|(value, _)| *value == id);
                let (_, expression) = rows.next()?;
                rows.next().is_none().then_some(expression)
            }
            pub fn payload_contract(&self, root: PlaceId) -> Option<&PayloadMembershipContract> {
                let mut rows = self
                    .payload_contracts
                    .iter()
                    .filter(|(place, _)| *place == root);
                let (_, contract) = rows.next()?;
                rows.next().is_none().then_some(contract)
            }
        }

        pub struct RuntimeEvidenceInputs<'input> {
            pub identities: &'input TerminalIdentityTable,
            pub structural_types: &'input [StructuralTypeDeclaration],
            pub structural_domains: &'input [StructuralDomainDeclaration],
            pub proposition_applications: &'input [PropositionApplicationIdentity],
            pub evidence_terms: &'input [EvidenceTermDeclaration],
            pub evidence_contract_lanes: &'input [EvidenceContractLane],
            pub proof_output_calls: &'input [ProofOutputCall],
            pub machines: &'input [RuntimeEvidenceBody],
            pub graph: &'input [DescriptorMachine],
        }
        impl RuntimeEvidenceInputs<'_> {
            pub fn machine(&self, id: MachineId) -> Option<&RuntimeEvidenceBody> {
                let mut rows = self.machines.iter().filter(|row| row.id == id);
                let row = rows.next()?;
                rows.next().is_none().then_some(row)
            }
        }
    }
    pub use inputs::{
        RuntimeEvidenceBody, RuntimeEvidenceInputs, RuntimeEvidenceOperation, RuntimeEvidenceResult,
    };

    use semantic_vocabulary::{EvidenceTermId, MachineId, PropositionId, ValueId};
    use terminal_psi::{
        DirectCallParts, EvidenceArgumentSubject, EvidenceContractLane, EvidenceContractLaneKind,
        ProofOutputCall, StructuralAccess, StructuralMultiplicity, TerminalModule,
    };

    #[derive(Clone)]
    enum ArgumentValue {
        Closed(String),
        Scalar(PayloadScalarObservation),
    }

    pub fn validate_runtime_evidence_inputs(module: &TerminalModule) -> Result<(), &'static str> {
        terminal_inputs::validate(module)
    }
    pub(crate) mod terminal_inputs {
        use super::*;
        use semantic_vocabulary::InvocationSite;
        use terminal_psi::OperationResult;

        pub(crate) fn validate(module: &TerminalModule) -> Result<(), &'static str> {
            for invocation in &module.proof_output_calls {
                if invocation
                    .runtime_call
                    .is_some_and(|call| matches!(call.site, InvocationSite::ExternalCompletion(_)))
                {
                    return Err("ordinary evidence invocation names an external completion");
                }
                let no_normal = invocation.runtime_result
                    == Some(terminal_psi::ProofOutputRuntimeResult::NoNormal);
                let edge = invocation
                    .runtime_call
                    .is_some_and(|call| matches!(call.site, InvocationSite::InvokeNoNormal(_)));
                if no_normal || edge {
                    if !no_normal || !edge || !invocation.outputs.is_empty() {
                        return Err(
                            "nonreturning evidence invocation has a normal result or output",
                        );
                    }
                    let retained = invocation
                        .runtime_call
                        .ok_or("nonreturning evidence invocation has no call")?;
                    let caller = module
                        .machine(invocation.caller)
                        .ok_or("nonreturning evidence invocation has no caller")?;
                    let mut calls = caller
                        .module_invocations()
                        .filter(|call| call.site == retained.site);
                    if calls
                        .next()
                        .is_none_or(|call| call.inputs.callee != retained.callee)
                        || calls.next().is_some()
                    {
                        return Err(
                            "nonreturning evidence invocation has no exact executable edge",
                        );
                    }
                }
            }
            let consumes_snapshots = module
                .proof_output_calls
                .iter()
                .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty());
            if module.evidence_contract_lanes.is_empty() && !consumes_snapshots {
                return Ok(());
            }
            let checks_calls = consumes_snapshots
                || module
                    .evidence_contract_lanes
                    .iter()
                    .any(|lane| lane.kind == EvidenceContractLaneKind::Requires);
            let graph = if consumes_snapshots {
                descriptor_results::terminal_graph(module)
            } else {
                Vec::new()
            };
            let mut bodies = Vec::with_capacity(module.machines.len());
            for machine in &module.machines {
                let observes = module.proof_output_calls.iter().any(|call| {
                    call.caller == machine.id
                        && call.runtime_call.is_some()
                        && !call.evidence_arguments.is_empty()
                });
                let mut calls = Vec::new();
                for block in machine.blocks.iter().filter(|_| checks_calls) {
                    for (index, operation) in block.operations.iter().enumerate() {
                        let Some(parts) = terminal_psi::direct_call_parts(&operation.kind) else {
                            continue;
                        };
                        calls.push(RuntimeEvidenceOperation {
                            site: InvocationSite::Operation(operation.id),
                            position: (
                                block.id,
                                u32::try_from(index)
                                    .map_err(|_| "evidence operation position exceeds u32")?,
                            ),
                            callee: parts.callee,
                            role: parts.role,
                            arguments: parts.arguments.to_vec(),
                            structural_arguments: parts.structural_arguments.to_vec(),
                            result: match &operation.result {
                                OperationResult::Unit => RuntimeEvidenceResult::Unit,
                                OperationResult::Scalar(value) => RuntimeEvidenceResult::Scalar {
                                    id: value.id,
                                    scalar_type: value.scalar_type,
                                },
                                OperationResult::Structural(result) => {
                                    RuntimeEvidenceResult::Structural(result.clone())
                                }
                            },
                            selected_evidence: parts.selected_evidence.to_vec(),
                        });
                    }
                    if let Some(invocation) = block.terminator.module_invocation() {
                        calls.push(RuntimeEvidenceOperation {
                            site: InvocationSite::InvokeNoNormal(block.terminator.edge()),
                            position: (
                                block.id,
                                u32::try_from(block.operations.len())
                                    .map_err(|_| "evidence invocation position exceeds u32")?,
                            ),
                            callee: invocation.callee,
                            role: terminal_psi::DirectCallRole::NoNormal,
                            arguments: invocation.arguments.to_vec(),
                            structural_arguments: invocation.structural_arguments.to_vec(),
                            result: RuntimeEvidenceResult::NoNormal,
                            selected_evidence: Vec::new(),
                        });
                    }
                }
                let mut values = Vec::new();
                if observes {
                    values.extend(machine.parameters.iter().map(|value| value.id));
                    for call in &calls {
                        values.extend_from_slice(&call.arguments);
                        if let Some(result) = call.result.scalar() {
                            values.push(result.0);
                        }
                    }
                    values.sort_unstable();
                    values.dedup();
                }
                let scalar_expressions = if observes {
                    descriptor_results::terminal_scalar_snapshots(machine, &values)
                } else {
                    Vec::new()
                };
                let payload_contracts = if observes {
                    descriptor_results::descriptor_machine(&graph, machine.id)
                        .ok_or("runtime evidence body has no current descriptor machine")?
                        .place_types
                        .keys()
                        .filter_map(|place| {
                            descriptor_results::terminal_payload_contract(machine, *place)
                                .map(|contract| (*place, contract))
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                bodies.push(RuntimeEvidenceBody {
                    id: machine.id,
                    entry: machine.entry,
                    declaration: machine.declaration.clone(),
                    parameters: machine
                        .parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect(),
                    structural_parameters: machine.structural_parameters.clone(),
                    outcome_specific_ensures: machine.contract.outcome_specific_ensures.clone(),
                    calls,
                    scalar_expressions,
                    payload_contracts,
                });
            }
            validate_runtime_evidence_inputs_in(&RuntimeEvidenceInputs {
                identities: &module.identities,
                structural_types: &module.structural_types,
                structural_domains: &module.structural_domains,
                proposition_applications: &module.proposition_applications,
                evidence_terms: &module.evidence_terms,
                evidence_contract_lanes: &module.evidence_contract_lanes,
                proof_output_calls: &module.proof_output_calls,
                machines: &bodies,
                graph: &graph,
            })
        }
    }

    pub fn validate_runtime_evidence_inputs_in(
        module: &RuntimeEvidenceInputs<'_>,
    ) -> Result<(), &'static str> {
        validate_subject_rosters(module)?;
        if !module
            .evidence_contract_lanes
            .iter()
            .any(|lane| lane.kind == EvidenceContractLaneKind::Requires)
            && !module
                .proof_output_calls
                .iter()
                .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty())
        {
            return Ok(());
        }
        let graph = module.graph;
        let mut retained = Vec::new();
        for caller in module.machines {
            if module.machine(caller.id).is_none() {
                return Err("evidence input body identity is ambiguous");
            }
            let mut history = None;
            for operation in caller.operations() {
                let parts = operation.parts();
                let expected = input_lanes(module, parts.callee);
                let mut invocations = module.proof_output_calls.iter().filter(|invocation| {
                    invocation.caller == caller.id
                        && invocation.runtime_call.is_some_and(|runtime| {
                            runtime.site == operation.site && runtime.callee == parts.callee
                        })
                });
                let Some(invocation) = invocations.next() else {
                    if expected.is_empty() {
                        continue;
                    }
                    return Err("runtime call omitted its declared evidence inputs");
                };
                if invocation.static_requirement_dispatch.is_some() {
                    continue;
                }
                if invocations.next().is_some()
                    || invocation.evidence_arguments.len() != expected.len()
                {
                    return Err(
                        "runtime call has ambiguous or incomplete ordinary evidence inputs",
                    );
                }
                retained.push((caller.id, operation.site));
                let use_position = position(caller, operation)?;
                let callee = module
                    .machine(parts.callee)
                    .ok_or("evidence input callee is absent")?;
                let declaration = callee
                    .declaration
                    .as_ref()
                    .ok_or("evidence target has no checked callable declaration")?;
                let callable = module
                    .identities
                    .text(declaration.callable)
                    .ok_or("evidence target callable identity is absent")?;
                let expected_identity =
                    terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                        callable,
                        declaration
                            .specialization
                            .as_ref()
                            .map(|application| &application.commitment),
                    );
                if module.identities.text(invocation.target_machine_identity)
                    != Some(expected_identity.as_str())
                {
                    return Err("ordinary evidence invocation changed its exact checked callable");
                }
                match (
                    parts.role,
                    invocation.runtime_result,
                    operation.result.scalar(),
                ) {
                    (
                        terminal_psi::DirectCallRole::NoNormal,
                        Some(terminal_psi::ProofOutputRuntimeResult::NoNormal),
                        None,
                    ) if matches!(operation.result, RuntimeEvidenceResult::NoNormal) => {}
                    (
                        terminal_psi::DirectCallRole::Unit,
                        Some(terminal_psi::ProofOutputRuntimeResult::Unit),
                        None,
                    ) => {}
                    (
                        terminal_psi::DirectCallRole::Scalar,
                        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(expected)),
                        Some((_, actual)),
                    ) if actual == expected => {}
                    _ => return Err("runtime evidence invocation changed its exact result role"),
                }
                if parts.role == terminal_psi::DirectCallRole::NoNormal {
                    if !invocation.outputs.is_empty() {
                        return Err("nonreturning evidence invocation publishes normal outputs");
                    }
                } else {
                    validate_output_roster(module, invocation, parts.callee)?;
                }
                for (input_position, (argument, lane)) in invocation
                    .evidence_arguments
                    .iter()
                    .zip(expected)
                    .enumerate()
                {
                    let target_term = term(module, lane.term)?;
                    if lane
                        .argument_subjects
                        .iter()
                        .any(|subject| matches!(subject, EvidenceArgumentSubject::Result { .. }))
                    {
                        return Err("an evidence Requires lane cannot depend on its future result");
                    }
                    if lane.position != input_position as u32
                        || argument.input_position != lane.position
                        || argument.callee_proposition != target_term.proposition
                    {
                        return Err(
                            "runtime evidence argument changed its exact declared input lane",
                        );
                    }
                    if history.is_none() {
                        history = Some(PayloadMembershipHistory::for_current_contracts(
                            module.structural_types,
                            module.structural_domains,
                            graph,
                            caller.id,
                            &caller.structural_parameters,
                            &caller.payload_contracts,
                        )?);
                    }
                    let history = history
                        .as_mut()
                        .ok_or("runtime evidence payload history is absent")?;
                    let target_application =
                        application(module, argument.instantiated_proposition)?;
                    let declared = application(module, target_term.proposition)?;
                    same_head(declared, target_application)?;
                    let target = observe_arguments(
                        module,
                        caller,
                        callee,
                        Some((operation, parts)),
                        use_position,
                        &lane.argument_subjects,
                        &target_application.arguments,
                        graph,
                        history,
                    )?;
                    let (source_proposition, source, source_cut) = source_arguments(
                        module,
                        caller,
                        invocation,
                        argument.source,
                        graph,
                        history,
                    )?;
                    let source_term = term(module, argument.source)?;
                    if source_term.interface != target_term.interface {
                        return Err("runtime evidence argument changed its witness interface");
                    }
                    same_head(application(module, source_proposition)?, target_application)?;
                    if source.len() != target.len() {
                        return Err(
                            "runtime evidence argument changed its complete proposition arity",
                        );
                    }
                    let mut captured_values = Vec::new();
                    for (source, target) in source.iter().zip(&target) {
                        match (source, target) {
                            (ArgumentValue::Closed(source), ArgumentValue::Closed(target))
                                if source == target => {}
                            (ArgumentValue::Scalar(source), ArgumentValue::Scalar(target)) => {
                                let entry_equal = source
                                    .entry
                                    .as_ref()
                                    .zip(target.entry.as_ref())
                                    .is_some_and(|(source, target)| source == target);
                                let value_equal = source
                                    .value
                                    .as_ref()
                                    .zip(target.value.as_ref())
                                    .is_some_and(|(source, target)| source == target);
                                if !entry_equal && !value_equal {
                                    return Err(
                                        "runtime evidence argument names a different captured value",
                                    );
                                }
                                if !entry_equal && let Some(value) = &source.value {
                                    value.visit_value_ids(|id| {
                                        captured_values.push(id);
                                        true
                                    });
                                }
                            }
                            _ => {
                                return Err(
                                    "runtime evidence argument has no complete typed snapshot correspondence",
                                );
                            }
                        }
                    }
                    if let Some(source_cut) = source_cut {
                        require_completed_capture(
                            graph,
                            caller.id,
                            source_cut,
                            use_position,
                            &captured_values,
                        )?;
                    }
                }
            }
        }
        if module.proof_output_calls.iter().any(|call| {
            call.static_requirement_dispatch.is_none()
                && call
                    .runtime_call
                    .is_some_and(|runtime| !retained.contains(&(call.caller, runtime.site)))
        }) {
            return Err("ordinary runtime evidence invocation has no authenticated call operation");
        }
        Ok(())
    }

    fn validate_subject_rosters(module: &RuntimeEvidenceInputs<'_>) -> Result<(), &'static str> {
        for lane in module.evidence_contract_lanes {
            let owner = module
                .machine(lane.machine)
                .ok_or("evidence subject owner is absent")?;
            let term = term(module, lane.term)?;
            let application = application(module, term.proposition)?;
            if application.evidence_interface.as_ref() != Some(&term.interface)
                || lane.argument_subjects.len() != application.arguments.len()
                || lane
                    .argument_subjects
                    .iter()
                    .enumerate()
                    .any(|(position, subject)| subject.argument_position() as usize != position)
            {
                return Err("evidence declaration has no complete exact typed subject roster");
            }
            for subject in &lane.argument_subjects {
                match subject {
                    EvidenceArgumentSubject::Parameter(parameter) => {
                        let exists = if parameter.structural_path.is_some() {
                            owner
                                .structural_parameters
                                .get(parameter.parameter_position as usize)
                                .is_some()
                        } else {
                            owner
                                .parameters
                                .get(parameter.parameter_position as usize)
                                .is_some()
                        };
                        if !exists {
                            return Err("evidence declaration subject names an absent formal");
                        }
                    }
                    EvidenceArgumentSubject::Result { .. }
                        if lane.kind == EvidenceContractLaneKind::Requires =>
                    {
                        return Err("an evidence Requires lane cannot depend on its future result");
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn validate_output_roster(
        module: &RuntimeEvidenceInputs<'_>,
        invocation: &ProofOutputCall,
        callee: MachineId,
    ) -> Result<(), &'static str> {
        let mut outputs = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == callee && lane.kind == EvidenceContractLaneKind::Ensures)
            .collect::<Vec<_>>();
        outputs.sort_by_key(|lane| lane.position);
        if invocation.outputs.len() != outputs.len() {
            return Err("ordinary evidence invocation omitted a declared output");
        }
        for (position, (output, lane)) in invocation.outputs.iter().zip(outputs).enumerate() {
            let declared = term(module, lane.term)?;
            if lane.position as usize != position
                || output.output_position != lane.position
                || lane.output_field.as_deref() != Some(output.output_field.as_str())
                || output.callee_proposition != declared.proposition
            {
                return Err("ordinary evidence output changed its exact declaration lane");
            }
            same_head(
                application(module, declared.proposition)?,
                application(module, output.instantiated_proposition)?,
            )?;
            if let Some(input_position) = output.forwarded_input_position {
                let inputs = input_lanes(module, callee);
                let input = inputs
                    .get(input_position as usize)
                    .ok_or("forwarded output has no declared input")?;
                let source = term(module, input.term)?;
                if output.callee_output.is_some()
                    || source.proposition != declared.proposition
                    || source.interface != declared.interface
                    || input.argument_subjects != lane.argument_subjects
                {
                    return Err("forwarded output changed its exact declared input meaning");
                }
            } else if output.callee_output != Some(lane.term) {
                return Err("ordinary evidence output has no exact declared producer");
            }
            if let Some(output_term) = output.output {
                let target = term(module, output_term)?;
                if target.proposition != output.instantiated_proposition
                    || target.interface != declared.interface
                {
                    return Err(
                        "ordinary evidence output changed its receiving witness declaration",
                    );
                }
            }
        }
        Ok(())
    }

    fn input_lanes<'a>(
        module: &'a RuntimeEvidenceInputs<'_>,
        machine: MachineId,
    ) -> Vec<&'a EvidenceContractLane> {
        let mut lanes = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| {
                lane.machine == machine && lane.kind == EvidenceContractLaneKind::Requires
            })
            .collect::<Vec<_>>();
        lanes.sort_by_key(|lane| lane.position);
        lanes
    }

    fn term<'a>(
        module: &'a RuntimeEvidenceInputs<'_>,
        id: EvidenceTermId,
    ) -> Result<&'a terminal_psi::EvidenceTermDeclaration, &'static str> {
        let mut rows = module.evidence_terms.iter().filter(|term| term.id == id);
        let term = rows.next().ok_or("runtime evidence term is absent")?;
        if rows.next().is_some() {
            return Err("runtime evidence term identity is ambiguous");
        }
        Ok(term)
    }
    fn application<'a>(
        module: &'a RuntimeEvidenceInputs<'_>,
        id: PropositionId,
    ) -> Result<&'a terminal_psi::PropositionApplicationIdentity, &'static str> {
        let mut rows = module
            .proposition_applications
            .iter()
            .filter(|row| row.id == id);
        let row = rows
            .next()
            .ok_or("runtime evidence proposition is absent")?;
        if rows.next().is_some() {
            return Err("runtime evidence proposition identity is ambiguous");
        }
        Ok(row)
    }
    fn same_head(
        source: &terminal_psi::PropositionApplicationIdentity,
        target: &terminal_psi::PropositionApplicationIdentity,
    ) -> Result<(), &'static str> {
        if source.declaration != target.declaration
            || source.binder_arguments != target.binder_arguments
            || source.arguments.len() != target.arguments.len()
            || source.evidence_interface != target.evidence_interface
        {
            return Err("runtime evidence transport changed its nominal application head");
        }
        Ok(())
    }
    fn position(
        machine: &RuntimeEvidenceBody,
        operation: &RuntimeEvidenceOperation,
    ) -> Result<DescriptorPosition, &'static str> {
        let mut positions = machine
            .calls
            .iter()
            .filter(|candidate| candidate.site == operation.site);
        let current = positions
            .next()
            .ok_or("evidence operation is absent from its caller")?;
        if positions.next().is_some() {
            return Err("evidence operation identity is ambiguous");
        }
        Ok(current.position)
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_arguments(
        module: &RuntimeEvidenceInputs<'_>,
        caller: &RuntimeEvidenceBody,
        callee: &RuntimeEvidenceBody,
        actual: Option<(&RuntimeEvidenceOperation, DirectCallParts<'_>)>,
        cut: DescriptorPosition,
        subjects: &[EvidenceArgumentSubject],
        public_arguments: &[String],
        graph: &[DescriptorMachine],
        history: &mut PayloadMembershipHistory<'_>,
    ) -> Result<Vec<ArgumentValue>, &'static str> {
        if subjects.len() != public_arguments.len() {
            return Err("evidence declaration omitted a proposition subject");
        }
        subjects.iter().enumerate().map(|(index, subject)| {
            if subject.argument_position() as usize != index { return Err("evidence argument subjects are not complete and positional"); }
            match subject {
                EvidenceArgumentSubject::Closed { .. } => Ok(ArgumentValue::Closed(public_arguments[index].clone())),
                EvidenceArgumentSubject::Unknown { .. } => Err("evidence argument has no retained typed subject"),
                EvidenceArgumentSubject::Result { .. } => {
                    let (operation, _) = actual.ok_or("an invocation input cannot refer to its result")?;
                    let value = operation.result.scalar().ok_or("evidence result subject is not scalar")?;
                    scalar_argument(module, caller, value.0, cut, graph, history)
                }
                EvidenceArgumentSubject::Parameter(parameter) => {
                    if let Some(subject_path) = &parameter.structural_path {
                        let formal = callee.structural_parameters.get(parameter.parameter_position as usize)
                            .ok_or("evidence subject has no structural formal")?;
                        if formal.access != StructuralAccess::Owned || formal.multiplicity != StructuralMultiplicity::Unrestricted {
                            return Err("evidence snapshot requires its independent borrowed or affine scope owner");
                        }
                        let (root, prefix) = if let Some((_, parts)) = actual {
                            let source = parts.structural_arguments.get(parameter.parameter_position as usize)
                                .ok_or("evidence subject has no evaluated structural actual")?;
                            if source.access != StructuralAccess::Owned { return Err("evidence actual is not an owned value snapshot"); }
                            (source.place, source.path.as_slice())
                        } else { (formal.place, &[][..]) };
                        let root_type = caller.payload_contract(root)
                            .ok_or("evidence snapshot actual has no current carrier")?.carrier;
                        let mut path = prefix.to_vec(); path.extend_from_slice(subject_path);
                        let selected = crate::descriptors::descriptor_lifetimes::descriptor_location(
                            module.structural_types, root, root_type, &path, None,
                        ).ok_or("evidence snapshot has no exact typed storage path")?;
                        Ok(ArgumentValue::Scalar(history.scalar_snapshot(cut, root, &selected.path)?))
                    } else {
                        let formal = callee.parameters.get(parameter.parameter_position as usize)
                            .ok_or("evidence subject has no scalar formal")?;
                        let value = actual.map_or(Some(*formal), |(_, parts)| parts.arguments.get(parameter.parameter_position as usize).copied())
                            .ok_or("evidence subject has no evaluated scalar actual")?;
                        scalar_argument(module, caller, value, cut, graph, history)
                    }
                }
            }
        }).collect()
    }
    fn scalar_argument(
        _module: &RuntimeEvidenceInputs<'_>,
        caller: &RuntimeEvidenceBody,
        value: ValueId,
        cut: DescriptorPosition,
        graph: &[DescriptorMachine],
        history: &mut PayloadMembershipHistory<'_>,
    ) -> Result<ArgumentValue, &'static str> {
        let source_value = value;
        let value = caller
            .scalar_expression(value)
            .ok_or("evidence scalar actual has no exact executable value")?
            .clone();
        let machine = descriptor_results::descriptor_machine(graph, caller.id)
            .ok_or("evidence caller has no current payload graph")?;
        let mut reads = machine
            .scalar_payload_reads
            .iter()
            .flat_map(|(position, reads)| {
                reads.iter().filter_map(|(location, observed)| {
                    (observed == &value).then_some((*position, location))
                })
            });
        if let Some((position, location)) = reads.next() {
            if reads.next().is_some() {
                return Err("evidence scalar observation is ambiguous");
            }
            require_completed_capture(graph, caller.id, position, cut, &[source_value])?;
            return Ok(ArgumentValue::Scalar(history.scalar_snapshot(
                position,
                location.root,
                &location.path,
            )?));
        }
        Ok(ArgumentValue::Scalar(PayloadScalarObservation {
            path: Vec::new(),
            value: Some(value),
            entry: None,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn source_arguments(
        module: &RuntimeEvidenceInputs<'_>,
        caller: &RuntimeEvidenceBody,
        consumer: &ProofOutputCall,
        source: EvidenceTermId,
        graph: &[DescriptorMachine],
        history: &mut PayloadMembershipHistory<'_>,
    ) -> Result<
        (
            PropositionId,
            Vec<ArgumentValue>,
            Option<DescriptorPosition>,
        ),
        &'static str,
    > {
        let source_term = term(module, source)?;
        let source_application = application(module, source_term.proposition)?;
        let mut entries = input_lanes(module, caller.id)
            .into_iter()
            .filter(|lane| lane.term == source);
        if let Some(entry) = entries.next() {
            if entries.next().is_some() {
                return Err("runtime evidence source has ambiguous entry scope");
            }
            let arguments = observe_arguments(
                module,
                caller,
                caller,
                None,
                (caller.entry, 0),
                &entry.argument_subjects,
                &source_application.arguments,
                graph,
                history,
            )?;
            return Ok((source_term.proposition, arguments, None));
        }
        let mut guarded = caller.operations().filter_map(|operation| {
            let parts = operation.parts();
            let mut rows = parts
                .selected_evidence
                .iter()
                .filter(|row| row.output == source);
            let row = rows.next()?;
            Some((operation, parts, row, rows.next().is_some()))
        });
        if let Some((operation, parts, row, duplicate)) = guarded.next() {
            if duplicate
                || guarded.next().is_some()
                || row.instantiated_proposition != source_term.proposition
                || row.validity.evidence_interface != source_term.interface
            {
                return Err("runtime evidence source has ambiguous guarded provenance");
            }
            let callee = module
                .machine(parts.callee)
                .ok_or("guarded evidence callee is absent")?;
            let mut declarations = callee.outcome_specific_ensures.iter().filter(|ensure| {
                ensure.guard == row.guard
                    && ensure.position == row.position
                    && ensure.obligation == row.callee_obligation
                    && ensure.proposition
                        == semantic_vocabulary::Proposition::Atom(row.callee_proposition)
                    && ensure.evidence.as_ref().is_some_and(|evidence| {
                        evidence.term == row.callee_term
                            && evidence.output_field == row.output_field
                            && evidence.argument_subjects == row.argument_substitutions
                    })
            });
            if declarations.next().is_none() || declarations.next().is_some() {
                return Err("guarded evidence source changed its exact callee declaration");
            }
            let result = operation
                .result
                .structural()
                .ok_or("guarded evidence source has no structural result")?;
            if row.validity.result != result.place
                || row.guard.result_type != result.structural_type
                || row
                    .validity
                    .proposition_dependencies
                    .iter()
                    .chain(&row.validity.interface_dependencies)
                    .any(|dependency| *dependency != result.place)
            {
                return Err("guarded evidence source lost its exact result validity cut");
            }
            let runtime = consumer
                .runtime_call
                .ok_or("ordinary evidence consumer has no runtime operation")?;
            let use_operation = caller
                .invocation(runtime.site)
                .ok_or("ordinary evidence consumer operation is absent")?;
            let use_position = position(caller, use_operation)?;
            let source_position = position(caller, operation)?;
            crate::descriptors::descriptor_results::require_unchanged_produced_payload(
                module.structural_types,
                graph,
                caller.id,
                source_position,
                use_position,
                &DescriptorLocation::whole(result.place),
            )?;
            let mut analysis = DescriptorAnalysis::new(module.structural_types, graph);
            if !analysis.case_is_established(
                caller.id,
                use_position,
                &DescriptorCasePredicate {
                    subject: DescriptorLocation::whole(result.place),
                    case: row.guard.result_case,
                },
            ) {
                return Err("guarded evidence input has no current selected result case");
            }
            let total_uses = row
                .uses
                .len()
                .checked_add(
                    module
                        .proof_output_calls
                        .iter()
                        .filter(|invocation| {
                            invocation.caller == caller.id && invocation.runtime_call.is_some()
                        })
                        .map(|invocation| {
                            invocation
                                .evidence_arguments
                                .iter()
                                .filter(|argument| argument.source == source)
                                .count()
                        })
                        .sum::<usize>(),
                )
                .ok_or("guarded evidence use cardinality overflow")?;
            if u32::try_from(total_uses).ok() != Some(row.expected_use_count) {
                return Err("guarded evidence input omitted a selected-term use");
            }
            let declared = application(module, row.callee_proposition)?;
            same_head(declared, source_application)?;
            let mut subjects = Vec::with_capacity(declared.arguments.len());
            for index in 0..declared.arguments.len() {
                let argument_position =
                    u32::try_from(index).map_err(|_| "guarded evidence argument exceeds u32")?;
                let mut parameters = row
                    .argument_substitutions
                    .iter()
                    .filter(|subject| subject.argument_position == argument_position);
                if let Some(parameter) = parameters.next() {
                    if parameters.next().is_some()
                        || row
                            .result_substitution
                            .is_some_and(|result| result.argument_position == argument_position)
                    {
                        return Err("guarded evidence source has duplicate typed subjects");
                    }
                    subjects.push(EvidenceArgumentSubject::Parameter(parameter.clone()));
                } else if row
                    .result_substitution
                    .is_some_and(|result| result.argument_position == argument_position)
                {
                    subjects.push(EvidenceArgumentSubject::Result { argument_position });
                } else {
                    if declared.arguments[index] != source_application.arguments[index] {
                        return Err("guarded evidence changed an unrepresented closed subject");
                    }
                    subjects.push(EvidenceArgumentSubject::Closed { argument_position });
                }
            }
            let arguments = observe_arguments(
                module,
                caller,
                callee,
                Some((operation, parts)),
                source_position,
                &subjects,
                &source_application.arguments,
                graph,
                history,
            )?;
            return Ok((source_term.proposition, arguments, Some(source_position)));
        }
        let mut producers = module.proof_output_calls.iter().filter_map(|invocation| {
            if invocation.caller != caller.id {
                return None;
            }
            let mut outputs = invocation
                .outputs
                .iter()
                .filter(|output| output.output == Some(source));
            let output = outputs.next()?;
            Some((invocation, output, outputs.next().is_some()))
        });
        let (producer, output, duplicate) = producers
            .next()
            .ok_or("runtime evidence source has no available producer")?;
        if duplicate
            || producers.next().is_some()
            || producer.static_requirement_dispatch.is_some()
            || output.instantiated_proposition != source_term.proposition
            || producer.ordinal >= consumer.ordinal
        {
            return Err("runtime evidence source has ambiguous or unavailable output scope");
        }
        let runtime = producer
            .runtime_call
            .ok_or("erased output has no retained runtime snapshot for this input")?;
        let operation = caller
            .invocation(runtime.site)
            .ok_or("evidence output producer operation is absent")?;
        let parts = operation.parts();
        if parts.callee != runtime.callee {
            return Err("evidence output runtime target changed");
        }
        let callee = module
            .machine(parts.callee)
            .ok_or("evidence output callee is absent")?;
        let mut lanes = module.evidence_contract_lanes.iter().filter(|lane| {
            lane.machine == parts.callee
                && lane.kind == EvidenceContractLaneKind::Ensures
                && lane.position == output.output_position
                && lane.output_field.as_deref() == Some(output.output_field.as_str())
        });
        let lane = lanes
            .next()
            .ok_or("evidence output changed its exact callee lane")?;
        if lanes.next().is_some()
            || term(module, lane.term)?.proposition != output.callee_proposition
        {
            return Err("evidence output declaration is ambiguous or substituted");
        }
        if let Some(input_position) = output.forwarded_input_position {
            let inputs = input_lanes(module, parts.callee);
            let input = inputs
                .get(input_position as usize)
                .ok_or("forwarded evidence output has no declared input")?;
            if output.callee_output.is_some()
                || input.argument_subjects != lane.argument_subjects
                || term(module, input.term)?.proposition != term(module, lane.term)?.proposition
                || term(module, input.term)?.interface != term(module, lane.term)?.interface
            {
                return Err("forwarded evidence output changed its exact declared input meaning");
            }
        } else if output.callee_output != Some(lane.term) {
            return Err("evidence output has no exact callee producer term");
        }
        same_head(
            application(module, output.callee_proposition)?,
            source_application,
        )?;
        let source_position = position(caller, operation)?;
        let arguments = observe_arguments(
            module,
            caller,
            callee,
            Some((operation, parts)),
            source_position,
            &lane.argument_subjects,
            &source_application.arguments,
            graph,
            history,
        )?;
        Ok((source_term.proposition, arguments, Some(source_position)))
    }

    fn require_completed_capture(
        graph: &[DescriptorMachine],
        machine: MachineId,
        source: DescriptorPosition,
        use_position: DescriptorPosition,
        values: &[ValueId],
    ) -> Result<(), &'static str> {
        descriptor_results::require_completed_scalar_capture(
            graph,
            machine,
            source,
            use_position,
            values,
        )
    }
}
pub(crate) mod invocation_inputs {

    use semantic_vocabulary::MachineId;
    use terminal_psi::{EvidenceContractLaneKind, TerminalModule};

    pub fn validate_empty_erased_invocation_inputs(
        module: &TerminalModule,
        target: MachineId,
    ) -> Result<(), &'static str> {
        let mut targets = module
            .machines
            .iter()
            .filter(|machine| machine.id == target);
        let machine = targets
            .next()
            .ok_or("empty erased invocation target is absent")?;
        if targets.next().is_some() {
            return Err("empty erased invocation target is ambiguous");
        }
        if module
            .evidence_contract_lanes
            .iter()
            .any(|lane| lane.machine == target && lane.kind == EvidenceContractLaneKind::Requires)
        {
            return Err("empty erased actual lane cannot supply declared requires evidence");
        }
        if !machine.contract.erased_scalar_formals.is_empty()
            || !machine.contract.erased_proof_formals.is_empty()
        {
            return Err("empty erased actual lane cannot supply declared proof formals");
        }
        let mut entries = machine
            .blocks
            .iter()
            .filter(|block| block.id == machine.entry);
        let entry = entries
            .next()
            .ok_or("empty erased invocation has no exact entry block")?;
        if entries.next().is_some() {
            return Err("empty erased invocation entry block is ambiguous");
        }
        if !entry.erased_scalar_formals.is_empty() || !entry.erased_proof_formals.is_empty() {
            return Err("empty erased actual lane cannot supply entry proof formals");
        }
        Ok(())
    }
}

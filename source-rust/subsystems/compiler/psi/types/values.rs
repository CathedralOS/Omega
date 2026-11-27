pub(crate) mod places {

    use crate::TerminalIdentityIndex;
    use crate::{OperationKind, StructuralAccess};
    use semantic_vocabulary::{
        ObligationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId,
        StructuralPlaceKind, StructuralTypeId, ValueId,
    };

    /// A captured storage path keeps resolved field identities, selector
    /// values and changes of presentation together across analyses.
    /// Unlike a portable proof path, it can name machine-local selectors.
    /// Different paths may still address the same cell; equality alone
    /// does not establish disjoint storage.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ResolvedStructuralPathSegment {
        /// ownership.md: "Record segments use the exact field identity".
        Field(StructuralFieldId),
        /// ownership.md: "A Case segment guards its payload namespace; it adds
        /// neither physical storage nor an independent claim."
        Case(StructuralCaseId),
        /// ownership.md: "Fixed-array segments use canonical zero-based literal
        /// indexes".
        FixedIndex(u64),
        /// An element selected at run time, named by the selector value captured
        /// when the place was formed. `wiki/spec/terminal-psi/loans.md`: "Loan
        /// formation captures a place, not a live selector expression." Because a
        /// value never changes, a later write to the selector's source names a
        /// different value and therefore a different place.
        RuntimeIndex { selector: ValueId },
        /// The descriptor's current extent, a scalar observation of its
        /// contents. This is metadata, not a physical field or another owner.
        /// byte_views.md: "a validated length observation of a machine or block
        /// parameter equals that exact parameter's `ViewExtent`".
        /// Replacing any ancestor descriptor replaces this observation's
        /// reaching content, just as a store replaces a scalar field's.
        Extent,
        /// Present the same storage through a checked shape. This step retains
        /// its physical scalar codec; it neither moves bytes nor adds access.
        RepresentationView {
            structural_type: StructuralTypeId,
            stored_scalar: Option<ScalarType>,
        },
        /// Cross a live permission descriptor into its separately owned backing.
        /// It is never an inline field, a claim transfer or a disjointness fact.
        Referent,
        /// The opaque contents packet of a type parameter. This logical
        /// location can move with its value but is never a captured address.
        UnknownContents,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
    pub enum StructuralPathSegment {
        AllElements,
        Case(StructuralCaseId),
        Field(TerminalIdentityIndex),
        FixedIndex(u64),
        FixedRange {
            start: u64,
            end: u64,
        },
        RuntimeIndex {
            index: ValueId,
            obligation: ObligationId,
        },
        RepresentationView {
            structural_type: StructuralTypeId,
            stored_scalar: Option<ScalarType>,
        },
        Referent,
    }

    impl Default for StructuralPathSegment {
        fn default() -> Self {
            Self::Field(TerminalIdentityIndex::ABSENT)
        }
    }

    macro_rules! call_and_scalar_constant_operations {
        () => {
            Self::Call { .. }
                | Self::SelectImplementation { .. }
                | Self::SelectMachine { .. }
                | Self::PortWrite { .. }
                | Self::RegisterAccess { .. }
                | Self::IntegerConstant { .. }
                | Self::BooleanConstant { .. }
                | Self::IeeeFloatConstant { .. }
                | Self::IeeeFloatCompare { .. }
                | Self::ScalarRepresentationView { .. }
                | Self::NearestIeeeFloatFusedMultiplyAdd { .. }
        };
    }

    macro_rules! scalar_predicate_operations {
        () => {
            Self::BooleanNot { .. }
                | Self::BooleanEqual { .. }
                | Self::IntegerEqual { .. }
                | Self::IntegerLessThan { .. }
                | Self::IntegerLessOrEqual { .. }
                | Self::IntegerBitwiseNot { .. }
                | Self::IntegerWiden { .. }
                | Self::IntegerExactCast { .. }
                | Self::IntegerBitwiseAnd { .. }
                | Self::IntegerBitwiseOr { .. }
                | Self::IntegerBitwiseXor { .. }
                | Self::WrappingIntegerShiftLeft { .. }
                | Self::WrappingIntegerShiftRight { .. }
                | Self::ExactIntegerShiftLeft { .. }
                | Self::ExactIntegerShiftRight { .. }
                | Self::ExactIntegerAdd { .. }
                | Self::ExactIntegerSubtract { .. }
                | Self::ExactIntegerMultiply { .. }
                | Self::ExactIntegerDivide { .. }
                | Self::ExactIntegerRemainder { .. }
                | Self::WrappingIntegerDivide { .. }
                | Self::WrappingIntegerRemainder { .. }
                | Self::SaturatingIntegerDivide { .. }
                | Self::SaturatingIntegerRemainder { .. }
                | Self::WrappingIntegerAdd { .. }
                | Self::SaturatingIntegerAdd { .. }
                | Self::WrappingIntegerSubtract { .. }
                | Self::SaturatingIntegerSubtract { .. }
                | Self::WrappingIntegerMultiply { .. }
                | Self::SaturatingIntegerMultiply { .. }
                | Self::TrappingInteger { .. }
                | Self::FloatResultPolicy { .. }
        };
    }

    impl StructuralPathSegment {
        pub const fn runtime_index(&self) -> Option<(ValueId, ObligationId)> {
            match self {
                Self::RuntimeIndex { index, obligation } => Some((*index, *obligation)),
                Self::AllElements
                | Self::Case(_)
                | Self::Field(_)
                | Self::FixedIndex(_)
                | Self::FixedRange { .. }
                | Self::RepresentationView { .. }
                | Self::Referent => None,
            }
        }
    }

    /// A declared lifetime names static leaves or a whole family of elements.
    pub fn declared_lifetime_path_is_static(path: &[StructuralPathSegment]) -> bool {
        path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Referent
                    | StructuralPathSegment::AllElements
            )
        })
    }

    pub fn is_static_structural_path(path: &[StructuralPathSegment]) -> bool {
        path.iter().all(|segment| {
            !matches!(segment, StructuralPathSegment::AllElements)
                && segment.runtime_index().is_none()
        })
    }

    pub fn is_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
        path.iter().all(|segment| match segment {
            StructuralPathSegment::Field(identity) => !identity.is_absent(),
            StructuralPathSegment::Case(_) => true,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
                true
            }
            StructuralPathSegment::AllElements
            | StructuralPathSegment::FixedRange { .. }
            | StructuralPathSegment::RepresentationView { .. }
            | StructuralPathSegment::Referent => false,
        })
    }

    pub fn is_static_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
        is_static_structural_path(path) && is_structural_scalar_store_path(path)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralPlaceDeclaration {
        pub id: PlaceId,
        pub kind: StructuralPlaceKind,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OperationProjection<'a> {
        pub root: PlaceId,
        pub path: &'a [StructuralPathSegment],
        /// The final canonical field, when the instruction keeps it outside
        /// its authored path. Operand consumers must not lose this selector.
        pub field: Option<StructuralFieldId>,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum StructuralStorageUse<'a> {
        Place(PlaceId),
        Projection(OperationProjection<'a>),
        Canonical {
            root: PlaceId,
            path: &'a [semantic_vocabulary::CanonicalStructuralPathSegment],
        },
        ObservedField {
            root: PlaceId,
            path: &'a [semantic_vocabulary::CanonicalStructuralPathSegment],
            field: StructuralFieldId,
        },
        Element {
            descriptor: OperationProjection<'a>,
            path: &'a [StructuralPathSegment],
        },
        Type(StructuralTypeId),
    }

    impl<'a> From<OperationProjection<'a>> for StructuralStorageUse<'a> {
        fn from(projection: OperationProjection<'a>) -> Self {
            Self::Projection(projection)
        }
    }

    impl OperationKind {
        pub fn map_structural_uses(&mut self, map: &mut impl FnMut(PlaceId) -> PlaceId) {
            self.map_structural_uses_with_access(&mut |place, _| map(place));
        }

        pub fn map_structural_uses_with_access(
            &mut self,
            map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
        ) {
            match self {
                Self::ActivateTaskRegister {
                    gdt,
                    task_state,
                    scratch,
                    ..
                } => {
                    for source in [gdt, task_state, scratch] {
                        source.place = map(source.place, source.access);
                    }
                }
                Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                    for source in [gdt, scratch] {
                        source.place = map(source.place, source.access);
                    }
                }
                Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
                Self::EstablishNativeCodeLayout { .. }
                | Self::EstablishActivationClaim { .. } => {}
                Self::TransformContent {
                    source,
                    parents,
                    derivations,
                } => {
                    let mut bindings = Vec::new();
                    for argument in std::iter::once(source).chain(parents) {
                        let original = argument.place;
                        argument.place = map(original, argument.access);
                        bindings.push((original, argument.place));
                    }
                    for derived in derivations.iter_mut() {
                        derived.map_places(&mut |place| {
                            bindings
                                .iter()
                                .find(|(source, _)| *source == place)
                                .map_or(place, |(_, target)| *target)
                        });
                    }
                    derivations.sort();
                }
                Self::ReleaseActivationClaim { source, .. }
                | Self::SplitActivationClaim { source, .. }
                | Self::StorageAddress { source }
                | Self::StorageAlignment { source }
                | Self::GlobalDescriptorTable { source, .. }
                | Self::MoveStructuralValue { source }
                | Self::PackedIntegerUnary { source, .. }
                | Self::DecomposeStructuralValue { subject: source }
                | Self::EstablishAccessReference { source, .. }
                | Self::EstablishReference { source, .. }
                | Self::EstablishElementView { source, .. }
                | Self::EstablishMutableElementView { source, .. } => {
                    source.place = map(source.place, source.access);
                }
                Self::MergeActivationClaim { lower, upper, .. }
                | Self::PackedIntegerBinary { left: lower, right: upper, .. } => {
                    lower.place = map(lower.place, lower.access);
                    upper.place = map(upper.place, upper.access);
                }
                Self::ElementViewLength { source }
                | Self::ElementViewRead { source, .. }
                | Self::ByteSequenceLength { source } => {
                    source.place = map(source.place, StructuralAccess::SharedBorrow);
                }
                Self::ReleaseReference { source, .. } => {
                    *source = map(*source, StructuralAccess::Owned);
                }
                Self::MoveStructuralField { source, .. } => {
                    *source = map(*source, StructuralAccess::MutableBorrow);
                }
                Self::ByteSequenceSubslice { source, .. }
                | Self::ElementViewSubslice { source, .. }
                | Self::ElementViewFixedArray { source, .. } => {
                    source.place = map(source.place, source.access);
                }
                Self::PrimitiveScalarRead { source, .. }
                | Self::StructuralByteSequenceFieldLength { source, .. }
                | Self::StructuralByteSequenceFieldRead { source, .. }
                | Self::StructuralCaseMembership { source, .. }
                | Self::StructuralLeafCopy { source, .. }
                | Self::StructuralCaseLeafCopy { source, .. }
                | Self::ByteSequenceRead { source, .. }
                | Self::BooleanStructuralField { source, .. }
                | Self::ScalarStructuralField { source, .. } => {
                    *source = map(*source, StructuralAccess::SharedBorrow)
                }
                Self::StructuralByteSequenceFieldByteStore { destination, .. }
                | Self::WriteOnlyPrimitiveStore { destination, .. }
                | Self::StructuralScalarFieldStore { destination, .. }
                | Self::ByteSequenceWrite { destination, .. } => {
                    *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                }
                Self::ElementViewWrite { destination, .. } => {
                    destination.place = map(destination.place, destination.access);
                }
                Self::ByteSequenceStore {
                    destination,
                    source,
                    ..
                }
                | Self::StructuralByteSequenceFieldStore {
                    destination,
                    source,
                    ..
                }
                | Self::StructuralByteViewFieldStore {
                    destination,
                    source,
                    ..
                } => {
                    *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                    *source = map(*source, StructuralAccess::SharedBorrow);
                }
                Self::StoreStructuralField {
                    destination, value, ..
                } => {
                    *destination = map(*destination, StructuralAccess::WriteOnlyBorrow);
                    value.place = map(value.place, value.access);
                }
                Self::EstablishByteSequenceConcatenation { left, right, .. }
                | Self::ByteSequenceEqual { left, right } => {
                    for operand in [left, right] {
                        map_byte_sequence_operand(operand, map);
                    }
                }
                Self::ByteSequencePredicate { operand, .. } => {
                    map_byte_sequence_operand(operand, map);
                }
                Self::EstablishStructuralArray { elements } => {
                    for argument in elements {
                        argument.place = map(argument.place, argument.access);
                    }
                }
                Self::EstablishRecord { fields }
                | Self::EstablishStructuralCase { fields, .. } => {
                    for field in fields {
                        match &mut field.value {
                            crate::RecordFieldValue::Structural(value) => {
                                value.place = map(value.place, value.access);
                            }
                            crate::RecordFieldValue::Scalar { .. } => {}
                        }
                    }
                }
                crate::call_structural_argument_variants!(structural_arguments) => {
                    for argument in structural_arguments {
                        argument.place = map(argument.place, argument.access);
                    }
                }
                Self::AtomicAccess { place, event, .. } => {
                    let access = match (event.observes_resident(), event.modifies_resident()) {
                        (true, true) => StructuralAccess::MutableBorrow,
                        (false, true) => StructuralAccess::WriteOnlyBorrow,
                        (_, false) => StructuralAccess::SharedBorrow,
                    };
                    *place = map(*place, access);
                }
                Self::EstablishScalarArray { .. }
                | Self::EstablishZeroedStructural { .. }
                | Self::EstablishStructuralLocal { .. }
                | Self::EstablishPrimitiveLocal { .. }
                | Self::EstablishScalarCase { .. }
                | Self::EstablishByteSequenceLiteral { .. }
                | Self::EstablishTrivialAffineLocal { .. }
                | Self::AtomicFence { .. }
                | call_and_scalar_constant_operations!()
                | Self::ReadScalarApplication { .. }
                | scalar_predicate_operations!() => {}
            }
        }

        pub fn structural_projections(&self) -> Vec<OperationProjection<'_>> {
            let mut projections = Vec::new();
            for (storage, _) in self.storage_uses() {
                match storage {
                    StructuralStorageUse::Projection(projection) => projections.push(projection),
                    StructuralStorageUse::Element { descriptor, .. } => {
                        projections.push(descriptor);
                    }
                    StructuralStorageUse::Place(_)
                    | StructuralStorageUse::Canonical { .. }
                    | StructuralStorageUse::ObservedField { .. }
                    | StructuralStorageUse::Type(_) => {}
                }
            }
            projections
        }

        /// Keep content effects with the same operands used for ownership
        /// and layout checks. Transporting a descriptor does not read its
        /// pointee; calls obtain delegated effects from their contracts.
        pub fn storage_uses(
            &self,
        ) -> Vec<(
            StructuralStorageUse<'_>,
            Option<access_vocabulary::AccessOperation>,
        )> {
            self.storage_uses_with_arguments(|_| true)
        }

        /// A geometry consumer can omit an argument whose existing pointer
        /// is carried unchanged. All storage reads, writes and projections
        /// outside simple call-argument transport remain visible.
        pub fn storage_uses_with_arguments(
            &self,
            include: impl Fn(&crate::StructuralArgument) -> bool,
        ) -> Vec<(
            StructuralStorageUse<'_>,
            Option<access_vocabulary::AccessOperation>,
        )> {
            use access_vocabulary::AccessOperation::CompoundMutation;
            use access_vocabulary::AccessOperation::Read;
            use access_vocabulary::AccessOperation::Take;
            use access_vocabulary::AccessOperation::Write;
            let mut uses = Vec::new();
            let mut push = |storage, operation| uses.push((storage, operation));
            match self {
                Self::ActivateTaskRegister {
                    gdt,
                    task_state,
                    scratch,
                    ..
                } => {
                    for source in [gdt, task_state, scratch] {
                        push(argument_projection(source).into(), Some(CompoundMutation));
                    }
                }
                Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                    for source in [gdt, scratch] {
                        push(argument_projection(source).into(), Some(CompoundMutation));
                    }
                }
                Self::ReadTaskRegister { .. }
                | Self::ReadSegmentSelector { .. }
                | Self::EstablishNativeCodeLayout { .. } => {}
                Self::EstablishActivationClaim { backing_type, .. } => {
                    push(StructuralStorageUse::Type(*backing_type), None);
                }
                Self::EstablishByteSequenceConcatenation { left, right, .. }
                | Self::ByteSequenceEqual { left, right } => {
                    for operand in [left, right] {
                        push(operand.storage_projection().into(), Some(Read));
                    }
                }
                Self::ByteSequencePredicate { operand, .. } => {
                    push(operand.storage_projection().into(), Some(Read));
                }
                Self::ReleaseActivationClaim { source, .. }
                | Self::SplitActivationClaim { source, .. } => {
                    push(argument_projection(source).into(), Some(CompoundMutation));
                }
                Self::StorageAddress { source }
                | Self::StorageAlignment { source }
                | Self::EstablishAccessReference { source, .. }
                | Self::EstablishReference { source, .. } => {
                    push(argument_projection(source).into(), None);
                }
                Self::MoveStructuralValue { source }
                | Self::DecomposeStructuralValue { subject: source } => {
                    push(
                        argument_projection(source).into(),
                        projected_transfer_operation(source),
                    );
                }
                Self::GlobalDescriptorTable {
                    operation, source, ..
                } => {
                    push(
                        argument_projection(source).into(),
                        Some(if operation.writes_operand() {
                            Write
                        } else {
                            Read
                        }),
                    );
                }
                Self::ElementViewLength { source }
                | Self::ByteSequenceLength { source }
                | Self::ByteSequenceSubslice { source, .. }
                | Self::ElementViewSubslice { source, .. }
                | Self::ElementViewFixedArray { source, .. } => {
                    push(argument_projection(source).into(), Some(Read));
                }
                Self::EstablishElementView {
                    source, element, ..
                }
                | Self::EstablishMutableElementView {
                    source, element, ..
                } => {
                    push(argument_projection(source).into(), None);
                    push(StructuralStorageUse::Type(*element), None);
                }
                Self::PackedIntegerUnary { source, .. } => push(argument_projection(source).into(), Some(Read)),
                Self::PackedIntegerBinary { left, right, .. } => {
                    for source in [left, right] {
                        push(argument_projection(source).into(), Some(Read));
                    }
                }
                Self::MergeActivationClaim { lower, upper, .. } => {
                    for source in [lower, upper] {
                        push(argument_projection(source).into(), Some(CompoundMutation));
                    }
                }
                Self::TransformContent {
                    source, parents, ..
                } => {
                    push(argument_projection(source).into(), Some(CompoundMutation));
                    for parent in parents {
                        push(argument_projection(parent).into(), Some(Read));
                    }
                }
                Self::ElementViewWrite {
                    destination, path, ..
                } => {
                    push(
                        StructuralStorageUse::Element {
                            descriptor: argument_projection(destination),
                            path,
                        },
                        Some(Write),
                    );
                }
                Self::ElementViewRead { source, path, .. } => {
                    push(
                        StructuralStorageUse::Element {
                            descriptor: argument_projection(source),
                            path,
                        },
                        Some(Read),
                    );
                }
                Self::PrimitiveScalarRead { source, path }
                | Self::StructuralCaseMembership { source, path, .. }
                | Self::StructuralLeafCopy { source, path } => {
                    push(
                        OperationProjection {
                            root: *source,
                            path,
                            field: None,
                        }
                        .into(),
                        Some(Read),
                    );
                }
                Self::StructuralByteSequenceFieldLength {
                    source,
                    path,
                    field,
                }
                | Self::StructuralByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    ..
                } => {
                    push(
                        OperationProjection {
                            root: *source,
                            path,
                            field: Some(*field),
                        }
                        .into(),
                        Some(Read),
                    );
                }
                Self::ReleaseReference { source, path } => {
                    push(
                        OperationProjection {
                            root: *source,
                            path,
                            field: None,
                        }
                        .into(),
                        None,
                    );
                }
                Self::AtomicAccess {
                    place, path, event, ..
                } => {
                    let operation =
                        access_vocabulary::AccessOperation::Atomic(event.access_operation());
                    push(
                        OperationProjection {
                            root: *place,
                            path,
                            field: None,
                        }
                        .into(),
                        Some(operation),
                    );
                }
                Self::MoveStructuralField {
                    source,
                    path,
                    field,
                } => {
                    push(
                        OperationProjection {
                            root: *source,
                            path,
                            field: *field,
                        }
                        .into(),
                        (field.is_some() || !path.is_empty()).then_some(Take),
                    );
                }
                Self::WriteOnlyPrimitiveStore {
                    destination, path, ..
                } => {
                    push(
                        OperationProjection {
                            root: *destination,
                            path,
                            field: None,
                        }
                        .into(),
                        Some(Write),
                    );
                }
                Self::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    ..
                }
                | Self::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    push(
                        OperationProjection {
                            root: *destination,
                            path,
                            field: Some(*field),
                        }
                        .into(),
                        Some(Write),
                    );
                }
                Self::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    push(
                        OperationProjection {
                            root: *destination,
                            path,
                            field: Some(*field),
                        }
                        .into(),
                        Some(Write),
                    );
                    push(StructuralStorageUse::Place(*source), Some(Read));
                }
                Self::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    push(
                        OperationProjection {
                            root: *destination,
                            path,
                            field: *field,
                        }
                        .into(),
                        Some(Write),
                    );
                    push(StructuralStorageUse::Place(*source), None);
                }
                Self::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    push(
                        OperationProjection {
                            root: *destination,
                            path,
                            field: *field,
                        }
                        .into(),
                        Some(Write),
                    );
                    push(
                        argument_projection(value).into(),
                        projected_transfer_operation(value),
                    );
                }
                Self::EstablishStructuralArray { elements } => {
                    for element in elements {
                        push(
                            argument_projection(element).into(),
                            projected_transfer_operation(element),
                        );
                    }
                }
                Self::EstablishRecord { fields }
                | Self::EstablishStructuralCase { fields, .. } => {
                    for field in fields {
                        if let crate::RecordFieldValue::Structural(value) = &field.value {
                            push(
                                argument_projection(value).into(),
                                projected_transfer_operation(value),
                            );
                        }
                    }
                }
                crate::call_structural_argument_variants!(structural_arguments) => {
                    for argument in structural_arguments.iter().filter(|arg| include(arg)) {
                        push(
                            argument_projection(argument).into(),
                            projected_transfer_operation(argument),
                        );
                    }
                }
                Self::StructuralCaseLeafCopy { source, path } => {
                    push(
                        StructuralStorageUse::Canonical {
                            root: *source,
                            path,
                        },
                        Some(Read),
                    );
                }
                Self::BooleanStructuralField {
                    source,
                    path,
                    field,
                }
                | Self::ScalarStructuralField {
                    source,
                    path,
                    field,
                } => {
                    push(
                        StructuralStorageUse::ObservedField {
                            root: *source,
                            path,
                            field: *field,
                        },
                        Some(Read),
                    );
                }
                Self::ByteSequenceRead { source, .. } => {
                    push(StructuralStorageUse::Place(*source), Some(Read))
                }
                Self::ByteSequenceWrite { destination, .. } => {
                    push(StructuralStorageUse::Place(*destination), Some(Write))
                }
                Self::ByteSequenceStore {
                    destination,
                    source,
                    ..
                } => {
                    push(StructuralStorageUse::Place(*destination), Some(Write));
                    push(StructuralStorageUse::Place(*source), Some(Read));
                }
                Self::EstablishScalarArray { .. }
                | Self::EstablishZeroedStructural { .. }
                | Self::EstablishStructuralLocal { .. }
                | Self::EstablishPrimitiveLocal { .. }
                | Self::EstablishScalarCase { .. }
                | Self::EstablishByteSequenceLiteral { .. }
                | Self::EstablishTrivialAffineLocal { .. }
                | Self::AtomicFence { .. }
                | call_and_scalar_constant_operations!()
                | Self::ReadScalarApplication { .. }
                | scalar_predicate_operations!() => {}
            }
            uses
        }

        pub fn structural_projection_paths_mut(&mut self) -> Vec<&mut Vec<StructuralPathSegment>> {
            let mut paths = Vec::new();
            match self {
                Self::ActivateTaskRegister {
                    gdt,
                    task_state,
                    scratch,
                    ..
                } => {
                    for source in [gdt, task_state, scratch] {
                        paths.push(&mut source.path);
                    }
                }
                Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                    for source in [gdt, scratch] {
                        paths.push(&mut source.path);
                    }
                }
                Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
                Self::EstablishByteSequenceConcatenation { left, right, .. }
                | Self::ByteSequenceEqual { left, right } => {
                    for operand in [left, right] {
                        paths.push(&mut operand.argument_mut().path);
                    }
                }
                Self::ByteSequencePredicate { operand, .. } => {
                    paths.push(&mut operand.argument_mut().path);
                }
                Self::ReleaseActivationClaim { source, .. }
                | Self::SplitActivationClaim { source, .. }
                | Self::StorageAddress { source }
                | Self::StorageAlignment { source }
                | Self::GlobalDescriptorTable { source, .. }
                | Self::MoveStructuralValue { source }
                | Self::PackedIntegerUnary { source, .. }
                | Self::DecomposeStructuralValue { subject: source }
                | Self::EstablishAccessReference { source, .. }
                | Self::EstablishReference { source, .. }
                | Self::EstablishElementView { source, .. }
                | Self::EstablishMutableElementView { source, .. }
                | Self::ElementViewLength { source }
                | Self::ElementViewRead { source, .. }
                | Self::ByteSequenceLength { source }
                | Self::ByteSequenceSubslice { source, .. }
                | Self::ElementViewSubslice { source, .. }
                | Self::ElementViewFixedArray { source, .. } => {
                    paths.push(&mut source.path);
                }
                Self::MergeActivationClaim { lower, upper, .. }
                | Self::PackedIntegerBinary { left: lower, right: upper, .. } => {
                    paths.push(&mut lower.path);
                    paths.push(&mut upper.path);
                }
                Self::TransformContent {
                    source, parents, ..
                } => {
                    paths.push(&mut source.path);
                    paths.extend(parents.iter_mut().map(|parent| &mut parent.path));
                }
                Self::ElementViewWrite { destination, .. } => {
                    paths.push(&mut destination.path);
                }
                Self::ReleaseReference { path, .. }
                | Self::PrimitiveScalarRead { path, .. }
                | Self::AtomicAccess { path, .. }
                | Self::WriteOnlyPrimitiveStore { path, .. }
                | Self::StructuralByteSequenceFieldLength { path, .. }
                | Self::StructuralByteSequenceFieldRead { path, .. }
                | Self::StructuralCaseMembership { path, .. }
                | Self::StructuralLeafCopy { path, .. }
                | Self::MoveStructuralField { path, .. }
                | Self::StructuralByteSequenceFieldByteStore { path, .. }
                | Self::StructuralByteSequenceFieldStore { path, .. }
                | Self::StructuralByteViewFieldStore { path, .. }
                | Self::StructuralScalarFieldStore { path, .. } => paths.push(path),
                Self::StoreStructuralField { path, value, .. } => {
                    paths.push(path);
                    paths.push(&mut value.path);
                }
                Self::EstablishStructuralArray { elements } => {
                    paths.extend(elements.iter_mut().map(|argument| &mut argument.path));
                }
                Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                    paths.extend(
                        fields
                            .iter_mut()
                            .filter_map(|field| match &mut field.value {
                                crate::RecordFieldValue::Structural(value) => Some(&mut value.path),
                                crate::RecordFieldValue::Scalar { .. } => None,
                            }),
                    );
                }
                crate::call_structural_argument_variants!(structural_arguments) => paths.extend(
                    structural_arguments
                        .iter_mut()
                        .map(|argument| &mut argument.path),
                ),
                _ => {}
            }
            paths
        }

        pub fn runtime_indexes(&self) -> Vec<(ValueId, ObligationId)> {
            self.structural_projections()
                .iter()
                .flat_map(|projection| projection.path)
                .filter_map(StructuralPathSegment::runtime_index)
                .collect()
        }
    }

    fn projected_transfer_operation(
        argument: &crate::StructuralArgument,
    ) -> Option<access_vocabulary::AccessOperation> {
        (argument.access == StructuralAccess::Owned && !argument.path.is_empty())
            .then_some(access_vocabulary::AccessOperation::Take)
    }

    fn argument_projection(argument: &crate::StructuralArgument) -> OperationProjection<'_> {
        OperationProjection {
            field: None,
            root: argument.place,
            path: &argument.path,
        }
    }

    fn map_byte_sequence_operand(
        operand: &mut crate::control_flow::ByteSequenceOperand,
        map: &mut impl FnMut(PlaceId, StructuralAccess) -> PlaceId,
    ) {
        let source = operand.argument_mut();
        source.place = map(source.place, source.access);
    }

    #[cfg(test)]
    mod tests {
        use super::{
            StructuralPathSegment, is_static_structural_scalar_store_path,
            is_structural_scalar_store_path,
        };
        use crate::{
            OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralAccess,
            StructuralArgument,
        };
        use semantic_vocabulary::{
            MachineId, ObligationId, PlaceId, ScalarTerm, StructuralFieldId, StructuralTypeId,
            ValueId,
        };

        fn place(identity: u64) -> PlaceId {
            PlaceId::new(identity).unwrap()
        }

        fn argument(identity: u64) -> StructuralArgument {
            StructuralArgument {
                place: place(identity),
                path: vec![runtime()],
                access: StructuralAccess::SharedBorrow,
            }
        }

        #[test]
        fn storage_uses_keep_field_effects_separate_from_descriptor_transport() {
            use crate::{AtomicAccessEvent, MemoryOrdering, StructuralStorageUse};
            use access_vocabulary::AccessOperation::Atomic;
            use access_vocabulary::AccessOperation::Read;
            use access_vocabulary::AccessOperation::Take;
            use access_vocabulary::AccessOperation::Write;
            use access_vocabulary::AtomicAccessOperation;
            let field = StructuralFieldId::new(7).unwrap();
            let read = OperationKind::StructuralByteSequenceFieldLength {
                source: place(1),
                path: Vec::new(),
                field,
            };
            let uses = read.storage_uses();
            let StructuralStorageUse::Projection(projection) = uses[0].0 else {
                panic!("the same operand roster retains the field projection")
            };
            assert_eq!(projection.root, place(1));
            assert_eq!(projection.field, Some(field));
            assert_eq!(uses[0].1, Some(Read));
            let scalar = OperationKind::ScalarStructuralField {
                source: place(1),
                path: Vec::new(),
                field,
            };
            let uses = scalar.storage_uses();
            assert!(
                matches!(uses[0].0, StructuralStorageUse::ObservedField { field: actual, .. } if actual == field)
            );
            assert_eq!(uses[0].1, Some(Read));
            let source = StructuralArgument {
                place: place(1),
                path: vec![StructuralPathSegment::Referent],
                access: StructuralAccess::SharedBorrow,
            };
            let borrow = OperationKind::EstablishReference {
                source: source.clone(),
                storage: None,
            };
            assert_eq!(
                borrow.storage_uses()[0].1,
                None,
                "forming a reference does not inspect its pointee"
            );
            let read = OperationKind::PrimitiveScalarRead {
                source: place(1),
                path: source.path,
            };
            assert_eq!(read.storage_uses()[0].1, Some(Read));
            for (event, expected) in [
                (
                    AtomicAccessEvent::Load {
                        ordering: MemoryOrdering::Receive,
                    },
                    AtomicAccessOperation::Load(MemoryOrdering::Receive),
                ),
                (
                    AtomicAccessEvent::Store {
                        value: ValueId::new(1).unwrap(),
                        ordering: MemoryOrdering::Publish,
                    },
                    AtomicAccessOperation::Store(MemoryOrdering::Publish),
                ),
                (
                    AtomicAccessEvent::Swap {
                        value: ValueId::new(1).unwrap(),
                        ordering: MemoryOrdering::GlobalOrder,
                    },
                    AtomicAccessOperation::Swap(MemoryOrdering::GlobalOrder),
                ),
            ] {
                let access = OperationKind::AtomicAccess {
                    place: place(1),
                    path: Vec::new(),
                    event,
                    resident_obligation: None,
                };
                assert_eq!(access.storage_uses()[0].1, Some(Atomic(expected)));
            }
            use crate::control_flow::ByteSequenceOperand;
            let bytes = ByteSequenceOperand::Field {
                source: StructuralArgument {
                    place: place(1),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                field,
                length: ValueId::new(1).unwrap(),
            };
            let equal = OperationKind::ByteSequenceEqual {
                left: bytes.clone(),
                right: bytes,
            };
            for (operand, operation) in equal.storage_uses() {
                let StructuralStorageUse::Projection(projection) = operand else {
                    panic!("both byte fields retain projections")
                };
                assert_eq!(projection.field, Some(field));
                assert_eq!(operation, Some(Read));
            }
            for (action, expected) in [
                (crate::GlobalDescriptorTableOperation::Store, Write),
                (crate::GlobalDescriptorTableOperation::Load, Read),
            ] {
                let operation = OperationKind::GlobalDescriptorTable {
                    service: semantic_vocabulary::ServiceId::new(1).unwrap(),
                    operation: action,
                    source: StructuralArgument {
                        place: place(1),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    },
                };
                assert_eq!(operation.storage_uses()[0].1, Some(expected));
            }
            let whole = OperationKind::MoveStructuralField {
                source: place(1),
                path: Vec::new(),
                field: None,
            };
            assert_eq!(
                whole.storage_uses()[0].1,
                None,
                "moving a whole value retains its restrictions"
            );
            let field = OperationKind::MoveStructuralField {
                source: place(1),
                path: Vec::new(),
                field: Some(field),
            };
            assert_eq!(field.storage_uses()[0].1, Some(Take));
        }

        #[test]
        fn owned_value_move_preserves_access_and_projected_source() {
            let path = vec![StructuralPathSegment::FixedIndex(3)];
            let mut operation = OperationKind::MoveStructuralValue {
                source: StructuralArgument {
                    place: place(1),
                    path: path.clone(),
                    access: StructuralAccess::Owned,
                },
            };
            let mut uses = Vec::new();
            operation.map_structural_uses_with_access(&mut |source, access| {
                uses.push((source, access));
                place(2)
            });
            assert_eq!(uses, vec![(place(1), StructuralAccess::Owned)]);
            let projections = operation.structural_projections();
            assert_eq!(projections.len(), 1);
            assert_eq!(projections[0].root, place(2));
            assert_eq!(projections[0].path, path.as_slice());
            assert_eq!(operation.structural_projection_paths_mut().len(), 1);
        }

        #[test]
        fn element_read_normalizes_only_the_descriptor_projection() {
            let mut operation = OperationKind::ElementViewRead {
                source: argument(1),
                index: ValueId::new(2).unwrap(),
                length: ValueId::new(3).unwrap(),
                obligation: ObligationId::new(2).unwrap(),
                path: vec![StructuralPathSegment::FixedIndex(1)],
            };
            let projections = operation.structural_projections();
            assert_eq!(projections.len(), 1);
            assert_eq!(projections[0].root, place(1));
            assert_eq!(projections[0].path, [runtime()]);

            let mut paths = operation.structural_projection_paths_mut();
            assert_eq!(paths.len(), 1);
            paths[0][0] = StructuralPathSegment::FixedIndex(0);
            let OperationKind::ElementViewRead { source, path, .. } = operation else {
                unreachable!("normalized element read");
            };
            assert_eq!(source.path, [StructuralPathSegment::FixedIndex(0)]);
            assert_eq!(path, [StructuralPathSegment::FixedIndex(1)]);
        }

        #[test]
        fn structural_use_mapping_preserves_scalar_and_proof_call_operands() {
            let scalar = ValueId::new(1).unwrap();
            let evidence =
                ScalarTerm::boolean_field(place(1), StructuralFieldId::new(1).unwrap());
            let mut operation = OperationKind::CallStructuralScalar {
                callee: MachineId::new(1).unwrap(),
                arguments: vec![scalar],
                erased_arguments: vec![evidence.clone()],
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![argument(1), argument(2)],
                claim_transfers: Vec::new(),
                requirement_obligations: vec![ObligationId::new(1).unwrap()],
                crash_continuations: Vec::new(),
            };
            let mut expected = operation.clone();
            if let OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } = &mut expected
            {
                structural_arguments[0].place = place(11);
                structural_arguments[1].place = place(12);
            }
            let mut visited = Vec::new();
            operation.map_structural_uses(&mut |source| {
                visited.push(source);
                place(source.get() + 10)
            });
            assert_eq!(visited, vec![place(1), place(2)]);
            assert_eq!(operation, expected);
        }

        #[test]
        fn structural_use_mapping_visits_nested_initializers_and_byte_operands() {
            use crate::control_flow::ByteSequenceOperand;

            let scalar = ValueId::new(1).unwrap();
            let mut record = OperationKind::EstablishRecord {
                fields: vec![
                    RecordFieldInitializer {
                        field: StructuralFieldId::new(1).unwrap(),
                        value: RecordFieldValue::Scalar {
                            value: scalar,
                            range_obligation: None,
                        },
                    },
                    RecordFieldInitializer {
                        field: StructuralFieldId::new(2).unwrap(),
                        value: RecordFieldValue::Structural(argument(2)),
                    },
                ],
            };
            let mut expected = record.clone();
            if let OperationKind::EstablishRecord { fields } = &mut expected
                && let RecordFieldValue::Structural(source) = &mut fields[1].value
            {
                source.place = place(12);
            }
            record.map_structural_uses(&mut |source| place(source.get() + 10));
            assert_eq!(record, expected);

            let mut concatenation = OperationKind::EstablishByteSequenceConcatenation {
                left: ByteSequenceOperand::Whole {
                    source: argument(1),
                    length: scalar,
                },
                right: ByteSequenceOperand::Field {
                    source: argument(2),
                    field: StructuralFieldId::new(1).unwrap(),
                    length: scalar,
                },
                capacity_obligation: ObligationId::new(1).unwrap(),
            };
            let mut expected = concatenation.clone();
            if let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
                &mut expected
            {
                *left = ByteSequenceOperand::Whole {
                    source: argument(11),
                    length: scalar,
                };
                if let ByteSequenceOperand::Field { source, .. } = right {
                    source.place = place(12);
                }
            }
            concatenation.map_structural_uses(&mut |source| place(source.get() + 10));
            assert_eq!(concatenation, expected);
        }

        #[test]
        fn structural_use_mapping_distinguishes_stores_from_establishments() {
            let mut store = OperationKind::StoreStructuralField {
                disposition: crate::StructuralStoreDisposition::Vacant,
                destination: place(1),
                path: vec![runtime()],
                field: StructuralFieldId::new(1),
                value: argument(2),
            };
            let mut expected = store.clone();
            if let OperationKind::StoreStructuralField {
                destination, value, ..
            } = &mut expected
            {
                *destination = place(11);
                value.place = place(12);
            }
            let mut visited = Vec::new();
            store.map_structural_uses(&mut |source| {
                visited.push(source);
                place(source.get() + 10)
            });
            assert_eq!(visited, vec![place(1), place(2)]);
            assert_eq!(store, expected);

            let mut view = OperationKind::EstablishMutableElementView {
                region: crate::ElementViewRegion::WholeCollection,
                destination: place(1),
                source: argument(2),
                element: StructuralTypeId::new(1).unwrap(),
            };
            let mut expected = view.clone();
            if let OperationKind::EstablishMutableElementView { source, .. } = &mut expected {
                source.place = place(12);
            }
            view.map_structural_uses(&mut |source| place(source.get() + 10));
            assert_eq!(view, expected);
            for mut declaration in [
                OperationKind::EstablishTrivialAffineLocal {
                    destination: place(1),
                },
                OperationKind::EstablishByteSequenceLiteral {
                    destination: place(1),
                    bytes: vec![7],
                    qualifications: Vec::new(),
                },
            ] {
                declaration
                    .map_structural_uses(&mut |_| panic!("a destination declaration is not a use"));
            }
        }

        fn field(identity: u32) -> StructuralPathSegment {
            StructuralPathSegment::Field(crate::TerminalIdentityIndex(identity))
        }

        fn runtime() -> StructuralPathSegment {
            StructuralPathSegment::RuntimeIndex {
                index: ValueId::new(1).unwrap(),
                obligation: ObligationId::new(1).unwrap(),
            }
        }

        #[test]
        fn field_store_carriers_compose_fields_and_elements_in_any_order() {
            for path in [
                vec![],
                vec![field(1), StructuralPathSegment::FixedIndex(2)],
                vec![field(1), runtime(), field(2)],
                vec![field(3), runtime(), StructuralPathSegment::FixedIndex(1)],
                vec![StructuralPathSegment::FixedIndex(0), field(4)],
            ] {
                assert!(is_structural_scalar_store_path(&path), "{path:?}");
            }
            for path in [
                vec![StructuralPathSegment::Referent],
                vec![field(1), StructuralPathSegment::Referent],
                vec![StructuralPathSegment::FixedRange { start: 0, end: 1 }],
                vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                    0,
                ))],
            ] {
                assert!(!is_structural_scalar_store_path(&path), "{path:?}");
            }
        }

        #[test]
        fn static_store_paths_compose_fields_and_literal_indices() {
            for path in [
                vec![],
                vec![field(1)],
                vec![field(2), field(3)],
                vec![StructuralPathSegment::FixedIndex(0)],
                vec![field(4), StructuralPathSegment::FixedIndex(2)],
                vec![
                    StructuralPathSegment::FixedIndex(0),
                    StructuralPathSegment::FixedIndex(1),
                ],
                vec![StructuralPathSegment::FixedIndex(0), field(5)],
                vec![field(4), StructuralPathSegment::FixedIndex(0), field(5)],
            ] {
                assert!(is_static_structural_scalar_store_path(&path), "{path:?}");
            }
            for path in [
                vec![field(4), runtime()],
                vec![StructuralPathSegment::Referent],
                vec![field(1), StructuralPathSegment::Referent],
                vec![StructuralPathSegment::Referent, field(1)],
                vec![StructuralPathSegment::Field(crate::TerminalIdentityIndex(
                    0,
                ))],
            ] {
                assert!(!is_static_structural_scalar_store_path(&path), "{path:?}");
            }
        }
    }
}
pub(crate) mod reference_origins {

    use super::{StructuralArgument, StructuralPathSegment};
    use crate::StructuralAccess;
    use semantic_vocabulary::PlaceId;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ReferenceOriginProjection {
        Exact(StructuralPathSegment),
        Element,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ReferenceOriginSource {
        pub place: PlaceId,
        pub path: Vec<ReferenceOriginProjection>,
        pub extent: semantic_vocabulary::ReferenceOriginExtent,
        pub access: StructuralAccess,
    }

    impl From<StructuralArgument> for ReferenceOriginSource {
        fn from(argument: StructuralArgument) -> Self {
            Self {
                place: argument.place,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: argument
                    .path
                    .into_iter()
                    .map(ReferenceOriginProjection::Exact)
                    .collect(),
                access: argument.access,
            }
        }
    }
}
pub(crate) mod signatures {
    use crate::TerminalIdentityIndex;
    use crate::{
        StructuralAccess, StructuralMultiplicity, StructuralPathQualification,
        StructuralPathSegment,
    };
    use semantic_vocabulary::{PlaceId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ValueDeclaration {
        pub id: ValueId,
        pub scalar_type: ScalarType,
        pub qualifications: semantic_vocabulary::ScalarQualificationSetId,
    }

    impl ValueDeclaration {
        pub const fn value_type(self) -> semantic_vocabulary::QualifiedScalarType {
            semantic_vocabulary::QualifiedScalarType {
                scalar_type: self.scalar_type,
                qualifications: self.qualifications,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProofFormalSource {
        ValueParameter(u32),
        GenericParameter(u32),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ErasedProofFormal {
        pub source: ProofFormalSource,
        pub type_identity: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalMachineResult {
        Unit,
        Scalar(ValueDeclaration),
        Structural(StructuralResultDeclaration),
    }

    impl TerminalMachineResult {
        pub const fn scalar(&self) -> Option<ValueDeclaration> {
            match self {
                Self::Scalar(result) => Some(*result),
                Self::Unit | Self::Structural(_) => None,
            }
        }

        pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
            match self {
                Self::Scalar(result) => Some(result),
                Self::Unit | Self::Structural(_) => None,
            }
        }

        record_storage::enum_variant_accessor!(structural, Structural; StructuralResultDeclaration; Self::Unit | Self::Scalar(_));
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralParameterDeclaration {
        pub place: PlaceId,
        pub position: u32,
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub access: StructuralAccess,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<StructuralPathQualification>,
        pub additional_input_qualifications: Vec<StructuralPathQualification>,
    }

    impl StructuralParameterDeclaration {
        pub fn find_by_place(declarations: &[Self], place: PlaceId) -> Option<&Self> {
            declarations
                .iter()
                .find(|parameter| parameter.place == place)
        }

        pub fn is_unqualified_root(&self) -> bool {
            matches!(
                self.multiplicity,
                crate::nonlinear_multiplicity_pair_pattern!()
            ) && self.qualifications.is_empty()
                && self.projected_qualifications.is_empty()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralResultDeclaration {
        pub place: PlaceId,
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<StructuralPathQualification>,
        pub reference_sources: Vec<StructuralReferenceResultSource>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralReferenceResultSource {
        pub path: Vec<StructuralPathSegment>,
        pub sources: Vec<StructuralReferenceOrigin>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralReferenceOrigin {
        Parameter(super::ReferenceOriginSource),
        Invocation {
            carrier_type: semantic_vocabulary::StructuralTypeId,
            path: Vec<super::ReferenceOriginProjection>,
            extent: semantic_vocabulary::ReferenceOriginExtent,
            access: crate::StructuralAccess,
        },
    }

    impl StructuralReferenceOrigin {
        pub fn parameter(&self) -> Option<&super::ReferenceOriginSource> {
            match self {
                Self::Parameter(source) => Some(source),
                Self::Invocation { .. } => None,
            }
        }
        pub fn access(&self) -> crate::StructuralAccess {
            match self {
                Self::Parameter(source) => source.access,
                Self::Invocation { access, .. } => *access,
            }
        }
        pub fn path(&self) -> &[super::ReferenceOriginProjection] {
            match self {
                Self::Parameter(source) => &source.path,
                Self::Invocation { path, .. } => path,
            }
        }
        pub fn extent(&self) -> semantic_vocabulary::ReferenceOriginExtent {
            match self {
                Self::Parameter(source) => source.extent,
                Self::Invocation { extent, .. } => *extent,
            }
        }
    }

    impl From<super::ReferenceOriginSource> for StructuralReferenceOrigin {
        fn from(source: super::ReferenceOriginSource) -> Self {
            Self::Parameter(source)
        }
    }

    impl From<StructuralArgument> for StructuralReferenceOrigin {
        fn from(source: StructuralArgument) -> Self {
            Self::Parameter(source.into())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralArgument {
        pub place: PlaceId,
        pub path: Vec<StructuralPathSegment>,
        pub access: StructuralAccess,
    }

    impl StructuralArgument {
        pub fn whole(place: PlaceId, access: StructuralAccess) -> Self {
            Self {
                place,
                path: Vec::new(),
                access,
            }
        }
    }
}

pub use reference_origins::{ReferenceOriginProjection, ReferenceOriginSource};

pub use places::{
    OperationProjection, ResolvedStructuralPathSegment, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralStorageUse, declared_lifetime_path_is_static,
    is_static_structural_path, is_static_structural_scalar_store_path,
    is_structural_scalar_store_path,
};
pub use signatures::{
    ErasedProofFormal, ProofFormalSource, StructuralArgument, StructuralParameterDeclaration,
    StructuralReferenceOrigin, StructuralReferenceResultSource, StructuralResultDeclaration,
    TerminalMachineResult, ValueDeclaration,
};

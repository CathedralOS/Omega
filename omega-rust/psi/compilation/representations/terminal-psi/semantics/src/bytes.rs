/*
Semantic judgments for byte-sequence carriers, byte-view operations, and little-endian primitive
byte-window reads and stores.
*/

pub(crate) mod byte_carriers {

    use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    };

    pub struct ByteCarrierScalarLeaf {
        pub field: StructuralFieldId,
        pub offset: u64,
        pub scalar_type: ScalarType,
    }

    pub struct ByteCarrierLayout {
        pub leaves: Vec<ByteCarrierScalarLeaf>,
        pub size: u64,
    }

    fn leaf_scalar_carriers(field: &StructuralFieldDeclaration) -> Option<(u64, ScalarType)> {
        match field.field_type {
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => (integer.bits() % 8 == 0)
                .then(|| (u64::from(integer.bits() / 8), ScalarType::Integer(integer))),
            StructuralFieldType::Scalar(ScalarType::Boolean) => Some((1, ScalarType::Boolean)),
            StructuralFieldType::BoundedInteger(bounds) => {
                let integer = bounds.integer_type();
                (integer.bits() % 8 == 0)
                    .then(|| (u64::from(integer.bits() / 8), ScalarType::Integer(integer)))
            }
            _ => None,
        }
    }

    pub fn byte_carrier_scalar_layout<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<ByteCarrierLayout> {
        let declaration = types.clone().find(|item| item.id == root)?;
        let size = crate::reference_storage_size(types.clone(), root)?;
        let fields = declaration.shape.record_fields()?;
        let mut leaves = Vec::with_capacity(fields.len());
        for field in fields {
            if field.relevance.is_erased() {
                return None;
            }
            let (_, scalar_type) = leaf_scalar_carriers(field)?;
            let projection = crate::reference_storage_projection(
                types.clone(),
                root,
                &[terminal_psi::StructuralPathSegment::Field(field.identity)],
            )?;
            if projection.logical_scalar != scalar_type || projection.stored_scalar != scalar_type {
                return None;
            }
            leaves.push(ByteCarrierScalarLeaf {
                field: field.id,
                offset: projection.byte_offset,
                scalar_type,
            });
        }
        Some(ByteCarrierLayout { leaves, size })
    }
}
pub(crate) mod byte_sequence_concatenation {
    use crate::{OperationSemanticError, OperationSemanticTag, StructuralEffectObservation};
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm,
        ScalarType,
    };
    use terminal_psi::{Operation, OperationKind};

    pub fn byte_sequence_concatenation_capacity_goal(
        operation: &Operation,
        capacity: u64,
    ) -> Result<Proposition, OperationSemanticError> {
        let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
        else {
            return Err(OperationSemanticError::OperandShapeMismatch(
                OperationSemanticTag::EstablishByteSequenceConcatenation,
            ));
        };
        let source_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        let length = |operand: &terminal_psi::control_flow::ByteSequenceOperand| {
            IntegerMathTerm::MathValue {
                source_type,
                value: operand.length(),
            }
        };
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(Box::new(length(left)), Box::new(length(right))),
            IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(capacity))),
        ))
    }

    pub fn byte_sequence_concatenation_length_equation(
        observation: &StructuralEffectObservation,
        producer: &Operation,
    ) -> Result<Option<Proposition>, OperationSemanticError> {
        let StructuralEffectObservation::ByteSequenceLengthRead { source, result } = observation
        else {
            return Ok(None);
        };
        let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &producer.kind
        else {
            return Ok(None);
        };
        let Some(produced) = producer.result.structural() else {
            return Err(OperationSemanticError::ResultShapeMismatch(
                OperationSemanticTag::EstablishByteSequenceConcatenation,
            ));
        };
        if produced.place != source.place || !source.path.is_empty() {
            return Ok(None);
        }
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
        let sum = ScalarTerm::exact_integer_add(
            integer_type,
            value(left.length()),
            value(right.length()),
        )
        .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(Some(Proposition::Equal(value(*result), sum)))
    }
}
pub(crate) mod byte_sequence_predicates {

    use crate::UnchangedScalarOrigin;
    use crate::runtime_structural_path_tip;
    use semantic_vocabulary::StructuralTypeId;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
    use terminal_psi::Operation;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::call_structural_pair_pattern;
    use terminal_psi::{
        ByteSequencePredicate, ByteSequencePredicateSet, OperationKind, StructuralAccess,
        StructuralParameterDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
        live_field_by_id,
    };

    pub fn byte_predicate_requirement(
        required: ByteSequencePredicateSet,
        established: ByteSequencePredicateSet,
        root: PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> semantic_vocabulary::Proposition {
        use semantic_vocabulary::{Proposition, ScalarTerm};
        let mut clauses = Vec::new();
        for predicate in required.predicates() {
            let mut singleton = ByteSequencePredicateSet::EMPTY;
            singleton.insert(predicate);
            if !established.implies(singleton) {
                clauses.push(Proposition::Equal(
                    ScalarTerm::ByteSequencePredicate {
                        predicate,
                        root,
                        path: path.to_vec(),
                    },
                    ScalarTerm::Boolean(true),
                ));
            }
        }
        clauses.sort();
        match clauses.len() {
            0 => Proposition::Truth,
            1 => clauses.pop().expect("one predicate clause"),
            _ => Proposition::Conjunction(clauses),
        }
    }

    pub fn byte_field_root(
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<(StructuralTypeId, StructuralAccess)> {
        if machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == source)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == source)
        {
            return None;
        }
        let place = machine.structural_place(source)?;
        let parameter = match place.kind {
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
                crate::storage::arrival_storage::arrival_parameter(machine, source)?
            }
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let operation = machine.operation(producer)?;
                let result = operation.result.structural()?;
                return (result.place == source
                    && result.structural_type == structural_type
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.qualification_establishments.is_empty()
                    && result.claims.is_empty())
                .then_some((structural_type, StructuralAccess::Owned));
            }
            _ => return None,
        };
        (matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some((parameter.structural_type, parameter.access))
    }

    pub fn byte_parameter_predicates(
        module: &TerminalModule,
        parameter: &StructuralParameterDeclaration,
    ) -> Option<ByteSequencePredicateSet> {
        byte_parameter_predicates_from_domains(&module.structural_domains, parameter)
    }

    pub fn byte_parameter_predicates_from_domains(
        domains: &[terminal_psi::StructuralDomainDeclaration],
        parameter: &StructuralParameterDeclaration,
    ) -> Option<ByteSequencePredicateSet> {
        byte_parameter_predicates_in(domains.iter(), parameter)
    }

    pub fn byte_parameter_predicates_in<'domain>(
        domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
        parameter: &StructuralParameterDeclaration,
    ) -> Option<ByteSequencePredicateSet> {
        if !parameter.projected_qualifications.is_empty() {
            return None;
        }
        byte_qualification_predicates_in(
            domains,
            parameter.structural_type,
            &parameter.qualifications,
        )
    }

    pub fn byte_qualification_predicates_in<'domain>(
        domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
        carrier: StructuralTypeId,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
    ) -> Option<ByteSequencePredicateSet> {
        if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
            return None;
        }
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        for identity in qualifications {
            let domain = domains.clone().find(|domain| domain.id == *identity)?;
            if domain.carrier != carrier
                || domain.content_projection.is_some()
                || domain.establishment_routes.is_some()
                || !domain.has_no_required_memberships()
            {
                return None;
            }
            predicates.insert(domain.byte_predicate?);
        }
        Some(predicates)
    }

    pub fn byte_predicate_carrier<'types>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
    ) -> bool {
        let Some(declaration) = crate::structural_declaration(types.clone(), carrier) else {
            return false;
        };
        match declaration.shape {
            StructuralTypeShape::ByteSequence(_) => true,
            StructuralTypeShape::FixedArray { element, .. } => {
                crate::structural_declaration(types.clone(), element).is_some_and(|row| {
                    matches!(row.shape, StructuralTypeShape::PrimitiveScalar(
                        semantic_vocabulary::ScalarType::Integer(integer)
                    ) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == Some(8) && !integer.is_address())
                })
            }
            _ => false,
        }
    }

    pub fn pure_byte_qualifications<'types, 'domain>(
        types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        (whole.is_empty()
            || byte_predicate_carrier(types.clone(), root)
                && byte_qualification_predicates_in(domains.clone(), root, whole).is_some())
            && projected.iter().all(|row| {
                crate::qualification_path_carrier(types.clone(), root, &row.path).is_some_and(
                    |carrier| {
                        byte_predicate_carrier(types.clone(), carrier)
                            && byte_qualification_predicates_in(
                                domains.clone(),
                                carrier,
                                &[row.domain],
                            )
                            .is_some()
                    },
                )
            })
    }

    fn operation_producer(
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<(&Operation, StructuralTypeId)> {
        machine.producing_operation(source)
    }

    fn authenticated_call_result<'machine>(
        module: &TerminalModule,
        machine: &'machine TerminalMachine,
        source: PlaceId,
    ) -> Option<&'machine terminal_psi::StructuralOperationResult> {
        let (operation, structural_type) = operation_producer(machine, source)?;
        let callee = match operation.kind {
            call_structural_pair_pattern!(callee, ..) => callee,
            _ => return None,
        };
        let result = operation.result.structural()?;
        let signature = module.machine(callee)?.result.structural()?;
        (result.place == source
            && result.structural_type == structural_type
            && signature.structural_type == structural_type
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && signature.multiplicity == result.multiplicity
            && signature.qualifications == result.qualifications
            && signature.projected_qualifications.is_empty()
            && signature.reference_sources.is_empty()
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty())
        .then_some(result)
    }

    pub fn byte_owned_call_result<'machine>(
        module: &TerminalModule,
        machine: &'machine TerminalMachine,
        source: PlaceId,
    ) -> Option<&'machine terminal_psi::StructuralOperationResult> {
        let result = authenticated_call_result(module, machine, source)?;
        module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == result.structural_type
                    && declaration.shape.is_bounded_owned_byte_sequence()
            })
            .then_some(result)
    }

    pub fn byte_view_call_result<'machine>(
        module: &TerminalModule,
        machine: &'machine TerminalMachine,
        source: PlaceId,
    ) -> Option<&'machine terminal_psi::StructuralOperationResult> {
        let result = authenticated_call_result(module, machine, source)?;
        module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == result.structural_type
                    && declaration.shape.is_shared_readable_byte_view()
            })
            .then_some(result)
    }

    pub fn byte_owned_result_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<ByteSequencePredicateSet> {
        let declaration = machine.structural_place(source)?;
        let StructuralPlaceKind::OperationResult {
            structural_type, ..
        } = declaration.kind
        else {
            return None;
        };
        if !module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type && declaration.is_bounded_owned_byte_sequence()
        }) {
            return None;
        }
        byte_result_predicates(module, machine, source)
    }

    fn byte_result_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<ByteSequencePredicateSet> {
        let (operation, structural_type) = operation_producer(machine, source)?;
        let result = if matches!(operation.kind, call_structural_pair_pattern!(..)) {
            authenticated_call_result(module, machine, source)?
        } else {
            let result = operation.result.structural()?;
            (result.place == source
                && result.structural_type == structural_type
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty())
            .then_some(result)?
        };
        if !module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type && declaration.shape.is_owned_or_shared_byte_carrier()
        }) {
            return None;
        }
        byte_qualification_predicates_in(
            module.structural_domains.iter(),
            structural_type,
            &result.qualifications,
        )
    }

    pub fn shared_byte_field_copy_predicates<'a>(
        types: impl Iterator<Item = &'a terminal_psi::StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ByteSequencePredicateSet> {
        let (StructuralPathSegment::Field(identity), parent_path) = path.split_last()? else {
            return None;
        };
        let parent = runtime_structural_path_tip(types.clone(), root, parent_path)?;
        let fields = parent.shape.record_fields()?;
        let field = terminal_psi::live_field_by_identity(fields, *identity)?;
        if !field
            .field_type
            .byte_sequence_carrier()
            .is_some_and(|carrier| carrier.is_shared_readable_view())
        {
            return None;
        }
        Some(field.field_type.byte_predicates())
    }

    pub fn byte_sequence_concatenation_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Option<ByteSequencePredicateSet> {
        let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
        else {
            return None;
        };
        let mut resolve = |source| byte_sequence_source_predicates(module, machine, source);
        Some(
            concatenation_operand_predicates(module, machine, left, &mut resolve)?.concatenation(
                concatenation_operand_predicates(module, machine, right, &mut resolve)?,
            ),
        )
    }

    pub fn byte_copied_field_predicates_at_transfer(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
        consumer: semantic_vocabulary::OperationId,
    ) -> Option<ByteSequencePredicateSet> {
        let (operation, structural_type) = operation_producer(machine, source)?;
        let result = operation.result.structural()?;
        let OperationKind::StructuralLeafCopy {
            source: backing,
            path,
        } = &operation.kind
        else {
            return None;
        };
        if result.place != source
            || result.structural_type != structural_type
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return None;
        }
        let carrier =
            module
                .structural_types
                .iter()
                .find_map(|declaration| match declaration.shape {
                    StructuralTypeShape::ByteSequence(
                        carrier @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) if declaration.id == structural_type => Some(carrier),
                    _ => None,
                })?;
        let parameter = machine.structural_parameter(*backing)?;
        if !matches!(
            parameter.access,
            StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
        ) {
            return None;
        }
        let (StructuralPathSegment::Field(identity), parent_path) = path.split_last()? else {
            return None;
        };
        if parent_path.iter().any(|segment| {
            !matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        }) {
            return None;
        }
        let parent = runtime_structural_path_tip(
            module.structural_types.iter(),
            parameter.structural_type,
            parent_path,
        )?;
        let fields = parent.shape.record_fields()?;
        let field = terminal_psi::live_field_by_identity(fields, *identity)?;
        if field.field_type.byte_sequence_carrier()? != carrier {
            return None;
        }
        let consuming = machine.operation(consumer)?;
        let call_arguments = consuming.kind.structural_arguments();
        let mut matching = call_arguments
            .iter()
            .chain(consuming.kind.constructor_arguments())
            .filter(|argument| argument.place == source)
            .peekable();
        if matching.peek().is_none()
            || matching.any(|argument| {
                !argument.path.is_empty() || argument.access != StructuralAccess::Owned
            })
        {
            return None;
        }
        if machine.operations()
            .any(|candidate| {
                candidate.id != consumer
                    && (may_change_owned_source(candidate, source)
                        || matches!(&candidate.kind,
                    OperationKind::EstablishReference { source: argument, .. }
                    | OperationKind::EstablishAccessReference { source: argument, .. }
                    | terminal_psi::element_view_establish_pair_pattern!(source: argument, ..)
                    if argument.place == source)
                        || matches!(&candidate.kind, OperationKind::ByteSequenceSubslice { source: viewed, .. }
                            if viewed.place == source))
            })
        {
            return None;
        }
        let carries = |arguments: &[terminal_psi::StructuralArgument]| {
            arguments.iter().any(|argument| argument.place == source)
        };
        if machine.blocks.iter().any(|block| match &block.terminator {
            terminal_psi::Terminator::Jump {
                structural_arguments,
                ..
            }
            | terminal_psi::Terminator::ExternalCompletion {
                structural_arguments,
                ..
            }
            | terminal_psi::Terminator::InvokeNoNormal {
                structural_arguments,
                ..
            } => carries(structural_arguments),
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                carries(&when_true.structural_arguments)
                    || carries(&when_false.structural_arguments)
            }
            terminal_psi::Terminator::ReturnStructural {
                source: returned, ..
            } => *returned == source,
            _ => false,
        }) {
            return None;
        }
        Some(field.field_type.byte_predicates())
    }

    fn concatenation_operand_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operand: &terminal_psi::control_flow::ByteSequenceOperand,
        resolve_whole: &mut impl FnMut(PlaceId) -> Option<ByteSequencePredicateSet>,
    ) -> Option<ByteSequencePredicateSet> {
        match operand {
            terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                if source.access != StructuralAccess::SharedBorrow {
                    return None;
                }
                if source.path.is_empty() {
                    return resolve_whole(source.place);
                }
                let (root, _) = byte_field_root(machine, source.place)?;
                let selected = runtime_structural_path_tip(
                    module.structural_types.iter(),
                    root,
                    &source.path,
                )?;
                if !matches!(selected.shape, StructuralTypeShape::ByteSequence(_)) {
                    return None;
                }
                Some(
                    crate::runtime_structural_field(
                        module.structural_types.iter(),
                        root,
                        &source.path,
                    )
                    .filter(|field| field.field_type.byte_sequence_carrier().is_some())
                    .map_or(ByteSequencePredicateSet::EMPTY, |field| {
                        field.field_type.byte_predicates()
                    }),
                )
            }
            terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } => {
                let (structural_type, access) = byte_field_root(machine, source.place)?;
                if source.access != StructuralAccess::SharedBorrow
                    || !matches!(
                        access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    )
                {
                    return None;
                }
                let declaration = runtime_structural_path_tip(
                    module.structural_types.iter(),
                    structural_type,
                    &source.path,
                )?;
                let fields = declaration.shape.record_fields()?;
                let field = live_field_by_id(fields, *field)?;
                field.field_type.byte_sequence_carrier()?;
                Some(field.field_type.byte_predicates())
            }
        }
    }

    fn may_change_owned_source(operation: &Operation, source: PlaceId) -> bool {
        match &operation.kind {
            OperationKind::ByteSequenceWrite { destination, .. }
            | OperationKind::ByteSequenceStore { destination, .. }
            | OperationKind::WriteOnlyPrimitiveStore { destination, .. }
            | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
            | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
            | OperationKind::StoreStructuralField { destination, .. } => *destination == source,
            OperationKind::MoveStructuralField { source: moved, .. } => *moved == source,
            kind => kind.structural_arguments().iter().any(|argument| {
                argument.place == source
                    && matches!(
                        argument.access,
                        StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                            | StructuralAccess::Owned
                    )
            }),
        }
    }

    pub fn byte_sequence_source_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<ByteSequencePredicateSet> {
        fn source_predicates(
            module: &TerminalModule,
            machine: &TerminalMachine,
            source: PlaceId,
            work: usize,
            memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
        ) -> Option<ByteSequencePredicateSet> {
            if let Some((_, result)) = memo.iter().find(|(place, _)| *place == source) {
                return *result;
            }
            let work = work.checked_sub(1)?;
            let place = machine.structural_place(source)?;
            let index = memo.len();
            memo.push((source, None));
            let result = (|| match place.kind {
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. } => {
                    let parameter =
                        crate::storage::arrival_storage::arrival_parameter(machine, source)?;
                    let owned_buffer = parameter.access == StructuralAccess::Owned
                        && module.structural_types.iter().any(|row| {
                            row.id == parameter.structural_type
                                && row.shape.is_bounded_owned_byte_sequence()
                        });
                    if !owned_buffer
                        && !matches!(
                            parameter.access,
                            terminal_psi::reading_borrow_pair_pattern!()
                        )
                    {
                        return None;
                    }
                    byte_parameter_predicates(module, parameter)
                }
                StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                    let mut producers =
                        machine
                            .operations()
                            .filter_map(|operation| match &operation.kind {
                                OperationKind::EstablishByteSequenceLiteral {
                                    destination,
                                    bytes,
                                    ..
                                } if *destination == source => Some(bytes),
                                _ => None,
                            });
                    let bytes = producers.next()?;
                    if producers.next().is_some() {
                        return None;
                    }
                    Some(ByteSequencePredicateSet::holding(bytes))
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    let operation = machine.operation(producer)?;
                    let result = operation.result.structural()?;
                    if result.place != source || result.structural_type != structural_type {
                        return None;
                    }
                    if let OperationKind::ByteSequenceSubslice {
                        source: backing, ..
                    } = &operation.kind
                    {
                        if result.multiplicity != StructuralMultiplicity::Unrestricted
                            || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty()
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                            || !module.structural_types.iter().any(|declaration| {
                                declaration.id == structural_type
                                    && declaration.shape.is_shared_readable_byte_view()
                            })
                        {
                            return None;
                        }
                        return Some(
                            if backing.path.is_empty() {
                                source_predicates(module, machine, backing.place, work, memo)?
                            } else {
                                let (root, access) = byte_field_root(machine, backing.place)?;
                                if access == StructuralAccess::WriteOnlyBorrow
                                    || backing.access == StructuralAccess::WriteOnlyBorrow
                                {
                                    return None;
                                }
                                shared_byte_field_copy_predicates(
                                    module.structural_types.iter(),
                                    root,
                                    &backing.path,
                                )?
                            }
                            .subslice(),
                        );
                    }
                    let declared = byte_result_predicates(module, machine, source)?;
                    if let OperationKind::StructuralLeafCopy {
                        source: backing,
                        path,
                    } = &operation.kind
                        && module.structural_types.iter().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_borrowed_byte_view()
                        })
                    {
                        let captured = if path.is_empty() {
                            source_predicates(module, machine, *backing, work, memo)?
                        } else {
                            let (root, access) = byte_field_root(machine, *backing)?;
                            if access == StructuralAccess::WriteOnlyBorrow {
                                return None;
                            }
                            shared_byte_field_copy_predicates(
                                module.structural_types.iter(),
                                root,
                                path,
                            )?
                        };
                        let mut predicates = declared;
                        for predicate in captured.predicates() {
                            predicates.insert(predicate);
                        }
                        return Some(predicates);
                    }
                    let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } =
                        &operation.kind
                    else {
                        return Some(declared);
                    };
                    if machine
                        .operations()
                        .any(|candidate| may_change_owned_source(candidate, source))
                    {
                        return byte_owned_result_predicates(module, machine, source);
                    }
                    let mut resolve =
                        |source| source_predicates(module, machine, source, work, memo);
                    Some(
                        concatenation_operand_predicates(module, machine, left, &mut resolve)?
                            .concatenation(concatenation_operand_predicates(
                                module,
                                machine,
                                right,
                                &mut resolve,
                            )?),
                    )
                }
                _ => None,
            })();
            memo[index].1 = result;
            result
        }
        source_predicates(
            module,
            machine,
            source,
            machine.structural_places.len().saturating_add(1),
            &mut Vec::new(),
        )
    }

    pub fn indexed_byte_store_goal(
        bound: semantic_vocabulary::Proposition,
        value: semantic_vocabulary::ValueId,
        predicates: ByteSequencePredicateSet,
    ) -> Result<semantic_vocabulary::Proposition, semantic_vocabulary::PropositionError> {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
        };
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid");
        let literal =
            |byte: u8| ScalarTerm::integer(byte_type, IntegerValue::Unsigned(u128::from(byte)));
        let stored = ScalarTerm::value(value, ScalarType::Integer(byte_type));
        let declares = |predicate| {
            predicates
                .predicates()
                .any(|declared| declared == predicate)
        };
        let mut legs = vec![bound];
        if declares(ByteSequencePredicate::AsciiOnly) || declares(ByteSequencePredicate::ValidUtf8)
        {
            legs.push(Proposition::LessOrEqual(stored.clone(), literal(0x7f)?));
        }
        if declares(ByteSequencePredicate::NoNul) {
            legs.push(Proposition::LessOrEqual(literal(1)?, stored));
        }
        Ok(match legs.len() {
            1 => legs.pop().expect("one leg"),
            _ => Proposition::Conjunction(legs),
        })
    }

    pub fn byte_literal_qualifications_hold(
        domains: &[terminal_psi::StructuralDomainDeclaration],
        carrier: StructuralTypeId,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
        bytes: &[u8],
    ) -> bool {
        byte_literal_qualifications_hold_in(domains.iter(), carrier, qualifications, bytes)
    }

    pub fn byte_literal_qualifications_hold_in<'domain>(
        domains: impl Iterator<Item = &'domain terminal_psi::StructuralDomainDeclaration> + Clone,
        carrier: StructuralTypeId,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
        bytes: &[u8],
    ) -> bool {
        !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
            && qualifications.iter().all(|identity| {
                domains
                    .clone()
                    .find(|domain| domain.id == *identity)
                    .is_some_and(|domain| {
                        domain.carrier == carrier
                            && domain.content_projection.is_none()
                            && domain.establishment_routes.is_none()
                            && domain.has_no_required_memberships()
                            && domain
                                .byte_predicate
                                .is_some_and(|predicate| predicate.holds_for(bytes))
                    })
            })
    }

    pub fn scalar_array_qualifications_hold<'row>(
        types: impl Iterator<Item = &'row terminal_psi::StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'row terminal_psi::StructuralDomainDeclaration> + Clone,
        carrier: StructuralTypeId,
        qualifications: &[semantic_vocabulary::StructuralDomainId],
        elements: &[semantic_vocabulary::ValueId],
        mut literal: impl FnMut(
            semantic_vocabulary::ValueId,
        ) -> Option<(
            semantic_vocabulary::ScalarType,
            semantic_vocabulary::IntegerValue,
        )>,
    ) -> bool {
        if qualifications.is_empty() {
            return true;
        }
        let Some(StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        }) = crate::structural_shape(types.clone(), carrier)
        else {
            return false;
        };
        let byte_type = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .expect("u8 is valid"),
        );
        if u64::try_from(elements.len()).ok() != Some(*length)
            || crate::structural_shape(types.clone(), *element)
                != Some(&StructuralTypeShape::PrimitiveScalar(byte_type))
        {
            return false;
        }
        let bytes = elements
            .iter()
            .map(|element| {
                let (scalar_type, semantic_vocabulary::IntegerValue::Unsigned(value)) =
                    literal(*element)?
                else {
                    return None;
                };
                (scalar_type == byte_type)
                    .then(|| u8::try_from(value).ok())
                    .flatten()
            })
            .collect::<Option<Vec<_>>>();
        bytes.is_some_and(|bytes| {
            byte_literal_qualifications_hold_in(domains, carrier, qualifications, &bytes)
        })
    }

    pub enum ScalarConstantOrigin {
        Literal(
            semantic_vocabulary::ScalarType,
            semantic_vocabulary::IntegerValue,
        ),
        Incoming(Vec<semantic_vocabulary::ValueId>),
    }

    pub fn scalar_constant_value(
        value: semantic_vocabulary::ValueId,
        mut origin: impl FnMut(semantic_vocabulary::ValueId) -> Option<ScalarConstantOrigin>,
    ) -> Option<(
        semantic_vocabulary::ScalarType,
        semantic_vocabulary::IntegerValue,
    )> {
        crate::unchanged_scalar_identity(value, |value| {
            Some(match origin(value)? {
                ScalarConstantOrigin::Literal(ty, value) => {
                    UnchangedScalarOrigin::Known((ty, value))
                }
                ScalarConstantOrigin::Incoming(values) => UnchangedScalarOrigin::Incoming(values),
            })
        })
    }

    pub fn machine_scalar_constant_origin(
        machine: &terminal_psi::TerminalMachine,
        value: semantic_vocabulary::ValueId,
    ) -> Option<ScalarConstantOrigin> {
        scalar_constant_origin_with(
            machine,
            machine
                .operations()
                .find_map(|operation| integer_constant(operation, value)),
            || {
                machine.blocks.iter().find_map(|block| {
                    block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == value)
                        .map(|ordinal| (block.id, ordinal))
                })
            },
        )
    }

    fn integer_constant(
        operation: &Operation,
        value: semantic_vocabulary::ValueId,
    ) -> Option<(
        semantic_vocabulary::ScalarType,
        semantic_vocabulary::IntegerValue,
    )> {
        let result = operation.result.scalar_ref()?;
        let terminal_psi::OperationKind::IntegerConstant { value: literal } = operation.kind else {
            return None;
        };
        (result.id == value).then_some((result.scalar_type, literal))
    }

    fn scalar_constant_origin_with(
        machine: &terminal_psi::TerminalMachine,
        literal: Option<(
            semantic_vocabulary::ScalarType,
            semantic_vocabulary::IntegerValue,
        )>,
        parameter: impl FnOnce() -> Option<(semantic_vocabulary::BlockId, usize)>,
    ) -> Option<ScalarConstantOrigin> {
        if let Some((scalar_type, literal)) = literal {
            return Some(ScalarConstantOrigin::Literal(scalar_type, literal));
        }
        let (owner, ordinal) = parameter()?;
        if owner == machine.entry {
            return None;
        }
        let incoming = machine
            .blocks
            .iter()
            .flat_map(crate::control_graph::forwarded_bindings)
            .filter(|binding| binding.target == owner)
            .map(|binding| binding.arguments);
        incoming_scalar_origin(incoming, ordinal)
    }

    fn incoming_scalar_origin<'block>(
        bindings: impl Iterator<Item = Option<&'block [semantic_vocabulary::ValueId]>>,
        ordinal: usize,
    ) -> Option<ScalarConstantOrigin> {
        let mut incoming = Vec::new();
        for arguments in bindings {
            incoming.push(*arguments?.get(ordinal)?);
        }
        Some(ScalarConstantOrigin::Incoming(incoming))
    }

    pub struct MachineScalarIndex<'machine> {
        machine: &'machine terminal_psi::TerminalMachine,
        definitions: Vec<(semantic_vocabulary::ValueId, &'machine Operation)>,
        parameters: Vec<(
            semantic_vocabulary::ValueId,
            semantic_vocabulary::BlockId,
            usize,
        )>,
        incoming: Vec<(
            semantic_vocabulary::BlockId,
            Option<&'machine [semantic_vocabulary::ValueId]>,
        )>,
    }

    impl<'machine> MachineScalarIndex<'machine> {
        pub fn new(machine: &'machine terminal_psi::TerminalMachine) -> Self {
            let mut definitions: Vec<_> = machine
                .operations()
                .filter_map(|operation| Some((operation.result.scalar_ref()?.id, operation)))
                .collect();
            definitions.sort_by_key(|(value, _)| *value);
            let mut parameters: Vec<_> = machine
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .enumerate()
                        .map(|(ordinal, parameter)| (parameter.id, block.id, ordinal))
                })
                .collect();
            parameters.sort_by_key(|(value, _, _)| *value);
            let mut incoming: Vec<_> = machine
                .blocks
                .iter()
                .flat_map(crate::control_graph::forwarded_bindings)
                .map(|binding| (binding.target, binding.arguments))
                .collect();
            incoming.sort_by_key(|(target, _)| *target);
            Self {
                machine,
                definitions,
                parameters,
                incoming,
            }
        }

        pub fn machine(&self) -> &'machine terminal_psi::TerminalMachine {
            self.machine
        }

        pub fn definitions(
            &self,
            value: semantic_vocabulary::ValueId,
        ) -> impl Iterator<Item = &'machine Operation> + '_ {
            let start = self.definitions.partition_point(|(id, _)| *id < value);
            self.definitions[start..]
                .iter()
                .take_while(move |(id, _)| *id == value)
                .map(|(_, operation)| *operation)
        }

        pub fn constant_origin(
            &self,
            value: semantic_vocabulary::ValueId,
        ) -> Option<ScalarConstantOrigin> {
            if let Some((scalar_type, literal)) = self
                .definitions(value)
                .find_map(|operation| integer_constant(operation, value))
            {
                return Some(ScalarConstantOrigin::Literal(scalar_type, literal));
            }
            let start = self.parameters.partition_point(|(id, _, _)| *id < value);
            let (_, owner, ordinal) = *self
                .parameters
                .get(start)
                .filter(|(id, _, _)| *id == value)?;
            if owner == self.machine.entry {
                return None;
            }
            let start = self.incoming.partition_point(|(target, _)| *target < owner);
            let bindings = self.incoming[start..]
                .iter()
                .take_while(|(target, _)| *target == owner)
                .map(|(_, arguments)| *arguments);
            incoming_scalar_origin(bindings, ordinal)
        }
    }
}

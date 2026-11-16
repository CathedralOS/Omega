/*
Semantic judgments for byte-sequence carriers, byte-view operations, and little-endian primitive
byte-window reads and stores.
*/

pub(crate) mod operation_bounds {
    use crate::{OperationSemanticError, OperationSemanticTag, ValueTypes};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, ObligationId, Proposition, ScalarTerm, ScalarType,
    };
    use terminal_psi::{Operation, OperationKind};

    /// The goal owned by an operation, shared by checking and evidence
    /// publication. Collection bounds do not establish their own source, access or
    /// measurement correspondence; those remain separate storage judgments.
    pub fn operation_safety_goal(
        operation: &Operation,
        types: &impl ValueTypes,
    ) -> Result<Option<(ObligationId, Proposition)>, OperationSemanticError> {
        if let Some(semantics) = crate::proof_bearing_scalar_leaf_semantics(operation, types)? {
            return Ok(Some((
                semantics.obligation(),
                semantics.canonical_goal().kernel_proposition()?,
            )));
        }
        let count =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
        let value = |id, tag| {
            let actual = types
                .value_type(id)
                .ok_or(OperationSemanticError::UnknownValue(id))?;
            if actual != count {
                return Err(OperationSemanticError::OperandShapeMismatch(tag));
            }
            Ok(ScalarTerm::value(id, count))
        };
        let (obligation, goal) = match &operation.kind {
            OperationKind::ByteSequenceRead {
                index,
                length,
                obligation,
                ..
            }
            | OperationKind::StructuralByteSequenceFieldRead {
                index,
                length,
                obligation,
                ..
            }
            | OperationKind::ByteSequenceWrite {
                index,
                length,
                obligation,
                ..
            }
            | OperationKind::ElementViewRead {
                index,
                length,
                obligation,
                ..
            } => {
                let tag = OperationSemanticTag::for_operation(&operation.kind);
                (
                    *obligation,
                    Proposition::LessThan(value(*index, tag)?, value(*length, tag)?),
                )
            }
            OperationKind::ByteSequenceSubslice {
                start,
                end,
                length,
                obligation,
                ..
            }
            | OperationKind::ElementViewSubslice {
                start,
                end,
                length,
                obligation,
                ..
            } => {
                let tag = OperationSemanticTag::for_operation(&operation.kind);
                let (start, end, length) =
                    (value(*start, tag)?, value(*end, tag)?, value(*length, tag)?);
                (
                    *obligation,
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(start, end.clone()),
                        Proposition::LessOrEqual(end, length),
                    ]),
                )
            }
            _ => return Ok(None),
        };
        Ok(Some((obligation, goal)))
    }
}

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
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                let bits = integer.bits()?;
                (bits % 8 == 0).then(|| (u64::from(bits / 8), ScalarType::Integer(integer)))
            }
            StructuralFieldType::Scalar(ScalarType::Boolean) => Some((1, ScalarType::Boolean)),
            StructuralFieldType::BoundedInteger(bounds) => {
                let integer = bounds.integer_type();
                let bits = integer.bits()?;
                (bits % 8 == 0).then(|| (u64::from(bits / 8), ScalarType::Integer(integer)))
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
        module: &TerminalModule,
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
                    && pure_byte_qualifications(
                        module.structural_types.iter(),
                        module.structural_domains.iter(),
                        structural_type,
                        &result.qualifications,
                        &result.projected_qualifications,
                    )
                    && result.qualification_establishments.is_empty()
                    && result.claims.is_empty())
                .then_some((structural_type, StructuralAccess::Owned));
            }
            _ => return None,
        };
        (matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) && pure_byte_qualifications(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            parameter.structural_type,
            &parameter.qualifications,
            &parameter.projected_qualifications,
        ))
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

    /// The octet predicate may represent only its own complete logical
    /// theory, never an additional generic clause on the same domain.
    pub(crate) fn primitive_byte_domain_theory_matches(
        domain: &terminal_psi::StructuralDomainDeclaration,
    ) -> bool {
        let Some(primitive) = domain.byte_predicate else {
            return false;
        };
        let Some(theory) = &domain.predicate else {
            return true;
        };
        if theory.proposition == semantic_vocabulary::Proposition::Truth {
            return true;
        }
        let mut required = ByteSequencePredicateSet::EMPTY;
        required.insert(primitive);
        theory.proposition
            == byte_predicate_requirement(
                required,
                ByteSequencePredicateSet::EMPTY,
                theory.subject,
                &[],
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
                || !primitive_byte_domain_theory_matches(domain)
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

    /// Read the predicates of the actual argument, including a stored view
    /// projection. A write-only argument supplies no observation of its bytes.
    pub fn byte_sequence_argument_predicates(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: &terminal_psi::StructuralArgument,
    ) -> Option<ByteSequencePredicateSet> {
        if source.access == StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        if source.path.is_empty() {
            return byte_sequence_source_predicates(module, machine, source.place);
        }
        let (root, access) = byte_field_root(module, machine, source.place)?;
        if access == StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        shared_byte_field_copy_predicates(module.structural_types.iter(), root, &source.path)
    }

    /// A transfer must establish its byte qualifications from the source.
    /// Its own result annotations cannot justify the qualifications they add.
    pub fn byte_transfer_qualifications_hold(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Result<Option<bool>, &'static str> {
        let (source, source_path, access) = match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                (*source, path.as_slice(), StructuralAccess::SharedBorrow)
            }
            OperationKind::MoveStructuralValue { source } => {
                (source.place, source.path.as_slice(), source.access)
            }
            _ => return Ok(None),
        };
        let result = operation
            .result
            .structural()
            .ok_or("byte transfer has no structural result")?;
        let mut checked = false;
        for (path, id) in result.qualifications.iter().map(|id| (&[][..], id)).chain(
            result
                .projected_qualifications
                .iter()
                .map(|row| (row.path.as_slice(), &row.domain)),
        ) {
            let mut matching = module.structural_domains.iter().filter(|row| row.id == *id);
            let domain = matching.next().ok_or("byte transfer domain is absent")?;
            if matching.next().is_some() {
                return Err("byte transfer domain is ambiguous");
            }
            if domain.byte_predicate.is_none() {
                continue;
            }
            let carrier = crate::qualification_path_carrier(
                module.structural_types.iter(),
                result.structural_type,
                path,
            )
            .ok_or("byte transfer qualification has no exact carrier")?;
            if !byte_predicate_carrier(module.structural_types.iter(), carrier) {
                return Err("byte transfer qualification carrier is not bytes");
            }
            let required = byte_qualification_predicates_in(
                module.structural_domains.iter(),
                carrier,
                std::slice::from_ref(id),
            )
            .ok_or("byte transfer qualification is not a complete primitive byte domain")?;
            if source == result.place {
                return Err("byte transfer cannot use its own result as evidence");
            }
            let mut combined_path = Vec::with_capacity(source_path.len() + path.len());
            combined_path.extend_from_slice(source_path);
            combined_path.extend_from_slice(path);
            let argument = terminal_psi::StructuralArgument {
                place: source,
                path: combined_path,
                access,
            };
            let supplied = byte_sequence_argument_predicates(module, machine, &argument)
                .ok_or("byte transfer source predicates are unavailable")?;
            checked = true;
            if !supplied.implies(required) {
                return Ok(Some(false));
            }
        }
        Ok(checked.then_some(true))
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
                let (root, _) = byte_field_root(module, machine, source.place)?;
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
                let (structural_type, access) = byte_field_root(module, machine, source.place)?;
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
                    // A join owns its shared descriptor, whose byte facts
                    // arrive with the parameter just as a buffer's do.
                    let owned_readable = parameter.access == StructuralAccess::Owned
                        && module.structural_types.iter().any(|row| {
                            row.id == parameter.structural_type
                                && row.shape.is_owned_or_shared_byte_carrier()
                        });
                    if !owned_readable
                        && !matches!(
                            parameter.access,
                            terminal_psi::reading_borrow_pair_pattern!()
                        )
                    {
                        return None;
                    }
                    let mut predicates = byte_parameter_predicates(module, parameter)?;
                    if matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                        && parameter.access == StructuralAccess::SharedBorrow
                        && module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && declaration.shape.is_shared_readable_byte_view()
                        })
                    {
                        shared_entry_byte_predicates(machine, source, &mut predicates);
                    }
                    Some(predicates)
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
                                let (root, access) =
                                    byte_field_root(module, machine, backing.place)?;
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
                    let copy = match &operation.kind {
                        OperationKind::MoveStructuralValue { source } => {
                            Some((source.place, source.path.as_slice()))
                        }
                        OperationKind::StructuralLeafCopy { source, path } => {
                            Some((*source, path.as_slice()))
                        }
                        _ => None,
                    };
                    if let Some((backing, path)) = copy
                        && module.structural_types.iter().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_borrowed_byte_view()
                        })
                    {
                        let captured = if path.is_empty() {
                            source_predicates(module, machine, backing, work, memo)?
                        } else {
                            let (root, access) = byte_field_root(module, machine, backing)?;
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

    // An entry fact remains true of the same shared descriptor. Only a
    // required positive atom establishes a predicate over its whole backing.
    fn shared_entry_byte_predicates(
        machine: &TerminalMachine,
        source: PlaceId,
        predicates: &mut ByteSequencePredicateSet,
    ) {
        use semantic_vocabulary::{Proposition, ScalarTerm};

        let mut required = machine.contract.requires.iter().collect::<Vec<_>>();
        while let Some(clause) = required.pop() {
            match clause {
                Proposition::Conjunction(parts) => required.extend(parts),
                Proposition::Equal(ScalarTerm::Boolean(true), term)
                | Proposition::Equal(term, ScalarTerm::Boolean(true)) => {
                    if let ScalarTerm::ByteSequencePredicate {
                        predicate,
                        root,
                        path,
                    } = term
                        && *root == source
                        && path.is_empty()
                    {
                        predicates.insert(*predicate);
                    }
                }
                _ => {}
            }
        }
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
                            && domain.is_pure_predicate()
                            && primitive_byte_domain_theory_matches(domain)
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
        /// One operation's typed denotation, not an expanded operand graph.
        Computed(semantic_vocabulary::ScalarTerm),
    }

    type KnownScalar = (
        semantic_vocabulary::ScalarType,
        semantic_vocabulary::IntegerValue,
    );

    enum ScalarConstantState {
        Unseen,
        Active,
        Complete(Option<KnownScalar>),
    }

    /// Resolve each actual scalar definition once. The prepared rows contain
    /// only this owner's real IDs, so sparse or hostile IDs cannot size an arena.
    /// Arithmetic stays with the existing typed evaluator; cycles and unavailable
    /// inputs supply no constant.
    pub struct ScalarConstants {
        values: Vec<(semantic_vocabulary::ValueId, ScalarConstantState)>,
        valid: bool,
    }

    impl Default for ScalarConstants {
        fn default() -> Self {
            Self {
                values: Vec::new(),
                valid: true,
            }
        }
    }

    impl ScalarConstants {
        pub fn for_values(values: impl IntoIterator<Item = semantic_vocabulary::ValueId>) -> Self {
            let mut values = values
                .into_iter()
                .map(|id| (id, ScalarConstantState::Unseen))
                .collect::<Vec<_>>();
            values.sort_unstable_by_key(|(id, _)| *id);
            let valid = values.windows(2).all(|pair| pair[0].0 < pair[1].0);
            Self { values, valid }
        }

        pub fn registered_count(&self) -> usize {
            self.values.len()
        }

        /// A draft owner appends its actual newly declared IDs in order. An
        /// out-of-order or repeated declaration invalidates this query's scope.
        pub fn register(&mut self, id: semantic_vocabulary::ValueId) -> bool {
            if !self.valid || self.values.last().is_some_and(|(last, _)| *last >= id) {
                self.valid = false;
                return false;
            }
            self.values.push((id, ScalarConstantState::Unseen));
            true
        }

        fn index(&self, value: semantic_vocabulary::ValueId) -> Option<usize> {
            self.values.binary_search_by_key(&value, |(id, _)| *id).ok()
        }

        fn known(&self, value: semantic_vocabulary::ValueId) -> Option<KnownScalar> {
            match &self.values.get(self.index(value)?)?.1 {
                ScalarConstantState::Complete(value) => *value,
                _ => None,
            }
        }

        pub fn value(
            &mut self,
            value: semantic_vocabulary::ValueId,
            mut origin: impl FnMut(semantic_vocabulary::ValueId) -> Option<ScalarConstantOrigin>,
        ) -> Option<KnownScalar> {
            use semantic_vocabulary::{ScalarTerm, ValueId};
            if !self.valid {
                return None;
            }
            self.index(value)?;
            enum Step {
                Visit(ValueId),
                Complete(ValueId, ScalarConstantOrigin),
            }
            let mut pending = vec![Step::Visit(value)];
            let mut active: Vec<usize> = Vec::new();
            let mut remaining = 8192usize;
            while let Some(step) = pending.pop() {
                let Some(next) = remaining.checked_sub(1) else {
                    for index in active {
                        if matches!(self.values[index].1, ScalarConstantState::Active) {
                            self.values[index].1 = ScalarConstantState::Complete(None);
                        }
                    }
                    return None;
                };
                remaining = next;
                match step {
                    Step::Visit(id) => {
                        let Some(index) = self.index(id) else {
                            continue;
                        };
                        match self.values[index].1 {
                            ScalarConstantState::Complete(_) => continue,
                            ScalarConstantState::Active => {
                                self.values[index].1 = ScalarConstantState::Complete(None);
                                continue;
                            }
                            ScalarConstantState::Unseen => {}
                        }
                        self.values[index].1 = ScalarConstantState::Active;
                        active.push(index);
                        let Some(definition) = origin(id) else {
                            self.values[index].1 = ScalarConstantState::Complete(None);
                            continue;
                        };
                        let mut dependencies = Vec::new();
                        match &definition {
                            ScalarConstantOrigin::Literal(..) => {}
                            ScalarConstantOrigin::Incoming(values) => {
                                if values.len() > remaining {
                                    self.values[index].1 = ScalarConstantState::Complete(None);
                                    continue;
                                }
                                dependencies.extend(values.iter().copied());
                            }
                            ScalarConstantOrigin::Computed(term) => {
                                term.visit_value_ids(|value| {
                                    dependencies.push(value);
                                    dependencies.len() <= remaining
                                });
                            }
                        }
                        if dependencies.len() > remaining {
                            self.values[index].1 = ScalarConstantState::Complete(None);
                            continue;
                        }
                        pending.push(Step::Complete(id, definition));
                        pending.extend(dependencies.into_iter().rev().map(Step::Visit));
                    }
                    Step::Complete(id, definition) => {
                        let index = self.index(id)?;
                        if matches!(self.values[index].1, ScalarConstantState::Complete(_)) {
                            continue;
                        }
                        let known = |value| self.known(value);
                        let result = match definition {
                            ScalarConstantOrigin::Literal(ty, literal) => {
                                crate::inhabitation::known_scalar_value_with(
                                    &ScalarTerm::value(id, ty),
                                    |value| (value == id).then_some((ty, literal)),
                                )
                            }
                            ScalarConstantOrigin::Incoming(values) => {
                                let mut values = values.into_iter();
                                values.next().and_then(known).filter(|first| {
                                    values.all(|value| known(value) == Some(*first))
                                })
                            }
                            ScalarConstantOrigin::Computed(term) => {
                                crate::inhabitation::known_scalar_value_with(&term, known)
                            }
                        };
                        self.values[index].1 = ScalarConstantState::Complete(result);
                    }
                }
            }
            self.known(value)
        }
    }

    /// Borrow the canonical scalar operation meaning. Merely knowing its
    /// result does not discharge the operation's independent safety obligation.
    pub fn scalar_operation_origin(
        operation: &Operation,
        value: semantic_vocabulary::ValueId,
        types: &dyn crate::ValueTypes,
    ) -> Option<ScalarConstantOrigin> {
        let result = operation.result.scalar_ref()?;
        if result.id != value || types.value_type(value) != Some(result.scalar_type) {
            return None;
        }
        if let Some((ty, literal)) = scalar_operation_literal(operation, value) {
            return Some(ScalarConstantOrigin::Literal(ty, literal));
        }
        let equation = if let Some(meaning) =
            crate::proof_bearing_scalar_leaf_semantics(operation, types).ok()?
        {
            meaning.result_equation().clone()
        } else if let Some(meaning) =
            crate::goal_free_scalar_leaf_semantics(operation, types).ok()?
        {
            meaning.result_equation().clone()
        } else {
            crate::trapping_integer_semantics(operation, types)
                .ok()??
                .result_equation()
                .clone()
        };
        let semantic_vocabulary::Proposition::Equal(left, right) = equation else {
            return None;
        };
        (left == semantic_vocabulary::ScalarTerm::value(value, result.scalar_type))
            .then_some(ScalarConstantOrigin::Computed(right))
    }

    /// Read a literal from the exact operation that owns this value. The
    /// caller borrows it from the same machine's draft coordinates or final
    /// index, never from a coincident numeric ID in another machine. This
    /// function authenticates the operation's result ID and scalar tag; a
    /// signature placeholder alone supplies no operation.
    pub fn scalar_operation_literal(
        operation: &Operation,
        value: semantic_vocabulary::ValueId,
    ) -> Option<(
        semantic_vocabulary::ScalarType,
        semantic_vocabulary::IntegerValue,
    )> {
        let result = operation.result.scalar_ref()?;
        if result.id != value {
            return None;
        }
        let literal = match (&operation.kind, result.scalar_type) {
            (
                terminal_psi::OperationKind::IntegerConstant { value },
                semantic_vocabulary::ScalarType::Integer(_),
            ) => *value,
            (
                terminal_psi::OperationKind::BooleanConstant { value },
                semantic_vocabulary::ScalarType::Boolean,
            ) => semantic_vocabulary::IntegerValue::Unsigned(u128::from(u8::from(*value))),
            _ => return None,
        };
        Some((result.scalar_type, literal))
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
            semantic_vocabulary::ScalarType,
        )>,
        constants: std::cell::RefCell<ScalarConstants>,
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
                        .map(|(ordinal, parameter)| {
                            (parameter.id, block.id, ordinal, parameter.scalar_type)
                        })
                })
                .collect();
            parameters.extend(
                machine
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| {
                        (parameter.id, machine.entry, ordinal, parameter.scalar_type)
                    }),
            );
            parameters.sort_by_key(|(value, _, _, _)| *value);
            let mut incoming: Vec<_> = machine
                .blocks
                .iter()
                .flat_map(crate::control_graph::forwarded_bindings)
                .map(|binding| (binding.target, binding.arguments))
                .collect();
            incoming.sort_by_key(|(target, _)| *target);
            let constants = ScalarConstants::for_values(
                definitions
                    .iter()
                    .map(|(id, _)| *id)
                    .chain(parameters.iter().map(|(id, ..)| *id)),
            );
            Self {
                machine,
                definitions,
                parameters,
                incoming,
                constants: std::cell::RefCell::new(constants),
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

        fn constant_origin(
            &self,
            value: semantic_vocabulary::ValueId,
        ) -> Option<ScalarConstantOrigin> {
            if let Some((scalar_type, literal)) = self
                .definitions(value)
                .find_map(|operation| scalar_operation_literal(operation, value))
            {
                return Some(ScalarConstantOrigin::Literal(scalar_type, literal));
            }
            let start = self.parameters.partition_point(|(id, ..)| *id < value);
            let (_, owner, ordinal, _) =
                *self.parameters.get(start).filter(|(id, ..)| *id == value)?;
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

        pub fn constant_value(&self, value: semantic_vocabulary::ValueId) -> Option<KnownScalar> {
            let known = self
                .constants
                .borrow_mut()
                .value(value, |value| self.computed_origin(value, self))?;
            (known.0 == crate::ValueTypes::value_type(self, value)?).then_some(known)
        }

        /// A definition must have exactly one actual producer. The receiving
        /// owner supplies this same machine's complete, checked value types.
        fn computed_origin(
            &self,
            value: semantic_vocabulary::ValueId,
            types: &dyn crate::ValueTypes,
        ) -> Option<ScalarConstantOrigin> {
            let mut definitions = self.definitions(value);
            if let Some(operation) = definitions.next() {
                if definitions.next().is_some() {
                    return None;
                }
                return scalar_operation_origin(operation, value, types);
            }
            self.constant_origin(value)
        }
    }

    impl crate::ValueTypes for MachineScalarIndex<'_> {
        fn value_type(
            &self,
            value: semantic_vocabulary::ValueId,
        ) -> Option<semantic_vocabulary::ScalarType> {
            let start = self.parameters.partition_point(|(id, ..)| *id < value);
            let parameter = self.parameters.get(start).filter(|(id, ..)| *id == value);
            if parameter.is_some()
                && self
                    .parameters
                    .get(start + 1)
                    .is_some_and(|(id, ..)| *id == value)
            {
                return None;
            }
            let mut definitions = self.definitions(value);
            match definitions.next() {
                Some(operation) if definitions.next().is_none() && parameter.is_none() => {
                    Some(operation.result.scalar_ref()?.scalar_type)
                }
                Some(_) => None,
                None => parameter.map(|(_, _, _, ty)| *ty),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
            OperationId, ScalarType, ValueId,
        };
        use terminal_psi::{
            Block, MachineContract, Operation, OperationKind, OperationResult,
            RegisterTargetApplicability, TerminalMachine, Terminator, ValueDeclaration,
        };

        fn value(raw: u64, scalar_type: ScalarType) -> ValueDeclaration {
            ValueDeclaration {
                id: ValueId::new(raw).unwrap(),
                scalar_type,
                qualifications: Default::default(),
            }
        }

        fn constant(raw: u64, scalar_type: ScalarType, kind: OperationKind) -> Operation {
            Operation::unsuspended(
                OperationId::new(raw).unwrap(),
                OperationResult::Scalar(value(raw, scalar_type)),
                kind,
            )
        }

        #[test]
        fn boolean_literals_keep_their_type_and_exact_zero_or_one_value() {
            let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
            for boolean in [false, true] {
                let operation = constant(
                    1,
                    ScalarType::Boolean,
                    OperationKind::BooleanConstant { value: boolean },
                );
                assert_eq!(
                    scalar_operation_literal(&operation, ValueId::new(1).unwrap()),
                    Some((
                        ScalarType::Boolean,
                        IntegerValue::Unsigned(u128::from(u8::from(boolean)))
                    ))
                );
                assert_eq!(
                    scalar_operation_literal(&operation, ValueId::new(2).unwrap()),
                    None
                );
                let wrong_type = constant(
                    1,
                    u8_type,
                    OperationKind::BooleanConstant { value: boolean },
                );
                assert_eq!(
                    scalar_operation_literal(&wrong_type, ValueId::new(1).unwrap()),
                    None
                );
            }
            let integer = constant(
                1,
                u8_type,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(7),
                },
            );
            assert_eq!(
                scalar_operation_literal(&integer, ValueId::new(1).unwrap()),
                Some((u8_type, IntegerValue::Unsigned(7)))
            );
            let wrong_type = constant(
                1,
                ScalarType::Boolean,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(1),
                },
            );
            assert_eq!(
                scalar_operation_literal(&wrong_type, ValueId::new(1).unwrap()),
                None
            );
        }

        #[test]
        fn the_existing_machine_index_resolves_boolean_forwarding_without_rebinding_parameters() {
            let first = BlockId::new(1).unwrap();
            let second = BlockId::new(2).unwrap();
            let formal = value(1, ScalarType::Boolean);
            let mut entry = Block::plain(
                first,
                vec![formal],
                Terminator::Jump {
                    edge: EdgeId::new(1).unwrap(),
                    target: second,
                    arguments: vec![ValueId::new(2).unwrap()],
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            );
            entry.operations.push(constant(
                2,
                ScalarType::Boolean,
                OperationKind::BooleanConstant { value: true },
            ));
            let join = Block::plain(
                second,
                vec![value(3, ScalarType::Boolean)],
                Terminator::ReturnUnit {
                    edge: EdgeId::new(2).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            );
            let machine = TerminalMachine {
                assembly_target_requirement: RegisterTargetApplicability::Any,
                id: MachineId::new(1).unwrap(),
                attachment: None,
                ranked_scc: None,
                structural_places: Vec::new(),
                call_formations: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: first,
                blocks: vec![entry, join],
                contract: MachineContract::empty(ContractId::new(1).unwrap()),
            };
            let index = MachineScalarIndex::new(&machine);
            let expected = Some((ScalarType::Boolean, IntegerValue::Unsigned(1)));
            assert_eq!(index.constant_value(ValueId::new(2).unwrap()), expected);
            assert_eq!(index.constant_value(ValueId::new(3).unwrap()), expected);
            assert_eq!(index.constant_value(ValueId::new(1).unwrap()), None);
            assert_eq!(index.constant_value(ValueId::new(99).unwrap()), None);
        }

        #[test]
        fn computed_scalars_follow_typed_operations_and_memoize_shared_inputs() {
            let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
            let id = |raw| ValueId::new(raw).unwrap();
            let operations = [
                constant(
                    1,
                    integer,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                ),
                constant(
                    2,
                    integer,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                ),
                constant(
                    3,
                    integer,
                    OperationKind::ExactIntegerAdd {
                        left: id(1),
                        right: id(2),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    },
                ),
                constant(
                    4,
                    integer,
                    OperationKind::WrappingIntegerAdd {
                        left: id(3),
                        right: id(3),
                    },
                ),
            ];
            let types = operations
                .iter()
                .map(|op| (op.result.scalar_ref().unwrap().id, integer))
                .collect::<std::collections::BTreeMap<_, _>>();
            let mut constants = ScalarConstants::for_values(
                operations
                    .iter()
                    .map(|operation| operation.result.scalar_ref().unwrap().id),
            );
            let mut visits = [0; 4];
            for _ in 0..2 {
                assert_eq!(
                    constants.value(id(4), |value| {
                        let index = usize::try_from(value.get() - 1).ok()?;
                        let operation = operations.get(index)?;
                        visits[index] += 1;
                        scalar_operation_origin(operation, value, &types)
                    }),
                    Some((integer, IntegerValue::Unsigned(2)))
                );
            }
            assert_eq!(
                visits, [1; 4],
                "shared operands and repeated actuals resolve once"
            );
            assert!(scalar_operation_origin(&operations[2], id(99), &types).is_none());
            let mut wrong_types = types.clone();
            wrong_types.insert(id(2), ScalarType::Boolean);
            assert!(scalar_operation_origin(&operations[2], id(3), &wrong_types).is_none());
        }

        #[test]
        fn scalar_constants_keep_arithmetic_failures_cycles_and_unknown_inputs_unknown() {
            use semantic_vocabulary::ScalarTerm;
            let ty = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let integer = ScalarType::Integer(ty);
            let id = |raw| ValueId::new(raw).unwrap();
            let mut constants = ScalarConstants::for_values((1..=8).map(id));
            let mut origin = |value: ValueId| {
                Some(match value.get() {
                    1 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(255)),
                    2 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(1)),
                    3 => ScalarConstantOrigin::Computed(ScalarTerm::ExactIntegerAdd {
                        scalar_type: ty,
                        left: Box::new(ScalarTerm::value(id(1), integer)),
                        right: Box::new(ScalarTerm::value(id(2), integer)),
                    }),
                    4 => ScalarConstantOrigin::Computed(ScalarTerm::WrappingIntegerAdd {
                        scalar_type: ty,
                        left: Box::new(ScalarTerm::value(id(1), integer)),
                        right: Box::new(ScalarTerm::value(id(2), integer)),
                    }),
                    5 => ScalarConstantOrigin::Incoming(vec![id(4), id(4)]),
                    6 => ScalarConstantOrigin::Incoming(vec![id(5), id(2)]),
                    7 => ScalarConstantOrigin::Computed(ScalarTerm::value(id(7), integer)),
                    8 => ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(256)),
                    _ => return None,
                })
            };
            assert_eq!(constants.value(id(3), &mut origin), None);
            assert_eq!(
                constants.value(id(4), &mut origin),
                Some((integer, IntegerValue::Unsigned(0)))
            );
            assert_eq!(
                constants.value(id(5), &mut origin),
                Some((integer, IntegerValue::Unsigned(0)))
            );
            for raw in [6, 7, 8, 99] {
                assert_eq!(constants.value(id(raw), &mut origin), None);
            }
            let mut budgeted = ScalarConstants::for_values((1..=5000).map(id));
            assert_eq!(
                budgeted.value(id(1), |value| Some(if value.get() == 5000 {
                    ScalarConstantOrigin::Literal(integer, IntegerValue::Unsigned(1))
                } else {
                    ScalarConstantOrigin::Computed(ScalarTerm::value(id(value.get() + 1), integer))
                })),
                None
            );
            assert_eq!(
                budgeted.value(id(1), |_| panic!("an exhausted root is not searched again")),
                None
            );
        }

        #[test]
        fn prepared_scalar_ids_refuse_duplicates_and_keep_sparse_ids_bounded() {
            let high = ValueId::new(u64::MAX).unwrap();
            let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
            let mut sparse = ScalarConstants::for_values([high]);
            assert_eq!(sparse.registered_count(), 1);
            assert_eq!(
                sparse.value(high, |_| Some(ScalarConstantOrigin::Literal(
                    integer,
                    IntegerValue::Unsigned(1)
                ))),
                Some((integer, IntegerValue::Unsigned(1)))
            );
            let mut duplicate = ScalarConstants::for_values([high, high]);
            assert_eq!(
                duplicate.value(high, |_| panic!("duplicate source IDs supply no scope")),
                None
            );
            let mut draft = ScalarConstants::default();
            assert!(draft.register(ValueId::new(2).unwrap()));
            assert!(!draft.register(ValueId::new(1).unwrap()));
            assert_eq!(
                draft.value(ValueId::new(2).unwrap(), |_| panic!(
                    "non-monotonic declarations supply no scope"
                )),
                None
            );
        }

        #[test]
        fn mixed_boolean_incoming_values_do_not_prove_a_constant() {
            for equal in [false, true] {
                let actual =
                    ScalarConstants::for_values((1..=3).map(|raw| ValueId::new(raw).unwrap()))
                        .value(ValueId::new(3).unwrap(), |value| match value.get() {
                            1 => Some(ScalarConstantOrigin::Literal(
                                ScalarType::Boolean,
                                IntegerValue::Unsigned(1),
                            )),
                            2 => Some(ScalarConstantOrigin::Literal(
                                ScalarType::Boolean,
                                IntegerValue::Unsigned(u128::from(u8::from(equal))),
                            )),
                            3 => Some(ScalarConstantOrigin::Incoming(vec![
                                ValueId::new(1).unwrap(),
                                ValueId::new(2).unwrap(),
                            ])),
                            _ => None,
                        });
                assert_eq!(
                    actual,
                    equal.then_some((ScalarType::Boolean, IntegerValue::Unsigned(1)))
                );
            }
        }
    }
}

/*
    External boundary effects, effect handler traits, boundary predicates,
    byte-buffer and scalar loans, crash outcomes, and host result binding.

    We stage pre-call byte and scalar buffers, validate normal and crashing
    host responses against declared boundary contracts, and commit authorized
    qualifications and buffer writebacks on completion.
*/

pub(crate) mod boundary_predicates {
    use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerMathTerm, IntegerValue, PlaceId, Proposition,
        PropositionContext, ScalarReadProjection, ScalarTerm, ScalarType, StructuralPlaceKind,
    };
    use terminal_psi::{BoundaryMachineDeclaration, StructuralArgument};

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
    use crate::engine::execution::TerminalExecution;
    use terminal_semantics::substitution::substitute_proposition_values;

    pub(crate) fn scalar_literals(
        arguments: &[TerminalScalarValue],
    ) -> Result<Vec<ScalarTerm>, TerminalInterpretError> {
        arguments.iter().copied().map(scalar_literal).collect()
    }

    fn scalar_literal(argument: TerminalScalarValue) -> Result<ScalarTerm, TerminalInterpretError> {
        argument.literal().ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "machine values have no mathematical literal",
        ))
    }

    pub(crate) fn validate_input_requirements(
        boundary: &BoundaryMachineDeclaration,
        arguments: &[crate::boundary::values::TerminalScalarValue],
        execution: &TerminalExecution<'_>,
        structural_arguments: &[StructuralArgument],
        structural_values: &[TerminalStructuralValue],
    ) -> Result<(), crate::boundary::errors::TerminalInterpretError> {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        if boundary.scalar_requires.is_empty() {
            return Ok(());
        }
        let arguments = scalar_literals(arguments)?;
        let parameters = boundary
            .scalar_contract_parameters()
            .ok_or_else(malformed)?;
        if arguments.len() != parameters.len()
            || arguments
                .iter()
                .zip(&parameters)
                .any(|(actual, parameter)| {
                    actual.scalar_type() != parameter.scalar_type || actual.validate().is_err()
                })
        {
            return Err(malformed());
        }
        proof_admission::check_predicate_evaluation_size(boundary.scalar_requires.iter()).map_err(
            |_| {
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement evaluation exceeds the resource limit",
                )
            },
        )?;
        let context = PropositionContext::from_value_types_and_places(
            parameters
                .iter()
                .map(|parameter| (parameter.id, parameter.scalar_type)),
            boundary.structural_parameters.iter().map(|parameter| {
                (
                    parameter.place,
                    StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    },
                )
            }),
        )
        .map_err(|_| malformed())?;
        let substitutions = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| (parameter.id, argument.clone()))
            .collect();
        let mut integers = ClosedIntegerEvaluator::default();
        let observations = InputObservations {
            execution,
            boundary,
            arguments: structural_arguments,
            values: structural_values,
        };
        for requirement in &boundary.scalar_requires {
            if requirement.validate().is_err() || context.validate(requirement).is_err() {
                return Err(malformed());
            }
            let requirement = substitute_proposition_values(requirement, &substitutions);
            match observations.evaluate(&requirement, &mut integers)? {
                Some(true) => {}
                Some(false) => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "boundary input requirement is false for this invocation",
                    ));
                }
                None => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "boundary input requirement has no supported closed evaluation",
                    ));
                }
            }
        }
        Ok(())
    }

    struct InputObservations<'execution, 'artifact, 'call> {
        execution: &'execution TerminalExecution<'artifact>,
        boundary: &'call BoundaryMachineDeclaration,
        arguments: &'call [StructuralArgument],
        values: &'call [TerminalStructuralValue],
    }

    impl InputObservations<'_, '_, '_> {
        fn evaluate(
            &self,
            predicate: &Proposition,
            integers: &mut ClosedIntegerEvaluator,
        ) -> Result<Option<bool>, TerminalInterpretError> {
            match predicate {
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    let identity = matches!(predicate, Proposition::Conjunction(_));
                    for child in children {
                        match self.evaluate(child, integers)? {
                            Some(value) if value != identity => return Ok(Some(value)),
                            Some(_) => {}
                            None => return Ok(None),
                        }
                    }
                    Ok(Some(identity))
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => match self.evaluate(premise, integers)? {
                    Some(false) => Ok(Some(true)),
                    Some(true) => self.evaluate(conclusion, integers),
                    None => Ok(None),
                },
                _ => {
                    let mut closed = predicate.clone();
                    self.close_predicate(&mut closed)?;
                    closed_guard(&closed, integers).map_err(|_| {
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "boundary input requirement evaluation exceeds the resource limit",
                        )
                    })
                }
            }
        }

        fn close_predicate(
            &self,
            predicate: &mut Proposition,
        ) -> Result<(), TerminalInterpretError> {
            match predicate {
                semantic_vocabulary::ordered_comparison_operands!(left, right)
                | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                    self.close_scalar(left)?;
                    self.close_scalar(right)?;
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    self.close_mathematical_integer(left)?;
                    self.close_mathematical_integer(right)?;
                }
                _ => {}
            }
            Ok(())
        }

        fn close_mathematical_integer(
            &self,
            term: &mut IntegerMathTerm,
        ) -> Result<(), TerminalInterpretError> {
            if let IntegerMathTerm::Embed(scalar) = term {
                self.close_scalar(scalar)?;
            } else {
                for operand in term.operands_mut() {
                    self.close_mathematical_integer(operand)?;
                }
            }
            Ok(())
        }

        fn close_scalar(&self, term: &mut ScalarTerm) -> Result<(), TerminalInterpretError> {
            for operand in term.operands_mut() {
                self.close_scalar(operand)?;
            }
            let observed = match term {
                ScalarTerm::StructuralRead {
                    root,
                    path,
                    scalar_type,
                } => {
                    let mut selected = Vec::with_capacity(path.len());
                    let path_length = path.len();
                    let mut extent = false;
                    for (position, segment) in path.iter_mut().enumerate() {
                        selected.push(match segment {
                            ScalarReadProjection::Static(segment) => *segment,
                            ScalarReadProjection::Referent => {
                                CanonicalStructuralPathSegment::Referent
                            }
                            ScalarReadProjection::Index(index) => {
                                let (_, value) = index.integer_value().ok_or_else(malformed)?;
                                let index = match value {
                                    IntegerValue::Unsigned(value) => u64::try_from(value),
                                    IntegerValue::Signed(value) => u64::try_from(value),
                                }
                                .map_err(|_| malformed())?;
                                CanonicalStructuralPathSegment::FixedIndex(index)
                            }
                            ScalarReadProjection::Extent => {
                                if position + 1 != path_length {
                                    return Err(malformed());
                                }
                                extent = true;
                                continue;
                            }
                        });
                    }
                    Some(if extent {
                        let ScalarType::Integer(carrier) = scalar_type else {
                            return Err(malformed());
                        };
                        self.read_extent(*root, &selected, *carrier)?
                    } else {
                        self.read(*root, &selected, *scalar_type)?
                    })
                }
                ScalarTerm::ViewExtent {
                    root,
                    path,
                    scalar_type,
                } => Some(self.read_extent(*root, path, *scalar_type)?),
                ScalarTerm::BooleanField { root, path } => {
                    Some(self.read(*root, path, ScalarType::Boolean)?)
                }
                ScalarTerm::IntegerField {
                    root,
                    path,
                    scalar_type,
                } => Some(self.read(*root, path, ScalarType::Integer(*scalar_type))?),
                ScalarTerm::IeeeFloatField { root, path, format } => {
                    Some(self.read(*root, path, ScalarType::IeeeFloat(*format))?)
                }
                _ => None,
            };
            if let Some(value) = observed {
                *term = scalar_literal(value)?;
            }
            Ok(())
        }

        fn read_extent(
            &self,
            root: PlaceId,
            path: &[CanonicalStructuralPathSegment],
            carrier: semantic_vocabulary::IntegerType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            use terminal_psi::{StructuralAccess, StructuralTypeShape};
            let position = self
                .boundary
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == root)
                .ok_or_else(malformed)?;
            let parameter = &self.boundary.structural_parameters[position];
            let actual = self.arguments.get(position).ok_or_else(malformed)?;
            self.values.get(position).ok_or_else(malformed)?;
            let relative = terminal_semantics::canonical_structural_storage_path(
                self.execution.program.structural_types().values(),
                parameter.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            let mut source = actual.clone();
            source.path.extend(relative);
            let access =
                match parameter.access {
                    StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow => {
                        self.execution
                            .validate_descriptor_borrow(&source, StructuralAccess::SharedBorrow)?;
                        StructuralAccess::SharedBorrow
                    }
                    StructuralAccess::WriteOnlyBorrow => {
                        if source.path.iter().any(|step| {
                            matches!(step, terminal_psi::StructuralPathSegment::Referent)
                        }) {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "boundary write-only descriptor extent cannot cross a reference",
                            ));
                        }
                        StructuralAccess::WriteOnlyBorrow
                    }
                };
            source.access = access;
            let selected =
                self.execution
                    .resolve_live_projection(source.place, &source.path, access)?;
            let length = match self
                .execution
                .program
                .structural_types()
                .get(&selected.structural_type)
                .map(|row| &row.shape)
            {
                Some(StructuralTypeShape::ElementView { .. }) => u64::try_from(
                    self.execution
                        .element_view_argument(&source, &selected)?
                        .len(),
                )
                .map_err(|_| malformed())?,
                Some(StructuralTypeShape::ByteSequence(_))
                    if access != StructuralAccess::WriteOnlyBorrow =>
                {
                    let binding = self.execution.byte_descriptor_binding(&source, &selected)?;
                    self.execution.byte_descriptor_length(&binding, &selected)?
                }
                _ => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "boundary extent requires an observable live view descriptor",
                    ));
                }
            };
            let value = match carrier.sign() {
                semantic_vocabulary::IntegerSign::Signed => {
                    IntegerValue::Signed(i128::from(length))
                }
                semantic_vocabulary::IntegerSign::Unsigned => {
                    IntegerValue::Unsigned(u128::from(length))
                }
            };
            if !carrier.admits(value) {
                return Err(malformed());
            }
            Ok(TerminalScalarValue::Integer {
                scalar_type: carrier,
                value,
            })
        }

        fn read(
            &self,
            root: PlaceId,
            path: &[CanonicalStructuralPathSegment],
            scalar_type: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let position = self
                .boundary
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == root)
                .ok_or_else(malformed)?;
            let parameter = &self.boundary.structural_parameters[position];
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input predicate cannot observe a write-only argument",
                ));
            }
            let argument = self.arguments.get(position).ok_or_else(malformed)?;
            let value = self.values.get(position).ok_or_else(malformed)?;
            let path = terminal_semantics::canonical_structural_storage_path(
                self.execution.program.structural_types().values(),
                parameter.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            self.execution
                .read_primitive_argument(argument, value, &path, scalar_type)
        }
    }

    pub(crate) fn closed_guard(
        proposition: &Proposition,
        integers: &mut ClosedIntegerEvaluator,
    ) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
        Ok(match proposition {
            Proposition::Truth => Some(true),
            Proposition::Falsehood => Some(false),
            Proposition::ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) =
                    (left, right)
                else {
                    return Ok(None);
                };
                use semantic_vocabulary::{
                    IeeeFloatComparisonKind as Kind, IeeeFloatComparisonOperation as Operation,
                };
                let (operation, negate) = match kind {
                    Kind::Equal => (Operation::Equal, false),
                    Kind::NotEqual => (Operation::NotEqual, false),
                    Kind::LessThan => (Operation::Less, false),
                    Kind::LessOrEqual => (Operation::LessOrEqual, false),
                    Kind::NotLessThan => (Operation::Less, true),
                    Kind::NotLessOrEqual => (Operation::LessOrEqual, true),
                };
                Some(
                    crate::engine::scalar_operations::ieee_float_compare(operation, *left, *right)
                        != negate,
                )
            }
            Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
                .boolean_value()
                .zip(right.boolean_value())
                .map(|(left, right)| left == right),
            semantic_vocabulary::ordered_comparison_operands!(left, right) => {
                let Some(((left_type, left), (right_type, right))) =
                    left.integer_value().zip(right.integer_value())
                else {
                    return Ok(None);
                };
                if left_type != right_type {
                    return Ok(None);
                }
                left_type
                    .compare(left, right)
                    .map(|ordering| match proposition {
                        Proposition::Equal(..) => ordering.is_eq(),
                        Proposition::LessThan(..) => ordering.is_lt(),
                        _ => !ordering.is_gt(),
                    })
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => integers
                .compare(left, right)?
                .map(|ordering| match proposition {
                    Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                    Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                }),
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                let conjunction = matches!(proposition, Proposition::Conjunction(_));
                let mut value = conjunction;
                for child in children {
                    let Some(child) = closed_guard(child, integers)? else {
                        return Ok(None);
                    };
                    value = if conjunction {
                        value && child
                    } else {
                        value || child
                    };
                }
                Some(value)
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => match closed_guard(premise, integers)? {
                Some(false) => Some(true),
                Some(true) => closed_guard(conclusion, integers)?,
                None => None,
            },
            _ => None,
        })
    }
}
pub(crate) mod byte_buffers {
    pub(crate) mod writeback {
        use crate::boundary::effects::TerminalEffectRejection;
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::engine::execution::TerminalExecution;
        use crate::storage::byte_sequences::ByteSequenceView;
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;
        use crate::storage::byte_sequences::storage::ByteSequenceStorage;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum BoundaryByteWriteScope {
            View,
            Owner,
        }

        #[derive(Debug)]
        pub struct TerminalBoundaryByteBuffer {
            argument_index: usize,
            capacity: u64,
            scope: BoundaryByteWriteScope,
            bytes: ByteSequenceStorage,
        }

        impl TerminalBoundaryByteBuffer {
            pub fn argument_index(&self) -> usize {
                self.argument_index
            }

            pub fn capacity(&self) -> u64 {
                self.capacity
            }

            pub fn bytes(&self) -> &[u8] {
                self.bytes.bytes()
            }

            pub fn bytes_mut(&mut self) -> &mut [u8] {
                self.bytes.bytes_mut()
            }

            pub fn replace(&mut self, bytes: &[u8]) -> Result<(), TerminalEffectRejection> {
                if self.scope != BoundaryByteWriteScope::Owner {
                    return Err(TerminalEffectRejection::new(
                        "boundary byte view grants no whole-field reaccess authority",
                    ));
                }
                if bytes.len() as u128 > u128::from(self.capacity) {
                    return Err(TerminalEffectRejection::new(
                        "boundary byte replacement exceeds inline capacity",
                    ));
                }
                self.bytes.replace(bytes).map_err(|reason| {
                    TerminalEffectRejection::new(format!("boundary byte storage: {reason:?}"))
                })
            }
        }

        pub(crate) struct BoundaryByteBufferBinding {
            argument_index: usize,
            capacity: u64,
            scope: BoundaryByteWriteScope,
            source: ByteSequenceBinding,
        }

        impl BoundaryByteBufferBinding {
            pub(crate) fn capture(
                execution: &TerminalExecution,
                argument_index: usize,
                source: &ByteSequenceBinding,
                scope: BoundaryByteWriteScope,
            ) -> Result<(Self, TerminalBoundaryByteBuffer), TerminalInterpretError> {
                let (bytes, capacity) = match source {
                    ByteSequenceBinding::MutableArray {
                        array,
                        offset,
                        length,
                        ..
                    } => {
                        if scope != BoundaryByteWriteScope::View {
                            return Err(malformed());
                        }
                        source.validate_presented_referent(execution)?;
                        let start = usize::try_from(*offset).map_err(|_| malformed())?;
                        let end =
                            usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?;
                        let source = execution.stored_byte_array_read_view(array, start, end)?;
                        let mut bytes =
                            ByteSequenceStorage::zeroed(source.len(), execution.account.as_ref())?;
                        source.copy_to(bytes.bytes_mut())?;
                        (bytes, *length)
                    }
                    ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. } => {
                        source.validate_presented_referent(execution)?;
                        let (bytes, capacity) = execution.exclusive_owner_bytes(source)?;
                        let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
                        (
                            ByteSequenceStorage::copy(bytes.bytes(), execution.account.as_ref())?,
                            if scope == BoundaryByteWriteScope::View {
                                length
                            } else {
                                capacity
                            },
                        )
                    }
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        return Err(malformed());
                    }
                };
                Ok((
                    Self {
                        argument_index,
                        capacity,
                        scope,
                        source: source.clone(),
                    },
                    TerminalBoundaryByteBuffer {
                        argument_index,
                        capacity,
                        scope,
                        bytes,
                    },
                ))
            }

            pub(crate) fn validate(
                &self,
                execution: &TerminalExecution,
                buffer: &TerminalBoundaryByteBuffer,
            ) -> Result<(), TerminalInterpretError> {
                if buffer.argument_index != self.argument_index
                    || buffer.capacity != self.capacity
                    || buffer.scope != self.scope
                    || buffer.bytes().len() as u128 > u128::from(self.capacity)
                    || self.scope == BoundaryByteWriteScope::View
                        && buffer.bytes().len() as u128 != u128::from(self.capacity)
                {
                    return Err(malformed());
                }
                let current_capacity = match &self.source {
                    ByteSequenceBinding::MutableArray {
                        array,
                        offset,
                        length,
                        ..
                    } => {
                        if self.scope != BoundaryByteWriteScope::View {
                            return Err(malformed());
                        }
                        self.source.validate_presented_referent(execution)?;
                        let start = usize::try_from(*offset).map_err(|_| malformed())?;
                        let end =
                            usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?;
                        if end > execution.stored_byte_array_length(array)? {
                            return Err(malformed());
                        }
                        for byte_offset in start..end {
                            execution.stored_byte_array_read(array, byte_offset)?;
                        }
                        *length
                    }
                    ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. } => {
                        self.source.validate_presented_referent(execution)?;
                        let (bytes, capacity) = execution.exclusive_owner_bytes(&self.source)?;
                        let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
                        if self.scope == BoundaryByteWriteScope::View {
                            length
                        } else {
                            capacity
                        }
                    }
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        return Err(malformed());
                    }
                };
                if current_capacity != self.capacity {
                    return Err(malformed());
                }
                Ok(())
            }

            pub(crate) fn commit(
                self,
                execution: &mut TerminalExecution,
                buffer: TerminalBoundaryByteBuffer,
            ) -> Result<(), TerminalInterpretError> {
                match self.source {
                    ByteSequenceBinding::MutableArray { array, offset, .. } => {
                        let start = usize::try_from(offset).map_err(|_| malformed())?;
                        for (byte_offset, byte) in buffer.bytes().iter().copied().enumerate() {
                            execution.stored_byte_array_write(
                                &array,
                                start.checked_add(byte_offset).ok_or_else(malformed)?,
                                byte,
                            )?;
                        }
                    }
                    ByteSequenceBinding::MutableField { field, .. } => {
                        execution.structural_byte_sequence_fields.insert(
                            field.clone(),
                            ByteSequenceBinding::Immutable(ByteSequenceView::new(buffer.bytes)),
                        )?;
                        execution.invalidate_stored_provenance(&field.parent);
                    }
                    ByteSequenceBinding::MutableOwner { home, .. } => {
                        *execution.byte_owner_bytes_mut(&home)? =
                            ByteSequenceView::new(buffer.bytes);
                    }
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        return Err(malformed());
                    }
                }
                Ok(())
            }
        }
    }

    use crate::boundary::effects::storage_inputs::BoundaryStorageInputs;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralByteSequenceRuntimeField;
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::calls::arguments::StructuralCallArguments;
    use crate::engine::custody::{
        StructuralArgumentSources, bind_active_retained_arguments, bind_active_structural_arguments,
    };
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::ByteSequenceView;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::runtime_rows::RuntimeRows;
    use crate::storage::structural_identities::LocalStructuralIdentities;
    use evaluation_accounting::EvaluationInvocationAccount;
    use terminal_psi::ByteSequenceCarrier;
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralArgument;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralParameterDeclaration;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeShape;
    pub use writeback::TerminalBoundaryByteBuffer;
    use writeback::{BoundaryByteBufferBinding, BoundaryByteWriteScope};

    pub(crate) struct BoundaryArguments {
        local_identity_cursor: Option<u64>,
        pub(crate) values: Vec<TerminalStructuralValue>,
        aggregate_backings: Vec<Option<crate::storage::reference::AggregateBacking>>,
        pub(crate) bytes: Vec<Option<Vec<u8>>>,
        pub(crate) buffers: Vec<TerminalBoundaryByteBuffer>,
        pub(crate) storage_inputs: BoundaryStorageInputs,
        bindings: Vec<BoundaryByteBufferBinding>,
        byte_sequences: Vec<Option<ByteSequenceBinding>>,
        primitive_loans: Vec<Option<crate::storage::primitive_storage::PrimitiveLoan>>,
        element_views: Vec<Option<crate::storage::element_views::ElementView>>,
    }

    impl BoundaryArguments {
        pub(crate) fn opaque_values(
            &self,
            original_identity_cursor: Option<u64>,
        ) -> Result<&[TerminalStructuralValue], TerminalInterpretError> {
            if self.local_identity_cursor != original_identity_cursor
                || self.aggregate_backings.iter().any(Option::is_some)
                || self.bytes.iter().any(Option::is_some)
                || !self.buffers.is_empty()
                || !self.bindings.is_empty()
                || !self.storage_inputs.is_empty()
                || self.byte_sequences.iter().any(Option::is_some)
                || self.primitive_loans.iter().any(Option::is_some)
                || self.element_views.iter().any(Option::is_some)
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "opaque argument transport cannot discard storage or loan companions",
                ));
            }
            Ok(&self.values)
        }

        pub(crate) fn reserve_host_result(
            &self,
            identities: &mut LocalStructuralIdentities,
            value: &TerminalStructuralValue,
        ) -> Result<(), TerminalInterpretError> {
            identities.reserve_host_at_cursor(value, self.local_identity_cursor)
        }

        pub(crate) fn into_call_arguments(
            mut self,
            parameters: &[StructuralParameterDeclaration],
            types: &crate::embedding::program::ProgramDeclarations<
                '_,
                terminal_psi::StructuralTypeDeclaration,
            >,
            cases: &std::collections::BTreeMap<
                crate::boundary::values::StructuralRuntimePlace,
                crate::boundary::structural_inputs::case_membership::StructuralCaseContents,
            >,
            sources: Option<StructuralArgumentSources<'_, '_>>,
            account: Option<&EvaluationInvocationAccount>,
        ) -> Result<StructuralCallArguments, TerminalInterpretError> {
            if parameters.len() != self.byte_sequences.len()
                || parameters.len() != self.values.len()
                || parameters.len() != self.aggregate_backings.len()
            {
                return Err(malformed());
            }
            for ((parameter, value), binding) in parameters
                .iter()
                .zip(&mut self.values)
                .zip(&self.byte_sequences)
            {
                if parameter.access == StructuralAccess::Owned && binding.is_some() {
                    value.absorb_qualifications(&parameter.qualifications);
                }
            }
            let values = match sources {
                Some(sources) => bind_active_retained_arguments(
                    parameters,
                    sources.arguments,
                    &self.values,
                    sources.provenance,
                    &|argument_index, rows| {
                        crate::boundary::structural_inputs::case_membership::active_projected_qualifications(
                            types,
                            &self.values[argument_index],
                            cases,
                            rows,
                        )
                    },
                    types,
                    account,
                ),
                None => bind_active_structural_arguments(
                    parameters,
                    &self.values,
                    types,
                    cases,
                    account,
                ),
            }
            .map_err(|error| error.in_argument_preparation("bind source memberships"))?;
            let aggregate_backings = RuntimeRows::from_rows(
                account,
                parameters.iter().zip(self.aggregate_backings).filter_map(
                    |(parameter, backing)| backing.map(|backing| (parameter.place, backing)),
                ),
            )?;
            let mut byte_sequences = RuntimeRows::new(account);
            let mut primitive_loans = RuntimeRows::new(account);
            for (parameter, loan) in parameters.iter().zip(self.primitive_loans) {
                if let Some(loan) = loan
                    && (values.get(&parameter.place) != Some(&loan.referent)
                        || loan.access != parameter.access
                        || primitive_loans.insert(parameter.place, loan)?.is_some())
                {
                    return Err(malformed());
                }
            }
            let mut element_views = RuntimeRows::new(account);
            for (parameter, view) in parameters.iter().zip(self.element_views) {
                let Some(view) = view else {
                    continue;
                };
                let admitted = if parameter.access == StructuralAccess::Owned {
                    view.owned_multiplicity() == Some(parameter.multiplicity)
                        && view.matches_descriptor_home(
                            values.get(&parameter.place).ok_or_else(malformed)?,
                        )
                } else {
                    terminal_semantics::borrowed_storage_reference_ceiling(
                        parameter,
                        types.values(),
                    )
                    .unwrap_or(parameter.access)
                        == view.access()
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && if matches!(
                            types.get(&parameter.structural_type).map(|row| &row.shape),
                            Some(StructuralTypeShape::FixedArray { .. })
                        ) {
                            crate::storage::scalar_array::borrowed_array_memberships_match(
                                values.get(&parameter.place).ok_or_else(malformed)?,
                                parameter,
                            )
                        } else {
                            parameter.qualifications.is_empty()
                                && terminal_semantics::view_descriptor_projected_qualifications(
                                    types.values(),
                                    parameter.structural_type,
                                    &parameter.projected_qualifications,
                                )
                        }
                        && view.matches_descriptor_home(
                            values.get(&parameter.place).ok_or_else(malformed)?,
                        )
                };
                if !admitted || element_views.insert(parameter.place, view)?.is_some() {
                    return Err(malformed().in_argument_preparation("admit element-view companion"));
                }
            }
            for (parameter, binding) in parameters.iter().zip(self.byte_sequences) {
                let Some(binding) = binding else {
                    continue;
                };
                let access = match &binding {
                    ByteSequenceBinding::Immutable(view)
                        if parameter.access == StructuralAccess::Owned
                            && view.matches_descriptor_home(
                                values.get(&parameter.place).ok_or_else(malformed)?,
                            ) =>
                    {
                        StructuralAccess::Owned
                    }
                    ByteSequenceBinding::Representation(view)
                        if parameter.access == StructuralAccess::Owned
                            && view.matches_descriptor_home(
                                values.get(&parameter.place).ok_or_else(malformed)?,
                            ) =>
                    {
                        StructuralAccess::Owned
                    }
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        StructuralAccess::SharedBorrow
                    }
                    ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. }
                        if terminal_semantics::borrowed_storage_reference_ceiling(
                            parameter,
                            types.values(),
                        )
                        .unwrap_or(parameter.access)
                            == StructuralAccess::WriteOnlyBorrow =>
                    {
                        StructuralAccess::WriteOnlyBorrow
                    }
                    ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. }
                    | ByteSequenceBinding::MutableArray { .. } => StructuralAccess::MutableBorrow,
                };
                if terminal_semantics::borrowed_storage_reference_ceiling(parameter, types.values())
                    .unwrap_or(parameter.access)
                    != access
                    || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        && !terminal_semantics::view_descriptor_parameter(
                            parameter,
                            types.values(),
                        ))
                    || !parameter.projected_qualifications.is_empty()
                    || match &binding {
                        ByteSequenceBinding::MutableField { referent, .. }
                        | ByteSequenceBinding::MutableOwner { referent, .. } => {
                            referent.qualifications != parameter.qualifications
                        }
                        ByteSequenceBinding::Immutable(_)
                        | ByteSequenceBinding::Representation(_) => false,
                        ByteSequenceBinding::MutableArray { referent, .. } => {
                            referent.qualifications != parameter.qualifications
                        }
                    }
                    || byte_sequences.insert(parameter.place, binding)?.is_some()
                {
                    return Err(malformed());
                }
            }
            Ok(StructuralCallArguments {
                aggregate_backings,
                local_identity_cursor: self.local_identity_cursor,
                descriptor_parameters: parameters.to_vec(),
                values,
                byte_sequences,
                primitive_loans,
                scalar_arrays: crate::storage::scalar_array::ScalarArrayBindings::new(account),
                element_views,
            })
        }

        pub(crate) fn capture_reference_inputs(
            &mut self,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            arguments: &[StructuralArgument],
        ) -> Result<(), TerminalInterpretError> {
            self.storage_inputs
                .capture_reference_inputs(declaration, arguments, &self.values)
        }

        pub(crate) fn prepare_reference_result(
            &mut self,
            execution: &TerminalExecution,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            result: &terminal_psi::StructuralOperationResult,
            selection: &super::storage_inputs::TerminalBoundaryReferenceSelection,
        ) -> Result<crate::storage::reference::PreparedReference, TerminalInterpretError> {
            self.storage_inputs.prepare_reference_result(
                execution,
                declaration,
                result,
                selection,
                &mut self.local_identity_cursor,
            )
        }

        pub(crate) fn validate_writeback(
            &mut self,
            execution: &TerminalExecution,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
        ) -> Result<(), TerminalInterpretError> {
            if self.buffers.len() != self.bindings.len() {
                return Err(malformed());
            }
            for (buffer, binding) in self.buffers.iter().zip(&self.bindings) {
                binding.validate(execution, buffer)?;
            }
            self.storage_inputs.validate_writeback(
                execution,
                declaration,
                &mut self.local_identity_cursor,
            )
        }

        pub(crate) fn commit(
            self,
            execution: &mut TerminalExecution,
        ) -> Result<(), TerminalInterpretError> {
            for (buffer, binding) in self.buffers.into_iter().zip(self.bindings) {
                binding.commit(execution, buffer)?;
            }
            self.storage_inputs.commit(execution)
        }

        pub(crate) fn prepare_original_storage(
            &mut self,
            execution: &mut TerminalExecution,
            handler: &mut impl crate::boundary::effects::TerminalEffectHandler,
            boundary: semantic_vocabulary::BoundaryMachineId,
            original: bool,
        ) -> Result<(), TerminalInterpretError> {
            self.storage_inputs.prepare_original_storage(
                execution,
                handler,
                boundary,
                &mut self.local_identity_cursor,
                original,
            )
        }

        pub(crate) fn commit_identity_cursor(&self, execution: &mut TerminalExecution) {
            execution
                .local_structural_identities
                .commit_cursor(self.local_identity_cursor);
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn resolve_boundary_arguments(
            &self,
            parameters: &[StructuralParameterDeclaration],
            arguments: &[StructuralArgument],
            storage_arguments: &[usize],
            normal_qualifications: &[terminal_psi::StructuralDomainRequirement],
            original: bool,
        ) -> Result<BoundaryArguments, TerminalInterpretError> {
            if parameters.len() != arguments.len() {
                return Err(malformed());
            }
            let mut ordinary_parameters = Vec::new();
            let mut ordinary_arguments = Vec::new();
            let mut scalar_indices = Vec::new();
            for (argument_index, (parameter, argument)) in
                parameters.iter().zip(arguments).enumerate()
            {
                let scalar_loan = matches!(
                    self.structural_declaration(parameter.structural_type)?
                        .shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                ) && parameter.access != StructuralAccess::Owned;
                if scalar_loan {
                    scalar_indices.push(argument_index);
                } else {
                    ordinary_parameters.push(parameter.clone());
                    ordinary_arguments.push(argument.clone());
                }
            }
            let mut resolved =
                self.prepare_boundary_arguments(&ordinary_parameters, &ordinary_arguments)?;
            for argument_index in scalar_indices {
                let value = resolved.storage_inputs.capture_scalar(
                    self,
                    &parameters[argument_index],
                    &arguments[argument_index],
                    argument_index,
                    &mut resolved.local_identity_cursor,
                )?;
                resolved.values.insert(argument_index, value);
                resolved.aggregate_backings.insert(argument_index, None);
                resolved.byte_sequences.insert(argument_index, None);
                resolved.primitive_loans.insert(argument_index, None);
                resolved.element_views.insert(argument_index, None);
            }
            let mut requested = std::collections::BTreeSet::new();
            for &argument_index in storage_arguments {
                if !requested.insert(argument_index) {
                    return Err(malformed());
                }
                let parameter = parameters.get(argument_index).ok_or_else(malformed)?;
                let argument = arguments.get(argument_index).ok_or_else(malformed)?;
                if original
                    && matches!(
                        self.structural_declaration(parameter.structural_type)?
                            .shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
                    )
                {
                    resolved.storage_inputs.capture_byte_view(
                        self,
                        parameter,
                        &resolved.values[argument_index],
                        resolved.byte_sequences[argument_index]
                            .as_ref()
                            .ok_or_else(malformed)?,
                        argument_index,
                    )?;
                    continue;
                }
                if !matches!(
                    parameter.access,
                    StructuralAccess::Owned | terminal_psi::reading_borrow_pair_pattern!()
                ) {
                    return Err(malformed());
                }
                if parameter.access == StructuralAccess::Owned
                    || !matches!(
                        self.structural_declaration(parameter.structural_type)?
                            .shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    )
                {
                    resolved
                        .storage_inputs
                        .capture_fields(
                            self,
                            parameter,
                            argument,
                            &resolved.values[argument_index],
                            resolved.element_views[argument_index].as_ref(),
                            resolved.aggregate_backings[argument_index].as_ref(),
                            argument_index,
                            normal_qualifications,
                        )
                        .map_err(|error| {
                            error.in_argument_preparation("capture host storage cells")
                        })?;
                }
            }
            for (argument_index, binding) in resolved.byte_sequences.iter().enumerate() {
                if resolved.storage_inputs.buffers.iter().any(|buffer| matches!(buffer,
                crate::TerminalBoundaryStorageBuffer::ByteView(buffer) if buffer.argument_index() == argument_index)) {
                resolved.bytes.push(None);
                continue;
            }
                match binding {
                    None => resolved.bytes.push(None),
                    Some(ByteSequenceBinding::Immutable(view)) => {
                        resolved.bytes.push(Some(view.bytes().to_vec()));
                    }
                    Some(binding @ ByteSequenceBinding::Representation(_)) => {
                        resolved
                            .bytes
                            .push(Some(binding.snapshot(self)?.bytes().to_vec()));
                    }
                    Some(
                        binding @ (ByteSequenceBinding::MutableArray { .. }
                        | ByteSequenceBinding::MutableField { .. }
                        | ByteSequenceBinding::MutableOwner { .. }),
                    ) => {
                        let parameter = &parameters[argument_index];
                        let declaration = self.structural_declaration(parameter.structural_type)?;
                        let scope = match declaration.shape {
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { .. },
                            ) => BoundaryByteWriteScope::View,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. },
                            ) => BoundaryByteWriteScope::Owner,
                            _ => return Err(malformed()),
                        };
                        let (binding, buffer) = BoundaryByteBufferBinding::capture(
                            self,
                            argument_index,
                            binding,
                            scope,
                        )?;
                        resolved.bytes.push(Some(buffer.bytes().to_vec()));
                        resolved.buffers.push(buffer);
                        resolved.bindings.push(binding);
                    }
                }
            }
            Ok(resolved)
        }

        pub(crate) fn prepare_boundary_arguments(
            &self,
            parameters: &[StructuralParameterDeclaration],
            arguments: &[StructuralArgument],
        ) -> Result<BoundaryArguments, TerminalInterpretError> {
            if parameters.len() != arguments.len() {
                return Err(malformed());
            }
            let mut resolved = BoundaryArguments {
                local_identity_cursor: self.local_structural_identities.cursor(),
                values: Vec::with_capacity(arguments.len()),
                aggregate_backings: Vec::with_capacity(arguments.len()),
                bytes: Vec::with_capacity(arguments.len()),
                buffers: Vec::new(),
                storage_inputs: BoundaryStorageInputs::default(),
                bindings: Vec::new(),
                byte_sequences: Vec::with_capacity(arguments.len()),
                primitive_loans: Vec::with_capacity(arguments.len()),
                element_views: Vec::with_capacity(arguments.len()),
            };
            let mut local_array_roots = RuntimeRows::new(self.account.as_ref());
            for (parameter, argument) in parameters.iter().zip(arguments) {
                self.require_owned_value_claims(argument)?;
                if matches!(
                    self.structural_declaration(parameter.structural_type)?
                        .shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                ) && parameter.access != StructuralAccess::Owned
                {
                    let loan = self.capture_primitive_loan(
                        parameter,
                        argument,
                        &mut resolved.local_identity_cursor,
                    )?;
                    resolved.values.push(loan.referent.clone());
                    resolved.aggregate_backings.push(None);
                    resolved.primitive_loans.push(Some(loan));
                    resolved.byte_sequences.push(None);
                    resolved.element_views.push(None);
                    continue;
                }
                resolved.primitive_loans.push(None);
                if let Some((referent, loan)) = self
                    .prepare_scalar_array_loan(
                        parameter,
                        argument,
                        &mut resolved.local_identity_cursor,
                        &mut local_array_roots,
                    )
                    .map_err(|error| error.in_argument_preparation("prepare array backing loan"))?
                {
                    resolved.values.push(referent);
                    resolved.aggregate_backings.push(None);
                    resolved.byte_sequences.push(None);
                    resolved.element_views.push(Some(loan));
                    continue;
                }
                if let Some((referent, binding)) = self
                    .prepare_byte_sequence_loan(
                        parameter,
                        argument,
                        &mut resolved.local_identity_cursor,
                        &mut local_array_roots,
                    )
                    .map_err(|error| error.in_argument_preparation("prepare byte backing loan"))?
                {
                    resolved.values.push(referent);
                    resolved.aggregate_backings.push(None);
                    resolved.byte_sequences.push(Some(binding));
                    resolved.element_views.push(None);
                    continue;
                }
                let (value, backing) = self
                    .present_structural_argument(argument, parameter.structural_type)
                    .map_err(|error| error.in_argument_preparation("resolve source storage"))?;
                let values = [value];
                resolved.aggregate_backings.push(backing);
                let mut bytes = self
                    .bind_byte_sequence_arguments(
                        std::slice::from_ref(parameter),
                        std::slice::from_ref(argument),
                        &values,
                    )
                    .map_err(|error| error.in_argument_preparation("bind byte descriptor"))?;
                let binding = bytes.remove(&parameter.place);
                resolved.byte_sequences.push(binding);
                let mut element_views = self
                    .bind_element_view_arguments(
                        std::slice::from_ref(parameter),
                        std::slice::from_ref(argument),
                        &values,
                    )
                    .map_err(|error| error.in_argument_preparation("bind element descriptor"))?;
                resolved
                    .element_views
                    .push(element_views.remove(&parameter.place));
                resolved.values.extend(values);
            }
            Ok(resolved)
        }

        pub(crate) fn prepare_byte_sequence_loan(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            identity_cursor: &mut Option<u64>,
            local_array_roots: &mut RuntimeRows<
                semantic_vocabulary::PlaceId,
                TerminalStructuralValue,
            >,
        ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
        {
            if terminal_semantics::borrowed_storage_parameter(
                parameter,
                self.structural_types().values(),
            ) && self
                .structural_types()
                .get(&parameter.structural_type)
                .is_some_and(|row| {
                    terminal_semantics::stored_reference_access(&row.shape).is_some()
                })
                && terminal_semantics::borrowed_storage_reference_ceiling(
                    parameter,
                    self.structural_types().values(),
                )
                .is_none()
            {
                return Ok(None);
            }
            if let Some(prepared) = self
                .prepare_byte_descriptor_argument(parameter, argument)
                .map_err(|error| {
                    error.in_argument_preparation("prepare resident byte descriptor")
                })?
            {
                return Ok(Some(prepared));
            }
            if let Some(prepared) = self
                .prepare_owner_view_argument(parameter, argument)
                .map_err(|error| error.in_argument_preparation("prepare byte owner view"))?
            {
                return Ok(Some(prepared));
            }
            if let Some(prepared) = self
                .prepare_array_view_argument_staged(
                    parameter,
                    argument,
                    identity_cursor,
                    local_array_roots,
                )
                .map_err(|error| error.in_argument_preparation("prepare byte array view"))?
            {
                return Ok(Some(prepared));
            }
            let declaration = self.structural_declaration(parameter.structural_type)?;
            if declaration.shape.is_bounded_owned_byte_sequence()
                && matches!(
                    parameter.access,
                    terminal_psi::exclusive_borrow_pair_pattern!()
                )
            {
                if argument.access != parameter.access
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                let binding = self.resolve_mutable_boundary_binding(parameter, argument)?;
                let referent = binding.owner_loan_referent().ok_or_else(malformed)?.clone();
                return Ok(Some((referent, binding)));
            }
            if parameter.access == StructuralAccess::SharedBorrow
                && declaration.shape.is_bounded_owned_byte_sequence()
            {
                if argument.access != StructuralAccess::SharedBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                return self
                    .resolve_shared_owner_binding(parameter, argument)
                    .map(Some);
            }
            if parameter.access == StructuralAccess::SharedBorrow
                && declaration.shape.is_borrowed_byte_view()
                && !argument.path.is_empty()
            {
                if argument.access != StructuralAccess::SharedBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                let (referent, binding) = self
                    .resolve_shared_boundary_binding(parameter, argument)
                    .map_err(|error| error.in_argument_preparation("prepare shared byte field"))?;
                if !self.byte_binding_qualifications_hold(
                    &binding,
                    parameter.structural_type,
                    &parameter.qualifications,
                )? {
                    return Err(malformed());
                }
                return Ok(Some((referent, binding)));
            }
            if parameter.access == StructuralAccess::MutableBorrow
                && declaration.shape.is_borrowed_byte_view()
            {
                if argument.access != StructuralAccess::MutableBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                let binding = self.resolve_mutable_boundary_binding(parameter, argument)?;
                let ByteSequenceBinding::MutableField {
                    field: destination,
                    capacity,
                    referent,
                    ..
                } = &binding
                else {
                    return Err(malformed());
                };
                let bytes = self
                    .structural_byte_sequence_fields
                    .require(destination)?
                    .immutable()?
                    .bytes();
                if bytes.len() as u128 > u128::from(*capacity) {
                    return Err(malformed());
                }
                return Ok(Some((referent.clone(), binding)));
            }
            Ok(None)
        }

        fn prepare_owner_view_argument(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
        ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
        {
            let types = self.structural_types();
            if parameter.access != StructuralAccess::SharedBorrow
                || !argument.path.is_empty()
                || !types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
            {
                return Ok(None);
            }
            let Some(owner) = self.structural_values.get(&argument.place) else {
                return Ok(None);
            };
            if !types
                .get(&owner.structural_type)
                .is_some_and(|declaration| declaration.is_bounded_owned_byte_sequence())
            {
                return Ok(None);
            }
            if argument.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
                || crate::engine::custody::place_is_claimed(&self.live_claims, argument.place)
            {
                return Err(malformed());
            }
            let (_, bytes) = self.bounded_owner_snapshot(argument)?;
            if !terminal_semantics::byte_literal_qualifications_hold_in(
                self.program.catalogs.structural_domains.iter(),
                parameter.structural_type,
                &parameter.qualifications,
                bytes.bytes(),
            ) {
                return Err(malformed());
            }
            let mut view = owner.clone();
            view.structural_type = parameter.structural_type;
            view.qualifications = parameter.qualifications.clone();
            let bytes = bytes.with_descriptor_home(view.clone());
            Ok(Some((view, ByteSequenceBinding::Immutable(bytes))))
        }

        fn resolve_mutable_boundary_binding(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
        ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
            if let Some(value) = self.declared_byte_owner(argument)? {
                if value.structural_type != parameter.structural_type {
                    return Err(malformed());
                }
                return self.lend_declared_owner(parameter, &value);
            }
            let (mut parent, identity, field, namespace) =
                self.structural_field_binding_frame(argument)?;
            let Some(ByteSequenceCarrier::BoundedOwned { capacity }) =
                field.field_type.byte_sequence_carrier()
            else {
                return Err(malformed());
            };
            let destination = StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                namespace,
                field: field.id,
            };

            let parent_type = parent.structural_type;
            parent.structural_type = parameter.structural_type;
            parent.qualifications = parameter.qualifications.clone();
            parent.projected_qualifications = parameter.projected_qualifications.clone();
            parent.path.push(StructuralPathSegment::Field(identity));

            let binding = ByteSequenceBinding::MutableField {
                field: destination,
                capacity,
                parent_type,
                referent: parent,
                descriptor_home: None,
            };
            binding.validate_mutable_referent(
                self,
                binding.owner_loan_referent().ok_or_else(malformed)?,
            )?;
            Ok(binding)
        }

        fn lend_declared_owner(
            &self,
            parameter: &StructuralParameterDeclaration,
            owner: &TerminalStructuralValue,
        ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
            let coordinate = StructuralRuntimePlace::from(owner);
            let Some(ByteSequenceCarrier::BoundedOwned { capacity }) = self
                .structural_types()
                .get(&owner.structural_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                    _ => None,
                })
            else {
                return Err(malformed());
            };
            if owner.qualifications != parameter.qualifications {
                return Err(malformed());
            }
            if let Some(binding) = self.captured_byte_owner_binding(owner) {
                binding.validate_mutable_referent(self, owner)?;
                return Ok(binding.clone());
            }
            let home = if self.structural_byte_storage.contains_key(&coordinate) {
                crate::storage::byte_sequences::binding::ByteOwnerHome::Storage(coordinate)
            } else {
                let frames = std::iter::once((
                    self.call_stack.len(),
                    self.current_machine,
                    &self.structural_values,
                    &self.byte_sequence_values,
                ))
                .chain(self.call_stack.iter().enumerate().map(
                    |(depth, frame)| {
                        (
                            depth,
                            frame.current_machine,
                            &frame.structural_values,
                            &frame.byte_sequence_values,
                        )
                    },
                ));
                let mut selected_home = None;
                for (depth, machine, values, bindings) in frames {
                    for (place, value) in values {
                        if StructuralRuntimePlace::from(value) != coordinate
                            || value.structural_type != owner.structural_type
                        {
                            continue;
                        }
                        let Some(binding) = bindings.get(place) else {
                            continue;
                        };
                        match binding {
                            ByteSequenceBinding::MutableOwner { .. }
                            | ByteSequenceBinding::MutableField { .. } => {
                                binding.validate_mutable_referent(self, owner)?;
                                return Ok(binding.clone());
                            }
                            ByteSequenceBinding::Immutable(_) if value.path.is_empty() => {
                                let declaration = self.program.machine(machine)?;
                                let owns_storage = declaration
                                    .structural_parameter(*place)
                                    .is_some_and(|parameter| {
                                        parameter.access == StructuralAccess::Owned
                                            && parameter.structural_type == owner.structural_type
                                    })
                                    || declaration
                                        .structural_place(*place)
                                        .and_then(|place| place.kind.as_operation_result())
                                        .is_some_and(|(_, structural_type)| {
                                            structural_type == owner.structural_type
                                        });
                                if owns_storage {
                                    selected_home = Some(
                                    crate::storage::byte_sequences::binding::ByteOwnerHome::Frame {
                                        depth,
                                        machine,
                                        place: *place,
                                    },
                                );
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if selected_home.is_some() {
                        break;
                    }
                }
                selected_home.ok_or_else(malformed)?
            };
            let binding = ByteSequenceBinding::MutableOwner {
                home,
                capacity,
                referent: owner.clone(),
                descriptor_home: None,
            };
            binding.validate_mutable_referent(self, owner)?;
            Ok(binding)
        }

        fn resolve_shared_owner_binding(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
        ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError>
        {
            let Some(value) = self.declared_byte_owner(argument)? else {
                return self.resolve_shared_boundary_binding(parameter, argument);
            };
            if value.structural_type != parameter.structural_type
                || parameter
                    .qualifications
                    .iter()
                    .any(|domain| !value.qualifications.contains(domain))
            {
                return Err(malformed());
            }
            let (_, bytes) = self.bounded_owner_snapshot(argument)?;
            Ok((
                value.clone(),
                ByteSequenceBinding::Immutable(bytes.with_descriptor_home(value.clone())),
            ))
        }

        pub(crate) fn bounded_owner_snapshot(
            &self,
            argument: &StructuralArgument,
        ) -> Result<(StructuralRuntimePlace, ByteSequenceView), TerminalInterpretError> {
            if argument.access != StructuralAccess::SharedBorrow {
                return Err(malformed());
            }
            let (coordinate, bytes, capacity) = if let Some(value) =
                self.declared_byte_owner(argument)?
            {
                let Some(StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity,
                })) = self
                    .structural_types()
                    .get(&value.structural_type)
                    .map(|row| &row.shape)
                else {
                    return Err(malformed());
                };
                let binding = self.byte_descriptor_binding(argument, &value)?;
                let ByteSequenceBinding::Immutable(bytes) = self.reborrow_byte_sequence_binding(
                    &binding,
                    &value,
                    StructuralAccess::SharedBorrow,
                )?
                else {
                    return Err(malformed());
                };
                (StructuralRuntimePlace::from(&value), bytes, *capacity)
            } else {
                let (mut parent, identity, field, namespace) =
                    self.structural_field_binding_frame(argument)?;
                let Some(ByteSequenceCarrier::BoundedOwned { capacity }) =
                    field.field_type.byte_sequence_carrier()
                else {
                    return Err(malformed());
                };
                let bytes = self
                    .structural_byte_sequence_fields
                    .get(&StructuralByteSequenceRuntimeField {
                        parent: StructuralRuntimePlace::from(&parent),
                        namespace,
                        field: field.id,
                    })
                    .ok_or_else(malformed)?
                    .immutable()?
                    .clone();
                parent.path.push(StructuralPathSegment::Field(identity));
                (StructuralRuntimePlace::from(&parent), bytes, capacity)
            };
            if bytes.len() as u128 > u128::from(capacity) {
                return Err(malformed());
            }
            Ok((coordinate, bytes))
        }

        fn resolve_shared_boundary_binding(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
        ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError>
        {
            let (mut parent, identity, field, namespace) =
                self.structural_field_binding_frame(argument)?;
            let carrier = field
                .field_type
                .byte_sequence_carrier()
                .ok_or_else(malformed)?;
            if !carrier.is_owned_or_shared_byte_carrier() {
                return Err(malformed());
            }
            let source = StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                namespace,
                field: field.id,
            };
            let bytes = self
                .structural_byte_sequence_fields
                .require(&source)?
                .immutable()?;
            let bytes = match carrier {
                ByteSequenceCarrier::BorrowedView { .. } => bytes.clone(),
                ByteSequenceCarrier::BoundedOwned { .. } => ByteSequenceView::new(
                    ByteSequenceStorage::copy(bytes.bytes(), self.account.as_ref())?,
                ),
                ByteSequenceCarrier::BoundedView { .. } => {
                    return Err(malformed());
                }
            };
            parent.structural_type = parameter.structural_type;
            parent.qualifications = parameter.qualifications.clone();
            parent.projected_qualifications = parameter.projected_qualifications.clone();
            parent.path.push(StructuralPathSegment::Field(identity));
            let bytes = bytes.with_descriptor_home(parent.clone());
            Ok((parent, ByteSequenceBinding::Immutable(bytes)))
        }

        fn structural_field_binding_frame<'a>(
            &'a self,
            argument: &'a StructuralArgument,
        ) -> Result<
            (
                TerminalStructuralValue,
                terminal_psi::TerminalIdentityIndex,
                &'a terminal_psi::StructuralFieldDeclaration,
                crate::boundary::values::StructuralFieldNamespace,
            ),
            TerminalInterpretError,
        > {
            let Some((StructuralPathSegment::Field(identity), prefix)) = argument.path.split_last()
            else {
                return Err(malformed());
            };
            let (prefix, case) = match prefix.split_last() {
                Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
                _ => (prefix, None),
            };
            let parent = self.resolve_live_projection(argument.place, prefix, argument.access)?;
            let (fields, namespace) = self.active_field_declarations(
                parent.structural_type,
                &StructuralRuntimePlace::from(&parent),
                case,
            )?;
            let field =
                terminal_psi::live_field_by_identity(fields, *identity).ok_or_else(malformed)?;
            Ok((parent, *identity, field, namespace))
        }
    }
}
pub(crate) mod crash_outcome {
    use super::boundary_predicates::closed_guard;
    use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
    use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};
    use terminal_psi::{BoundaryMachineDeclaration, CrashCause, CrashRouteGuard};
    use terminal_semantics::substitution::substitute_proposition_values;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum BoundaryCrashOutcomeError {
        InvalidDeclaration,
        InvalidArguments,
        UndeclaredCause,
        GuardFalse,
        UnsupportedGuard,
        GuardEvaluationLimitExceeded,
    }

    pub fn validate_boundary_crash_outcome(
        boundary: &BoundaryMachineDeclaration,
        arguments: &[ScalarTerm],
        cause: CrashCause,
    ) -> Result<(), BoundaryCrashOutcomeError> {
        proof_admission::check_predicate_evaluation_size(
            boundary
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => &Proposition::Truth,
                    CrashRouteGuard::Predicate(predicate) => predicate.proposition(),
                }),
        )
        .map_err(|_| BoundaryCrashOutcomeError::GuardEvaluationLimitExceeded)?;
        validate_boundary_crash_routes(boundary)?;
        if arguments.len() != boundary.scalar_parameters.len()
            || arguments
                .iter()
                .zip(&boundary.scalar_parameters)
                .any(|(argument, expected)| {
                    !matches!(
                        argument,
                        ScalarTerm::Boolean(_)
                            | ScalarTerm::Integer { .. }
                            | ScalarTerm::IeeeFloatLiteral(_)
                    ) || argument.scalar_type() != *expected
                        || argument.validate().is_err()
                })
        {
            return Err(BoundaryCrashOutcomeError::InvalidArguments);
        }
        let bucket = boundary
            .crash_routes
            .iter()
            .find(|bucket| bucket.cause == cause)
            .ok_or(BoundaryCrashOutcomeError::UndeclaredCause)?;
        let parameters = boundary
            .scalar_contract_parameters()
            .ok_or(BoundaryCrashOutcomeError::InvalidArguments)?;
        let substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.id, argument.clone()))
            .collect();
        let mut unsupported = false;
        let mut exhausted = false;
        let mut integers = ClosedIntegerEvaluator::default();
        for guard in &bucket.alternatives {
            let value = match guard {
                CrashRouteGuard::Truth => Ok(Some(true)),
                CrashRouteGuard::Predicate(predicate) => closed_guard(
                    &substitute_proposition_values(predicate.proposition(), &substitutions),
                    &mut integers,
                ),
            };
            match value {
                Ok(Some(true)) => return Ok(()),
                Ok(Some(false)) => {}
                Ok(None) => unsupported = true,
                Err(ClosedIntegerEvaluationError::ResourceLimitExceeded) => exhausted = true,
            }
        }
        Err(if exhausted {
            BoundaryCrashOutcomeError::GuardEvaluationLimitExceeded
        } else if unsupported {
            BoundaryCrashOutcomeError::UnsupportedGuard
        } else {
            BoundaryCrashOutcomeError::GuardFalse
        })
    }

    fn validate_boundary_crash_routes(
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), BoundaryCrashOutcomeError> {
        if boundary
            .crash_routes
            .windows(2)
            .any(|pair| pair[0].cause >= pair[1].cause)
            || boundary.crash_routes.iter().any(|bucket| {
                bucket.alternatives.is_empty()
                    || bucket
                        .alternatives
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                        && bucket.alternatives != [CrashRouteGuard::Truth])
                    || bucket.alternatives.iter().any(|guard| {
                        matches!(guard, CrashRouteGuard::Predicate(predicate)
                        if predicate.proposition().validate().is_err())
                    })
            })
        {
            return Err(BoundaryCrashOutcomeError::InvalidDeclaration);
        }
        if boundary.crash_routes.is_empty() {
            return Ok(());
        }
        let context = PropositionContext::from_value_types(
            boundary
                .scalar_contract_parameters()
                .ok_or(BoundaryCrashOutcomeError::InvalidDeclaration)?
                .iter()
                .map(|parameter| (parameter.id, parameter.scalar_type)),
        )
        .map_err(|_| BoundaryCrashOutcomeError::InvalidDeclaration)?;
        for predicate in boundary
            .crash_routes
            .iter()
            .flat_map(|bucket| &bucket.alternatives)
            .filter_map(|guard| match guard {
                CrashRouteGuard::Truth => None,
                CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
            })
        {
            if matches!(predicate, Proposition::Truth | Proposition::Falsehood)
                || !predicate.visit_value_ids(|_| {})
                || context.validate(predicate).is_err()
            {
                return Err(BoundaryCrashOutcomeError::InvalidDeclaration);
            }
        }
        Ok(())
    }
}
pub(crate) mod normal_qualifications {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::engine::execution::TerminalExecution;
    use semantic_vocabulary::{StructuralCaseId, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralPathQualification, StructuralPathSegment};

    #[derive(Debug, Clone)]
    pub(crate) struct CapturedNormalQualification {
        pub(crate) path: Vec<StructuralPathSegment>,
        subject: CapturedQualificationSubject,
        carrier: StructuralTypeId,
        domain: StructuralDomainId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum CapturedQualificationSubject {
        Active(StructuralRuntimePlace),
        Inactive {
            guard: StructuralRuntimePlace,
            guard_carrier: StructuralTypeId,
            suffix: Vec<StructuralPathSegment>,
        },
    }

    enum NormalQualificationSubject {
        Active(TerminalStructuralValue),
        Inactive(CapturedQualificationSubject),
    }

    impl NormalQualificationSubject {
        fn captured(&self) -> CapturedQualificationSubject {
            match self {
                Self::Active(value) => {
                    CapturedQualificationSubject::Active(StructuralRuntimePlace::from(value))
                }
                Self::Inactive(subject) => subject.clone(),
            }
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn normal_qualifications_cover(
            &self,
            value: &TerminalStructuralValue,
            whole: &[StructuralDomainId],
            projected: &[StructuralPathQualification],
            captured: &[CapturedNormalQualification],
        ) -> Result<bool, TerminalInterpretError> {
            for domain in whole {
                if !self.normal_qualification_is_covered(value, &[], *domain, captured)? {
                    return Ok(false);
                }
            }
            for row in projected {
                if !self.normal_qualification_is_covered(value, &row.path, row.domain, captured)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        pub(crate) fn capture_normal_qualifications(
            &self,
            owner: &TerminalStructuralValue,
            promised: &[StructuralPathQualification],
        ) -> Result<Vec<CapturedNormalQualification>, TerminalInterpretError> {
            let mut captured = Vec::new();
            for row in promised {
                let carrier = terminal_semantics::qualification_path_carrier(
                    self.structural_types().values(),
                    owner.structural_type,
                    &row.path,
                )
                .ok_or_else(malformed)?;
                let domain = self
                    .program
                    .catalogs
                    .structural_domains
                    .iter()
                    .find(|domain| domain.id == row.domain && domain.carrier == carrier)
                    .ok_or_else(malformed)?;
                if self.normal_qualification_is_intrinsic(owner, &row.path, row.domain) {
                    continue;
                }
                let subject = self.normal_qualification_subject(owner, &row.path)?;
                if let NormalQualificationSubject::Active(value) = &subject
                    && (value.structural_type != domain.carrier
                        || !value.qualifications.contains(&row.domain))
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "normal host preservation lacks the exact original input membership",
                    ));
                }
                captured.push(CapturedNormalQualification {
                    path: row.path.clone(),
                    subject: subject.captured(),
                    carrier,
                    domain: row.domain,
                });
            }
            Ok(captured)
        }

        pub(crate) fn normal_qualification_is_covered(
            &self,
            value: &TerminalStructuralValue,
            path: &[StructuralPathSegment],
            domain: StructuralDomainId,
            captured: &[CapturedNormalQualification],
        ) -> Result<bool, TerminalInterpretError> {
            if self.normal_qualification_is_intrinsic(value, path, domain) {
                return Ok(true);
            }
            let subject = self.normal_qualification_subject(value, path)?;
            let carrier = terminal_semantics::qualification_path_carrier(
                self.structural_types().values(),
                value.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            let subject = subject.captured();
            Ok(captured.iter().any(|row| {
                row.domain == domain && row.carrier == carrier && row.subject == subject
            }))
        }

        pub(crate) fn validate_normal_case_preservation(
            &self,
            owner: &TerminalStructuralValue,
            captured: &[CapturedNormalQualification],
            changed: &[(StructuralRuntimePlace, StructuralCaseId)],
        ) -> Result<(), TerminalInterpretError> {
            for row in captured
                .iter()
                .filter(|row| matches!(row.subject, CapturedQualificationSubject::Inactive { .. }))
            {
                for (position, segment) in row.path.iter().enumerate() {
                    let StructuralPathSegment::Case(expected) = segment else {
                        continue;
                    };
                    let carrier = self
                        .resolve_reference_storage_projection(
                            owner,
                            &row.path[..position],
                            StructuralAccess::SharedBorrow,
                        )?
                        .ok_or_else(malformed)?
                        .value;
                    let home = StructuralRuntimePlace::from(&carrier);
                    let actual = self.structural_cases.require(&home)?;
                    if actual.structural_type != carrier.structural_type {
                        return Err(malformed());
                    }
                    let returned = changed
                        .iter()
                        .find(|(place, _)| *place == home)
                        .map_or(actual.case, |(_, case)| *case);
                    if actual.case != *expected {
                        if returned == *expected {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "case replacement cannot establish an uncaptured normal membership",
                            ));
                        }
                        break;
                    }
                    if returned != *expected {
                        break;
                    }
                }
            }
            Ok(())
        }

        fn normal_qualification_is_intrinsic(
            &self,
            value: &TerminalStructuralValue,
            path: &[StructuralPathSegment],
            domain: StructuralDomainId,
        ) -> bool {
            terminal_semantics::scalar_projection_entails_membership(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                value.structural_type,
                path,
                domain,
            )
        }

        fn normal_qualification_subject(
            &self,
            value: &TerminalStructuralValue,
            path: &[StructuralPathSegment],
        ) -> Result<NormalQualificationSubject, TerminalInterpretError> {
            for (position, segment) in path.iter().enumerate() {
                match segment {
                    StructuralPathSegment::AllElements => {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "normal host membership preservation needs an exact captured element",
                        ));
                    }
                    StructuralPathSegment::Case(expected) => {
                        let carrier = self
                            .resolve_reference_storage_projection(
                                value,
                                &path[..position],
                                StructuralAccess::SharedBorrow,
                            )?
                            .ok_or_else(malformed)?
                            .value;
                        let actual = self
                            .structural_cases
                            .get(&StructuralRuntimePlace::from(&carrier))
                            .ok_or_else(malformed)?;
                        if actual.structural_type != carrier.structural_type {
                            return Err(malformed());
                        }
                        if actual.case != *expected {
                            return Ok(NormalQualificationSubject::Inactive(
                                CapturedQualificationSubject::Inactive {
                                    guard: StructuralRuntimePlace::from(&carrier),
                                    guard_carrier: carrier.structural_type,
                                    suffix: path[position..].to_vec(),
                                },
                            ));
                        }
                    }
                    _ => {}
                }
            }
            if path.is_empty() {
                return Ok(NormalQualificationSubject::Active(value.clone()));
            }
            self.resolve_reference_storage_projection(value, path, StructuralAccess::SharedBorrow)?
                .map(|projection| NormalQualificationSubject::Active(projection.value))
                .ok_or_else(malformed)
        }
    }
}
pub(crate) mod original_storage {
    use crate::{TerminalScalarBacking, TerminalScalarValue};
    use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
    use std::rc::Rc;
    use terminal_psi::{StructuralAccess, TerminalPsiIdentity};

    pub(crate) struct OriginalByteCell {
        pub(crate) owner: crate::storage::byte_sequences::ByteSequenceView,
        pub(crate) backing: Rc<dyn TerminalScalarBacking>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalOriginalStorageRoot {
        pub(crate) artifact: TerminalPsiIdentity,
        pub(crate) namespace: u64,
        pub(crate) allocation: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct OriginalStorageAllocation {
        pub(crate) root: TerminalOriginalStorageRoot,
        pub(crate) structural_type: StructuralTypeId,
    }
    impl TerminalOriginalStorageRoot {
        pub fn artifact(&self) -> TerminalPsiIdentity {
            self.artifact
        }
        pub fn namespace(&self) -> u64 {
            self.namespace
        }
        pub fn allocation(&self) -> u64 {
            self.allocation
        }
    }

    #[derive(Debug)]
    pub struct TerminalOriginalScalarValue {
        pub(crate) path: Vec<CanonicalStructuralPathSegment>,
        pub(crate) value: TerminalScalarValue,
        pub(crate) backing: Option<Rc<dyn TerminalScalarBacking>>,
    }
    impl TerminalOriginalScalarValue {
        pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.path
        }
        pub fn value(&self) -> TerminalScalarValue {
            self.value
        }
        pub fn backing(&self) -> Option<&Rc<dyn TerminalScalarBacking>> {
            self.backing.as_ref()
        }
    }

    #[derive(Debug)]
    pub struct TerminalOriginalStorageRequest {
        pub(crate) root: TerminalOriginalStorageRoot,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) argument_index: usize,
        pub(crate) referent: Vec<CanonicalStructuralPathSegment>,
        pub(crate) access: StructuralAccess,
        pub(crate) values: Vec<TerminalOriginalScalarValue>,
        pub(crate) allocation_contents: Vec<TerminalOriginalScalarValue>,
        pub(crate) argument_path: Vec<CanonicalStructuralPathSegment>,
        pub(crate) view_type: StructuralTypeId,
        pub(crate) references: Vec<TerminalOriginalReferenceValue>,
        pub(crate) scalar_projection: Option<
            Vec<(
                Vec<CanonicalStructuralPathSegment>,
                Vec<CanonicalStructuralPathSegment>,
            )>,
        >,
        pub(crate) byte_extent: Option<usize>,
        pub(crate) byte_window: Option<(usize, usize)>,
    }
    impl TerminalOriginalStorageRequest {
        pub fn byte_extent(&self) -> Option<usize> {
            self.byte_extent
        }
        pub fn root(&self) -> TerminalOriginalStorageRoot {
            self.root
        }
        pub fn structural_type(&self) -> StructuralTypeId {
            self.structural_type
        }
        pub fn argument_index(&self) -> usize {
            self.argument_index
        }
        pub fn referent(&self) -> &[CanonicalStructuralPathSegment] {
            &self.referent
        }
        pub fn access(&self) -> StructuralAccess {
            self.access
        }
        pub fn argument_path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.argument_path
        }
        pub fn view_type(&self) -> StructuralTypeId {
            self.view_type
        }
        pub fn references(&self) -> &[TerminalOriginalReferenceValue] {
            &self.references
        }
        pub fn loan(&self) -> TerminalOriginalStorageLoan {
            TerminalOriginalStorageLoan {
                root: self.root,
                referent: self.referent.clone(),
                scalar_projection: self.scalar_projection.clone(),
                byte_window: self.byte_window,
            }
        }
        pub fn values(&self) -> &[TerminalOriginalScalarValue] {
            &self.values
        }
        pub fn allocation_contents(&self) -> &[TerminalOriginalScalarValue] {
            &self.allocation_contents
        }
    }

    #[derive(Debug, Clone)]
    pub struct TerminalOriginalStorageLoan {
        pub(crate) root: TerminalOriginalStorageRoot,
        pub(crate) referent: Vec<CanonicalStructuralPathSegment>,
        pub(crate) scalar_projection: Option<
            Vec<(
                Vec<CanonicalStructuralPathSegment>,
                Vec<CanonicalStructuralPathSegment>,
            )>,
        >,
        pub(crate) byte_window: Option<(usize, usize)>,
    }
    impl TerminalOriginalStorageLoan {
        pub fn byte_window(&self) -> Option<(usize, usize)> {
            self.byte_window
        }
        pub fn root(&self) -> TerminalOriginalStorageRoot {
            self.root
        }
        pub fn referent(&self) -> &[CanonicalStructuralPathSegment] {
            &self.referent
        }

        pub fn scalar_path(
            &self,
            view_path: &[CanonicalStructuralPathSegment],
        ) -> Option<Vec<CanonicalStructuralPathSegment>> {
            if let Some(projection) = &self.scalar_projection {
                let position = projection
                    .binary_search_by(|(path, _)| path.as_slice().cmp(view_path))
                    .ok()?;
                Some(projection[position].1.clone())
            } else {
                let mut path = self.referent.clone();
                path.extend_from_slice(view_path);
                Some(path)
            }
        }
    }

    pub type TerminalOriginalScalarBackings = Vec<Vec<Rc<dyn TerminalScalarBacking>>>;

    #[derive(Debug)]
    pub struct TerminalOriginalReferenceValue {
        pub(crate) path: Vec<CanonicalStructuralPathSegment>,
        pub(crate) target: TerminalOriginalStorageLoan,
        pub(crate) referent_type: StructuralTypeId,
        pub(crate) access: StructuralAccess,
    }
    impl TerminalOriginalReferenceValue {
        pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.path
        }
        pub fn target(&self) -> &TerminalOriginalStorageLoan {
            &self.target
        }
        pub fn referent_type(&self) -> StructuralTypeId {
            self.referent_type
        }
        pub fn access(&self) -> StructuralAccess {
            self.access
        }
    }
}
pub(crate) mod results {
    use crate::boundary::effects::TerminalEffect;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::results::{TerminalCrash, TerminalCrashSite};
    use crate::boundary::values::{
        TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralValue,
    };
    use crate::engine::execution::TerminalExecution;
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;
    use terminal_psi::BoundaryMachineDeclaration;
    use terminal_psi::BoundaryMachineResult;
    use terminal_psi::CrashCause;
    use terminal_psi::OperationResult;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralTypeDeclaration;
    use terminal_psi::StructuralTypeShape;

    #[derive(Clone, PartialEq, Eq)]
    pub struct TerminalByteSequenceValue(crate::storage::byte_sequences::ByteSequenceView);

    impl std::fmt::Debug for TerminalByteSequenceValue {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("TerminalByteSequenceValue")
                .field("length", &self.0.bytes().len())
                .finish()
        }
    }

    impl TerminalByteSequenceValue {
        pub fn copy(
            bytes: &[u8],
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, TerminalInterpretError> {
            let storage =
                crate::storage::byte_sequences::storage::ByteSequenceStorage::copy(bytes, account)?;
            Ok(Self(crate::storage::byte_sequences::ByteSequenceView::new(
                storage,
            )))
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalEffectResult {
        Crash(CrashCause),
        Unit,
        Scalar(TerminalScalarValue),
        ScalarAggregate(TerminalScalarAggregateValue),
        Structural(TerminalStructuralValue),
        ByteSequence(TerminalByteSequenceValue),
        CapturedReference(super::storage_inputs::TerminalBoundaryReferenceSelection),
        Reference(TerminalReferenceResult),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalReferencePayload {
        ScalarAggregate(TerminalScalarAggregateValue),
        Zeroed,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalReferenceResult {
        pub referent: TerminalReferencePayload,
        pub references: Vec<(
            Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            TerminalReferenceResult,
        )>,
    }

    impl TerminalExecution<'_> {
        pub(crate) fn admit_boundary_crash(
            &self,
            declaration: &BoundaryMachineDeclaration,
            effect: &TerminalEffect,
            cause: CrashCause,
        ) -> Result<TerminalCrash, TerminalInterpretError> {
            let TerminalEffect::BoundaryCall {
                operation,
                boundary,
                arguments,
                ..
            } = effect
            else {
                return Err(malformed());
            };
            if *boundary != declaration.id {
                return Err(malformed());
            }
            let arguments = super::boundary_predicates::scalar_literals(arguments)?;
            super::crash_outcome::validate_boundary_crash_outcome(declaration, &arguments, cause)
                .map_err(|reason| TerminalInterpretError::BoundaryCrashNotPermitted {
                operation: *operation,
                boundary: *boundary,
                cause,
                reason,
            })?;
            Ok(TerminalCrash {
                site: TerminalCrashSite::BoundaryCall {
                    machine: self.current_machine,
                    block: self.current,
                    operation: *operation,
                    boundary: *boundary,
                },
                cause,
                site_guard: Vec::new(),
                frontier_lower_bound: self.live_claims.keys().copied().collect(),
            })
        }

        pub(crate) fn preflight_boundary_result(
            &self,
            boundary: &BoundaryMachineDeclaration,
            result: &OperationResult,
        ) -> Result<(), TerminalInterpretError> {
            if boundary.requires_checked_result(&self.program.catalogs.structural_domains) {
                return Err(TerminalInterpretError::RuntimeSupplySelectionRejected {
                    boundary: boundary.id,
                    reason: "host import cannot establish result provenance owned by another issuer",
                });
            }
            if let Some(result) = result.as_structural()
                && ((requires_materialized_host_custody(
                    &self.structural_types(),
                    result.structural_type,
                ) && !self.supports_scalar_aggregate_result(result)
                    && !self.supports_byte_sequence_result(result)
                    && !self.supports_captured_reference_result(boundary, result)
                    && !(self.supports_host_reference_result(result)
                        && boundary.result_reference_regions.iter().any(|region| {
                            region.path.is_empty()
                                && matches!(
                                    region.origin,
                                    terminal_psi::TerminalDynamicReferenceOrigin::Invocation { .. }
                                )
                        })))
                    || self.structural_values.contains_key(&result.place)
                    || self
                        .live_affine_frontier
                        .iter()
                        .any(|value| value.place == result.place)
                    || (!result.claims.is_empty()
                        && result.multiplicity != StructuralMultiplicity::Linear)
                    || !result.projected_qualifications.is_empty())
            {
                return Err(malformed());
            }
            Ok(())
        }

        pub(crate) fn supports_captured_reference_result(
            &self,
            boundary: &BoundaryMachineDeclaration,
            result: &terminal_psi::StructuralOperationResult,
        ) -> bool {
            result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && !boundary.result_reference_regions.is_empty()
                && boundary.result_reference_regions.iter().all(|region| {
                    region.path.is_empty()
                        && matches!(
                            region.origin,
                            terminal_psi::TerminalDynamicReferenceOrigin::Parameter { .. }
                        )
                })
                && matches!(
                    self.structural_types()
                        .get(&result.structural_type)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::Reference {
                        access: terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow,
                        ..
                    })
                )
        }

        pub(crate) fn commit_boundary_reference_result(
            &mut self,
            result: &terminal_psi::StructuralOperationResult,
            prepared: crate::storage::reference::PreparedReference,
        ) -> Result<(), TerminalInterpretError> {
            let mut carrier = TerminalStructuralValue::minted(
                self.local_structural_identities.allocate()?,
                result.structural_type,
                Vec::new(),
            );
            carrier.projected_qualifications = prepared.projected_qualifications;
            self.reference_referents.insert(
                crate::boundary::values::StructuralRuntimePlace::from(&carrier),
                prepared.referent,
            )?;
            self.structural_values.insert(result.place, carrier)?;
            self.track_affine_result(result);
            Ok(())
        }

        fn supports_byte_sequence_result(
            &self,
            result: &terminal_psi::StructuralOperationResult,
        ) -> bool {
            result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && matches!(
                    self.structural_types()
                        .get(&result.structural_type)
                        .map(|definition| &definition.shape),
                    Some(StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        }
                    ))
                )
        }

        fn supports_host_reference_result(
            &self,
            result: &terminal_psi::StructuralOperationResult,
        ) -> bool {
            result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && self
                    .reference_contract(result.structural_type)
                    .is_ok_and(|(referent, _)| {
                        self.structural_types()
                            .get(&referent)
                            .is_some_and(|declaration| {
                                crate::boundary::values::ScalarAggregatePlan::new(
                                    declaration,
                                    |identity| self.structural_types().get(&identity),
                                )
                                .is_ok()
                            })
                            || terminal_semantics::zeroed_structural_leaves_modulo_references(
                                self.structural_types().values(),
                                referent,
                            )
                            .is_some()
                    })
        }

        fn supports_scalar_aggregate_result(
            &self,
            result: &terminal_psi::StructuralOperationResult,
        ) -> bool {
            result.projected_qualifications.is_empty()
                && (result.claims.is_empty()
                    || result.multiplicity == StructuralMultiplicity::Linear)
                && self
                    .structural_types()
                    .get(&result.structural_type)
                    .is_some_and(|declaration| {
                        let scalar_fields =
                            |fields: &[terminal_psi::StructuralFieldDeclaration]| {
                                fields.iter().all(|field| {
                                    !field.relevance.is_erased()
                                        && field.field_type.scalar_type().is_some()
                                })
                            };
                        match &declaration.shape {
                            StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::FixedArray { .. }
                            | StructuralTypeShape::PrimitiveScalar(_) => {
                                crate::boundary::values::ScalarAggregatePlan::new(
                                    declaration,
                                    |identity| self.structural_types().get(&identity),
                                )
                                .is_ok()
                            }
                            StructuralTypeShape::Sum { cases } => {
                                cases.iter().all(|case| scalar_fields(&case.fields))
                            }
                            _ => false,
                        }
                    })
        }
    }

    pub(crate) fn requires_materialized_host_custody(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        root: StructuralTypeId,
    ) -> bool {
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            if terminal_semantics::scalar_array_leaf_shape(types.values(), current).is_some() {
                return true;
            }
            let Some(declaration) = types.get(&current) else {
                return true;
            };
            let mut fields = Vec::new();
            match &declaration.shape {
                StructuralTypeShape::Reference { .. } => {
                    return true;
                }
                StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::AtomicCell(_) => {
                    return true;
                }
                StructuralTypeShape::PrimitiveScalar(_) => {}
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => pending.push(*element),
                StructuralTypeShape::Record {
                    fields: members, ..
                } => fields.extend(members),
                StructuralTypeShape::Sum { .. } => return true,
                StructuralTypeShape::Mixed {
                    fields: members,
                    cases,
                } => {
                    fields.extend(members);
                    fields.extend(cases.iter().flat_map(|case| &case.fields));
                }
            }
            for field in fields {
                match field.field_type {
                    terminal_psi::StructuralFieldType::BoundedInteger(_)
                    | terminal_psi::StructuralFieldType::RefinedScalar(_) => return true,
                    terminal_psi::StructuralFieldType::Structural(child) => pending.push(child),
                    terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    | terminal_psi::StructuralFieldType::Erased { .. } => {}
                    terminal_psi::StructuralFieldType::ByteSequence(_)
                    | terminal_psi::StructuralFieldType::PredicateByteSequence { .. } => {
                        return true;
                    }
                }
            }
        }
        false
    }

    impl TerminalExecution<'_> {
        pub(crate) fn commit_boundary_result(
            &mut self,
            result: &OperationResult,
            expected: &BoundaryMachineResult,
            returned: TerminalEffectResult,
        ) -> Result<(), TerminalInterpretError> {
            match (result, expected, returned) {
                (
                    OperationResult::Structural(declaration),
                    BoundaryMachineResult::Structural(expected),
                    TerminalEffectResult::ByteSequence(bytes),
                ) if declaration.structural_type == expected.structural_type
                    && declaration.multiplicity == expected.multiplicity
                    && expected.qualifications.is_empty()
                    && expected.projected_qualifications.is_empty()
                    && self.supports_byte_sequence_result(declaration) =>
                {
                    if self.byte_sequence_values.contains_key(&declaration.place) {
                        return Err(malformed());
                    }
                    let bytes = bytes.0.into_account(self.account.as_ref())?;
                    let value = TerminalStructuralValue::minted(
                        self.local_structural_identities.allocate()?,
                        declaration.structural_type,
                        Vec::new(),
                    );
                    self.byte_sequence_values.insert(
                        declaration.place,
                        crate::storage::byte_sequences::binding::ByteSequenceBinding::Immutable(
                            bytes,
                        ),
                    )?;
                    self.structural_values.insert(declaration.place, value)?;
                    self.track_affine_result(declaration);
                }
                (
                    OperationResult::Unit,
                    BoundaryMachineResult::Unit,
                    TerminalEffectResult::Unit,
                ) => {}
                (
                    OperationResult::Scalar(declaration),
                    BoundaryMachineResult::Scalar(expected),
                    TerminalEffectResult::Scalar(value),
                ) if declaration.value_type() == *expected
                    && value.scalar_type() == expected.scalar_type =>
                {
                    self.values.insert(declaration.id, value)?;
                }
                (
                    OperationResult::Structural(declaration),
                    BoundaryMachineResult::Structural(expected),
                    TerminalEffectResult::ScalarAggregate(value),
                ) if declaration.structural_type == expected.structural_type
                    && declaration.multiplicity == expected.multiplicity
                    && declaration.qualifications == expected.qualifications
                    && declaration.projected_qualifications.is_empty()
                    && expected.projected_qualifications.is_empty()
                    && self.supports_scalar_aggregate_result(declaration) =>
                {
                    self.publish_scalar_aggregate(declaration, value)?;
                }
                (
                    OperationResult::Structural(declaration),
                    BoundaryMachineResult::Structural(expected),
                    TerminalEffectResult::Structural(value),
                ) if declaration.structural_type == expected.structural_type
                    && declaration.multiplicity == expected.multiplicity
                    && declaration.qualifications == expected.qualifications
                    && declaration.projected_qualifications.is_empty()
                    && expected.projected_qualifications.is_empty()
                    && value.structural_type == expected.structural_type
                    && value.qualifications == expected.qualifications
                    && value.path.is_empty()
                    && !requires_materialized_host_custody(
                        &self.structural_types(),
                        declaration.structural_type,
                    ) =>
                {
                    self.structural_host_atoms.insert(
                        crate::boundary::values::StructuralRuntimePlace::from(&value),
                        crate::storage::structural_storage::HostStructuralPayload::Opaque(
                            value.opaque_identity,
                        ),
                    )?;
                    self.structural_values.insert(declaration.place, value)?;
                    self.track_affine_result(declaration);
                }
                (
                    OperationResult::Structural(declaration),
                    BoundaryMachineResult::Structural(expected),
                    TerminalEffectResult::Reference(supply),
                ) if declaration.structural_type == expected.structural_type
                    && declaration.multiplicity == expected.multiplicity
                    && declaration.qualifications == expected.qualifications
                    && declaration.projected_qualifications.is_empty()
                    && expected.projected_qualifications.is_empty()
                    && declaration.claims.is_empty()
                    && self.supports_host_reference_result(declaration) =>
                {
                    self.publish_host_reference_result(declaration, supply)?;
                }
                _ => return Err(malformed()),
            }
            Ok(())
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::engine::calls::frames::tests::empty_program;
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
            TerminalStorageScheduleIdentity,
        };
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use std::sync::Arc;
        use terminal_psi::{
            BoundaryStructuralResultDeclaration, ByteSequenceCarrier, StructuralAccess,
        };

        #[test]
        fn immutable_host_bytes_retain_accounted_snapshot_until_last_copy_drops() {
            let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            };
            let sponsor = BuildEvaluationSponsor::new_with_measurement(
                BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 100, 4, 100, 100).unwrap(),
                measurement,
            )
            .unwrap();
            let account = sponsor.begin_invocation(measurement, 100).unwrap();
            let mut input = *b"path";
            let bytes = TerminalByteSequenceValue::copy(&input, Some(&account)).unwrap();
            input[0] = b'X';
            assert_eq!(&input, b"Xath");
            let retained = bytes.clone();
            drop(bytes);
            assert_eq!(retained.0.bytes(), b"path");
            assert_eq!(sponsor.live_payload_bytes(), 4);
            assert!(TerminalByteSequenceValue::copy(b"x", Some(&account)).is_err());
            assert_eq!(sponsor.live_payload_bytes(), 4);
            drop(retained);
            assert_eq!(sponsor.live_payload_bytes(), 0);
            let unaccounted = TerminalByteSequenceValue::copy(b"path", None).unwrap();
            let adopted = unaccounted.0.into_account(Some(&account)).unwrap();
            assert_eq!(adopted.bytes(), b"path");
            assert_eq!(sponsor.live_payload_bytes(), 4);
            assert!(
                TerminalByteSequenceValue::copy(b"x", None)
                    .unwrap()
                    .0
                    .into_account(Some(&account))
                    .is_err()
            );
            drop(adopted);
            assert_eq!(sponsor.live_payload_bytes(), 0);
        }

        #[test]
        fn host_byte_results_require_exact_shared_carrier_and_real_storage() {
            for access in [
                Some(StructuralAccess::SharedBorrow),
                Some(StructuralAccess::MutableBorrow),
                None,
            ] {
                for opaque in [false, true] {
                    let structural_type = StructuralTypeId::new(1).unwrap();
                    let mut program = empty_program();
                    Arc::get_mut(&mut program)
                        .unwrap()
                        .fixture_module_mut()
                        .structural_types
                        .push(StructuralTypeDeclaration {
                            requires: Vec::new(),
                            quotient_carrier: None,
                            id: structural_type,
                            identity: terminal_psi::TerminalIdentityIndex(0),
                            owns_linear_claim: false,
                            copyable: true,
                            no_code_affine_cleanup: true,
                            descriptor_cleanup: true,
                            shape: StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { access },
                            ),
                            scalar_restriction: None,
                        });
                    let mut execution = TerminalExecution::start_with_program(
                        program,
                        &[],
                        crate::TerminalStructuralInputs::default(),
                        None,
                        None,
                    )
                    .unwrap();
                    let declaration = terminal_psi::StructuralOperationResult {
                        place: PlaceId::new(1).unwrap(),
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    };
                    let expected =
                        BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                            structural_type,
                            multiplicity: declaration.multiplicity,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        });
                    let response = if opaque {
                        TerminalEffectResult::Structural(TerminalStructuralValue::minted(
                            7,
                            structural_type,
                            Vec::new(),
                        ))
                    } else {
                        TerminalEffectResult::ByteSequence(
                            TerminalByteSequenceValue::copy(b"path", None).unwrap(),
                        )
                    };
                    let accepted = execution.commit_boundary_result(
                        &OperationResult::Structural(declaration.clone()),
                        &expected,
                        response,
                    );
                    let valid = access == Some(StructuralAccess::SharedBorrow) && !opaque;
                    assert_eq!(
                        accepted.is_ok(),
                        valid,
                        "access={access:?}, opaque={opaque}, result={accepted:?}"
                    );
                    assert_eq!(
                        execution
                            .byte_sequence_values
                            .contains_key(&declaration.place),
                        valid
                    );
                    assert_eq!(
                        execution.structural_values.contains_key(&declaration.place),
                        valid
                    );
                    if valid {
                        let bytes = execution
                            .byte_sequence_values
                            .get(&declaration.place)
                            .unwrap()
                            .immutable()
                            .unwrap();
                        assert_eq!(bytes.bytes(), b"path");
                        let repeated = TerminalEffectResult::ByteSequence(
                            TerminalByteSequenceValue::copy(b"other", None).unwrap(),
                        );
                        assert!(
                            execution
                                .commit_boundary_result(
                                    &OperationResult::Structural(declaration.clone()),
                                    &expected,
                                    repeated
                                )
                                .is_err()
                        );
                        assert_eq!(
                            execution
                                .byte_sequence_values
                                .get(&declaration.place)
                                .unwrap()
                                .immutable()
                                .unwrap()
                                .bytes(),
                            b"path"
                        );
                    }
                }
            }
        }
    }
}
pub(crate) mod scalar_loans {
    use std::iter::once;
    use std::sync::Arc;

    use semantic_vocabulary::{CanonicalStructuralPathSegment, PlaceId, ScalarType};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeShape, reading_borrow_pair_pattern,
    };

    use super::original_storage::TerminalOriginalStorageLoan;
    use crate::boundary::effects::TerminalEffectRejection;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralFieldNamespace, StructuralRuntimePlace, TerminalScalarValue,
        TerminalStructuralValue,
    };
    use crate::engine::execution::TerminalExecution;
    use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
    use crate::storage::primitive_storage::{
        PrimitiveStorage, PrimitiveStorageEncoding, PrimitiveStorageEndpoint,
    };

    #[derive(Debug)]
    pub struct TerminalBoundaryScalarBuffer {
        pub(crate) original_storage: Option<TerminalOriginalStorageLoan>,
        identity: Arc<()>,
        argument_index: usize,
        field_path: Vec<CanonicalStructuralPathSegment>,
        access: StructuralAccess,
        value: TerminalScalarValue,
        restriction: Option<StructuralFieldType>,
        stored_scalar: Option<ScalarType>,
        encoding: Option<PrimitiveStorageEncoding>,
    }

    impl TerminalBoundaryScalarBuffer {
        pub(crate) fn capture_identity(&self) -> Arc<()> {
            Arc::clone(&self.identity)
        }

        pub fn original_storage(&self) -> Option<&TerminalOriginalStorageLoan> {
            self.original_storage.as_ref()
        }
        pub fn argument_index(&self) -> usize {
            self.argument_index
        }

        pub fn field_path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.field_path
        }

        pub fn access(&self) -> StructuralAccess {
            self.access
        }

        pub fn value(&self) -> TerminalScalarValue {
            self.value
        }

        pub fn storage_scalar_type(&self) -> ScalarType {
            self.stored_scalar.unwrap_or(self.value.scalar_type())
        }

        pub fn replace(
            &mut self,
            value: TerminalScalarValue,
        ) -> Result<(), TerminalEffectRejection> {
            if !matches!(self.access, StructuralAccess::MutableBorrow)
                || value.scalar_type() != self.value.scalar_type()
                || !terminal_scalar_belongs_to_type(value)
                || self.stored_scalar.is_some_and(|stored| {
                    !crate::storage::primitive_storage::scalar_fits_storage(stored, value)
                })
                || self
                    .restriction
                    .as_ref()
                    .is_some_and(|field| !satisfies_source_restriction(value, field, self.encoding))
            {
                return Err(TerminalEffectRejection::new(
                    "boundary scalar replacement violates access, carrier or original field restriction",
                ));
            }
            self.value = value;
            Ok(())
        }
    }

    pub(crate) struct ScalarLoanBinding {
        identity: Arc<()>,
        pub(crate) argument_index: usize,
        field_path: Vec<CanonicalStructuralPathSegment>,
        pub(crate) access: StructuralAccess,
        pub(crate) source: PlaceId,
        pub(crate) storage: PrimitiveStorage,
        pub(crate) resident_storage: Option<PrimitiveStorage>,
        pub(crate) direct_original: bool,
        scalar_type: ScalarType,
        shared_view: bool,
        original: TerminalScalarValue,
        restriction: Option<StructuralFieldType>,
    }

    pub(crate) fn satisfies_source_restriction(
        value: TerminalScalarValue,
        field: &StructuralFieldType,
        encoding: Option<PrimitiveStorageEncoding>,
    ) -> bool {
        let source = match encoding {
            Some(encoding) => field
                .scalar_type()
                .ok_or_else(malformed)
                .and_then(|scalar| encoding.source_value(value, scalar)),
            None => Ok(value),
        };
        source.is_ok_and(|source| source.satisfies_field(field))
    }

    #[derive(Default)]
    pub(crate) struct BoundaryScalarLoans {
        pub(crate) buffers: Vec<TerminalBoundaryScalarBuffer>,
        pub(crate) bindings: Vec<ScalarLoanBinding>,
    }

    impl BoundaryScalarLoans {
        pub(crate) fn is_empty(&self) -> bool {
            self.buffers.is_empty() && self.bindings.is_empty()
        }

        pub(crate) fn capture(
            &mut self,
            execution: &TerminalExecution,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            argument_index: usize,
            identity_cursor: &mut Option<u64>,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            let StructuralTypeShape::PrimitiveScalar(expected) = execution
                .structural_declaration(parameter.structural_type)?
                .shape
            else {
                return Err(malformed());
            };
            if parameter.access != argument.access
                || !matches!(parameter.access, reading_borrow_pair_pattern!())
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || (parameter.access == StructuralAccess::MutableBorrow
                    && (!parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()))
            {
                return Err(malformed());
            }
            let loan = execution.capture_primitive_loan(parameter, argument, identity_cursor)?;
            let value = loan.referent;
            let storage = loan.storage;
            let scalar_type = loan.scalar_type;
            let shared_view = loan.shared_view;
            if scalar_type != expected {
                return Err(malformed());
            }
            self.capture_storage(
                execution,
                argument.place,
                storage,
                scalar_type,
                shared_view,
                parameter.access,
                argument_index,
                Vec::new(),
            )?;
            Ok(value)
        }

        fn capture_storage(
            &mut self,
            execution: &TerminalExecution,
            source: PlaceId,
            storage: PrimitiveStorage,
            scalar_type: ScalarType,
            shared_view: bool,
            access: StructuralAccess,
            argument_index: usize,
            field_path: Vec<CanonicalStructuralPathSegment>,
        ) -> Result<(), TerminalInterpretError> {
            if access == StructuralAccess::MutableBorrow {
                execution.validate_scalar_loan_qualifications(&storage)?;
            }
            let original = execution.read_primitive_storage(
                source,
                storage.clone(),
                scalar_type,
                scalar_type,
                shared_view,
            )?;
            let restriction = execution.captured_scalar_restriction(&storage, access)?;
            if let (Some(field), Some(encoding)) = (&restriction, storage.encoding) {
                let source_type = field.scalar_type().ok_or_else(malformed)?;
                let source_value = encoding.source_value(original, source_type)?;
                if source_value != execution.endpoint_scalar(source, &storage.endpoint)? {
                    return Err(malformed());
                }
            }
            if restriction.as_ref().is_some_and(|field| {
                !satisfies_source_restriction(original, field, storage.encoding)
            }) {
                return Err(malformed());
            }
            let identity = Arc::new(());
            self.buffers.push(TerminalBoundaryScalarBuffer {
                original_storage: None,
                identity: Arc::clone(&identity),
                argument_index,
                field_path: field_path.clone(),
                access,
                value: original,
                restriction: restriction.clone(),
                stored_scalar: storage.storage_scalar(),
                encoding: storage.encoding,
            });
            self.bindings.push(ScalarLoanBinding {
                direct_original: false,
                resident_storage: None,
                identity,
                argument_index,
                field_path,
                access,
                source,
                storage,
                scalar_type,
                shared_view,
                original,
                restriction,
            });
            Ok(())
        }

        pub(crate) fn validate_writeback(
            &self,
            execution: &TerminalExecution,
            buffers: &[&TerminalBoundaryScalarBuffer],
        ) -> Result<(), TerminalInterpretError> {
            if buffers.len() != self.bindings.len() {
                return Err(malformed());
            }
            for (buffer, binding) in buffers.iter().zip(&self.bindings) {
                if !Arc::ptr_eq(&buffer.identity, &binding.identity)
                    || buffer.argument_index != binding.argument_index
                    || buffer.field_path != binding.field_path
                    || buffer.access != binding.access
                    || buffer.value.scalar_type() != binding.scalar_type
                    || !terminal_scalar_belongs_to_type(buffer.value)
                    || buffer.stored_scalar != binding.storage.storage_scalar()
                    || buffer.encoding != binding.storage.encoding
                    || binding.storage.storage_scalar().is_some_and(|stored| {
                        !crate::storage::primitive_storage::scalar_fits_storage(
                            stored,
                            buffer.value,
                        )
                    })
                    || (binding.access == StructuralAccess::SharedBorrow
                        && buffer.value != binding.original)
                    || binding.restriction.as_ref().is_some_and(|field| {
                        !satisfies_source_restriction(buffer.value, field, binding.storage.encoding)
                    })
                {
                    return Err(malformed());
                }
                let current = execution.read_primitive_storage(
                    binding.source,
                    binding
                        .resident_storage
                        .clone()
                        .unwrap_or_else(|| binding.storage.clone()),
                    binding.scalar_type,
                    binding.scalar_type,
                    binding.shared_view,
                )?;
                let expected = if binding.direct_original {
                    buffer.value
                } else {
                    binding.original
                };
                if (binding.direct_original && buffer.original_storage.is_none())
                    || current != expected
                {
                    return Err(malformed());
                }
            }
            Ok(())
        }

        pub(crate) fn commit(
            self,
            execution: &mut TerminalExecution,
            buffers: impl IntoIterator<Item = TerminalBoundaryScalarBuffer>,
        ) -> Result<(), TerminalInterpretError> {
            for (buffer, binding) in buffers.into_iter().zip(self.bindings) {
                if binding.access != StructuralAccess::SharedBorrow {
                    if binding.direct_original {
                        execution.record_original_scalar_write(
                            binding
                                .resident_storage
                                .as_ref()
                                .unwrap_or(&binding.storage),
                        )?;
                    } else {
                        execution.store_primitive_storage(
                            binding.source,
                            binding.storage,
                            binding.scalar_type,
                            buffer.value,
                        )?;
                    }
                }
            }
            Ok(())
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn validate_scalar_loan_qualifications(
            &self,
            storage: &PrimitiveStorage,
        ) -> Result<(), TerminalInterpretError> {
            let Some(target) = scalar_qualification_owner(storage) else {
                return Ok(());
            };
            self.validate_storage_loan_qualifications(target)
        }

        pub(crate) fn validate_scalar_loan_restoration(
            &self,
            storage: &PrimitiveStorage,
            owner: &StructuralRuntimePlace,
            normal: &[crate::boundary::effects::normal_qualifications::CapturedNormalQualification],
        ) -> Result<(), TerminalInterpretError> {
            let Some(target) = scalar_qualification_owner(storage) else {
                return Ok(());
            };
            self.validate_storage_loan_restoration_at(target, owner, normal)
        }

        pub(crate) fn validate_storage_loan_qualifications(
            &self,
            target: &StructuralRuntimePlace,
        ) -> Result<(), TerminalInterpretError> {
            self.validate_storage_loan_restoration(target, &[])
        }

        pub(crate) fn validate_storage_loan_restoration(
            &self,
            target: &StructuralRuntimePlace,
            normal: &[crate::boundary::effects::normal_qualifications::CapturedNormalQualification],
        ) -> Result<(), TerminalInterpretError> {
            self.validate_storage_loan_restoration_at(target, target, normal)
        }

        fn validate_storage_loan_restoration_at(
            &self,
            target: &StructuralRuntimePlace,
            owner: &StructuralRuntimePlace,
            normal: &[crate::boundary::effects::normal_qualifications::CapturedNormalQualification],
        ) -> Result<(), TerminalInterpretError> {
            if target.opaque_identity != owner.opaque_identity
                || !target.path.starts_with(&owner.path)
            {
                return Err(malformed());
            }
            let owns_target = |value: &TerminalStructuralValue| {
                value.opaque_identity == target.opaque_identity
                    && target.path.starts_with(&value.path)
            };
            let covered =
                |value: &TerminalStructuralValue,
                 qualifications: &[semantic_vocabulary::StructuralDomainId],
                 projected: &[terminal_psi::StructuralPathQualification]| {
                    self.normal_qualifications_cover(value, qualifications, projected, normal)
                };
            let unsupported = || {
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable host storage has an uncovered original qualification or owner",
                )
            };
            for (machine_id, values) in once((self.current_machine, &self.structural_values)).chain(
                self.call_stack
                    .iter()
                    .map(|frame| (frame.current_machine, &frame.structural_values)),
            ) {
                let machine = self.program.machine(machine_id)?;
                for (place, value) in values.iter().filter(|(_, value)| owns_target(value)) {
                    if !covered(
                        value,
                        &value.qualifications,
                        &value.projected_qualifications,
                    )? {
                        return Err(unsupported());
                    }
                    for parameter in machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .filter(|parameter| parameter.place == *place)
                    {
                        if !covered(
                            value,
                            &parameter.qualifications,
                            &parameter.projected_qualifications,
                        )? {
                            return Err(unsupported());
                        }
                    }
                    for result in machine
                        .operations()
                        .filter_map(|operation| operation.result.structural())
                        .filter(|result| result.place == *place)
                    {
                        if !covered(
                            value,
                            &result.qualifications,
                            &result.projected_qualifications,
                        )? {
                            return Err(unsupported());
                        }
                    }
                }
            }
            for reference in self
                .reference_referents
                .values()
                .filter(|reference| owns_target(&reference.view))
            {
                if !covered(
                    &reference.view,
                    &reference.view.qualifications,
                    &reference.view.projected_qualifications,
                )? {
                    return Err(unsupported());
                }
            }
            Ok(())
        }

        pub(crate) fn captured_scalar_restriction(
            &self,
            storage: &PrimitiveStorage,
            access: StructuralAccess,
        ) -> Result<Option<StructuralFieldType>, TerminalInterpretError> {
            if let PrimitiveStorageEndpoint::Projection { fragments, .. } = &storage.endpoint {
                for fragment in fragments {
                    if let Some(restriction) =
                        self.captured_scalar_restriction(&fragment.storage, access)?
                    {
                        let unrestricted = matches!(restriction,
                        StructuralFieldType::Scalar(scalar) if scalar != semantic_vocabulary::ScalarType::Boolean)
                            || matches!(restriction, StructuralFieldType::IeeeFloat(_));
                        if access != StructuralAccess::SharedBorrow && !unrestricted {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "projected host scalar replacement requires its original field restrictions",
                            ));
                        }
                    }
                }
                return Ok(None);
            }
            let PrimitiveStorageEndpoint::ScalarField(cell) = &storage.endpoint else {
                return Ok(None);
            };
            let parent_type = self.actual_storage_type_from_owner(&cell.parent)?;
            let declaration = self.structural_declaration(parent_type)?;
            let fields = if cell.namespace == StructuralFieldNamespace::CasePayload {
                let case = self.structural_cases.require(&cell.parent)?;
                if case.structural_type != declaration.id {
                    return Err(malformed());
                }
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return Err(malformed());
                };
                &terminal_psi::find_structural_case(cases, case.case)
                    .ok_or_else(malformed)?
                    .fields
            } else {
                match &declaration.shape {
                    terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                    _ => return Err(malformed()),
                }
            };
            let mut matching = fields.iter().filter(|field| field.id == cell.field);
            let (Some(field), None) = (matching.next(), matching.next()) else {
                return Err(malformed());
            };
            if field.relevance.is_erased()
                || field.field_type.scalar_type().is_none()
                || (matches!(
                    field.field_type,
                    StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
                ) && access != StructuralAccess::MutableBorrow)
            {
                return Err(malformed());
            }
            Ok(Some(field.field_type.clone()))
        }
    }

    pub(crate) fn captured_scalar_restoration_owner(
        storage: &PrimitiveStorage,
        presentation: StructuralRuntimePlace,
    ) -> Result<StructuralRuntimePlace, TerminalInterpretError> {
        let owner = scalar_qualification_owner(storage);
        if owner.is_none()
            && matches!(
                storage.endpoint,
                PrimitiveStorageEndpoint::Projection { .. }
            )
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "mutable host scalar projection has no single original restoration owner",
            ));
        }
        Ok(match owner {
            Some(owner)
                if owner.opaque_identity != presentation.opaque_identity
                    || !owner.path.starts_with(&presentation.path) =>
            {
                owner.clone()
            }
            _ => presentation,
        })
    }

    pub(crate) fn scalar_qualification_owner(
        storage: &PrimitiveStorage,
    ) -> Option<&StructuralRuntimePlace> {
        match &storage.endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => Some(place),
            PrimitiveStorageEndpoint::ScalarField(field) => Some(&field.parent),
            PrimitiveStorageEndpoint::ArrayElement { array, .. } => Some(&array.referent),
            PrimitiveStorageEndpoint::Projection { fragments, .. } => {
                let owner = scalar_qualification_owner(&fragments.first()?.storage)?;
                fragments
                    .iter()
                    .all(|fragment| scalar_qualification_owner(&fragment.storage) == Some(owner))
                    .then_some(owner)
            }
            PrimitiveStorageEndpoint::OwnedArrayElement { .. }
            | PrimitiveStorageEndpoint::RepresentationByte(_)
            | PrimitiveStorageEndpoint::ViewByte { .. } => None,
        }
    }
}
pub(crate) use crate::boundary_storage_inputs as storage_inputs;

use crate::boundary::effects::byte_buffers::TerminalBoundaryByteBuffer;
use crate::boundary::effects::results::TerminalEffectResult;
use crate::boundary::effects::storage_inputs::TerminalBoundaryStorageBuffer;
use crate::boundary::errors::TerminalInterpretError;
use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::BTreeMap;
use terminal_psi::{BoundaryMachineResult, CompletionReceipt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffect {
    MemoryFence {
        operation: OperationId,
        kind: terminal_psi::MemoryFenceKind,
    },
    BoundaryCall {
        operation: OperationId,
        boundary: BoundaryMachineId,
        arguments: Vec<TerminalScalarValue>,
        structural_arguments: Vec<TerminalStructuralValue>,
        byte_sequence_arguments: Vec<Option<Vec<u8>>>,
        completion_receipts: Vec<CompletionReceipt>,
        result: BoundaryMachineResult,
    },
    ComponentCall {
        operation: OperationId,
        boundary: BoundaryMachineId,
        provider: terminal_psi::TerminalPsiIdentity,
        candidate: MachineId,
        instance_namespace: u64,
        effects: Vec<TerminalEffect>,
    },
    PortWrite {
        operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalCallStorageRequirements<'handler> {
    pub arguments: &'handler [usize],
    pub original: bool,
}

pub trait TerminalEffectHandler {
    fn resolve_provider_storage(
        &mut self,
        _request: &crate::TerminalProviderStorageRequest,
    ) -> Result<Option<crate::TerminalProviderStorageSupply>, TerminalEffectRejection> {
        Ok(None)
    }
    fn poll_host_computation(
        &mut self,
        _effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(true)
    }

    fn poll_effect_readiness(
        &mut self,
        _effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(true)
    }

    fn accepts_reference_results(&self, _boundary: BoundaryMachineId) -> bool {
        false
    }

    fn storage_requirements(
        &self,
        _boundary: BoundaryMachineId,
        _scalar_arguments: &[TerminalScalarValue],
    ) -> TerminalCallStorageRequirements<'_> {
        TerminalCallStorageRequirements::default()
    }
    fn prepare_original_storage(
        &mut self,
        _boundary: BoundaryMachineId,
        _requests: &[original_storage::TerminalOriginalStorageRequest],
    ) -> Result<original_storage::TerminalOriginalScalarBackings, TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "handler does not supply original storage homes",
        ))
    }

    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection>;

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !storage_buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "handler does not support boundary storage loans",
            ));
        }
        self.handle_effect_with_byte_buffers(effect, byte_buffers)
    }

    fn handle_effect_with_byte_buffers(
        &mut self,
        effect: &TerminalEffect,
        buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "handler does not support mutable boundary byte buffers",
            ));
        }
        self.handle_effect_result(effect)
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(
            effect,
            TerminalEffect::BoundaryCall {
                result: BoundaryMachineResult::Structural(_),
                ..
            }
        ) {
            return Err(TerminalEffectRejection::new(
                "handler does not supply structural boundary results",
            ));
        }
        self.handle_effect(effect)?;
        Ok(TerminalEffectResult::Unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEffectRejection {
    pub reason: String,
}

pub use terminal_installation::{
    AdmittedAssumedBinding, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    AssumedBindingSelection, ProviderInstallationSelection, admit_target_layouts,
    admit_target_machine_selections,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeSupplies {
    pub(crate) native_code_layouts:
        Option<std::sync::Arc<crate::TerminalNativeCodeLayoutEnvironment>>,
    pub(crate) provider_selections: Option<terminal_installation::AdmittedProviderSelections>,
    pub(crate) x86_flags: Option<TerminalX86FlagsEnvironment>,
    pub(crate) memory_ordering: Option<crate::TerminalMemoryOrderingEnvironment>,
    pub(crate) entry: Option<terminal_psi::TargetMachineCallable>,
    pub(crate) code_identity: terminal_codec::TerminalExecutionIdentity,
    pub(crate) supplies: BTreeMap<BoundaryMachineId, AdmittedBoundarySupply>,
    pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
    pub(crate) folded_constant_target: Option<String>,
    pub(crate) target_machines: Option<AdmittedTargetMachineSelections>,
    pub(crate) target_layouts: Option<std::sync::Arc<AdmittedTargetLayouts>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmittedBoundarySupply {
    CheckedProvider(MachineId),
    ComponentProvider {
        provider: terminal_psi::TerminalPsiIdentity,
        provider_boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    DeferredComponent,
    HostImport,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalRuntimeSupplyRequest<'installation> {
    pub native_code_layouts: Option<&'installation crate::TerminalNativeCodeLayoutEnvironment>,
    pub x86_flags: Option<TerminalX86FlagsEnvironment>,
    pub memory_ordering: Option<crate::TerminalMemoryOrderingEnvironment>,
    pub entry: Option<terminal_psi::TargetMachineCallable>,
    pub installation: Option<&'installation terminal_installation::AdmittedProviderSelections>,
    pub host_imports: &'installation [BoundaryMachineId],
    pub target_machines: Option<&'installation AdmittedTargetMachineSelections>,
    pub target_layouts: Option<&'installation AdmittedTargetLayouts>,
}

impl From<&terminal_installation::AdmittedProviderSelections> for TerminalRuntimeSupplies {
    fn from(selected: &terminal_installation::AdmittedProviderSelections) -> Self {
        Self {
            provider_selections: Some(selected.clone()),
            x86_flags: None,
            memory_ordering: None,
            code_identity: selected.execution_identity().clone(),
            entry: None,
            supplies: selected
                .providers()
                .map(|(boundary, machine)| {
                    (boundary, AdmittedBoundarySupply::CheckedProvider(machine))
                })
                .chain(selected.assumed_bindings().iter().map(|relationship| {
                    (
                        relationship.required,
                        AdmittedBoundarySupply::CheckedProvider(relationship.implementation),
                    )
                }))
                .collect(),
            assumed_bindings: selected.assumed_bindings().to_vec(),
            folded_constant_target: selected.folded_constant_target().map(str::to_owned),
            target_machines: None,
            target_layouts: None,
            native_code_layouts: None,
        }
    }
}

impl TerminalRuntimeSupplies {
    pub fn with_component_provider(
        mut self,
        selected: &terminal_installation::AdmittedComponentProvider,
    ) -> Result<Self, TerminalInterpretError> {
        let boundary = selected.receiving_boundary();
        if selected.receiver() != &self.code_identity || self.supplies.contains_key(&boundary) {
            return Err(TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary,
                reason: "component selection names another receiver or an already selected slot",
            });
        }
        self.supplies.insert(
            boundary,
            AdmittedBoundarySupply::ComponentProvider {
                provider: selected.provider(),
                provider_boundary: selected.provider_boundary(),
                candidate: selected.candidate(),
            },
        );
        Ok(self)
    }

    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.code_identity.semantic()
    }

    pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
        &self.assumed_bindings
    }
}

impl TerminalEffectRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalX86FlagsEnvironment {
    rflags: u64,
}
impl TerminalX86FlagsEnvironment {
    pub fn new(rflags: u64) -> Result<Self, TerminalInterpretError> {
        if rflags & 2 == 0 || rflags & !0x3f7fd7 != 0 {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "invalid x86 flags environment: reserved RFLAGS bits",
            ));
        }
        Ok(Self { rflags })
    }
    pub(crate) fn snapshot(self) -> u64 {
        self.rflags & !((1 << 16) | (1 << 17))
    }

    pub(crate) fn restore_arithmetic_bits(&mut self, image: u64) {
        const ARITHMETIC: u64 = 0x8d5;
        self.rflags = (self.rflags & !ARITHMETIC) | (image & ARITHMETIC);
    }
}

/*
The operation family of lowering: expressions as scalar operations emitted in
evaluation order, and the `OperationSafety` obligation of each partial one.
A record-typed expression is storage and goes to `places`.
*/

use crate::body::Qualification;
use semantic_vocabulary::{ObligationId, OperationId, ScalarType, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, ProgramPoint};
use terminal_psi::{Operation, OperationKind, OperationResult};

use super::places::{Bound, Witness};
use super::{Evaluation, Lowering, Reached, Value, declaration};
use crate::body::{Expression, ExpressionKind, Policy, Strict, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Lowering<'_> {
    pub(super) fn operation_identity(&mut self, source: SourceOrigin) -> OperationId {
        let id = self.sources.operations.push(source.into());
        let location = self.operation_locations.push(None);
        debug_assert_eq!(id, location);
        id
    }

    /// The obligation guarding `operation`, created at the point before it.
    fn safety(&mut self, operation: OperationId, source: SourceOrigin) -> Checked<ObligationId> {
        let before = self.here()?;
        Ok(self.obligation(
            ObligationKind::OperationSafety {
                site: terminal_check_tables::obligations::ExecutionSite::Operation(operation),
            },
            before,
            source,
        ))
    }

    pub(super) fn emit(
        &mut self,
        operation: OperationId,
        result: Option<ScalarType>,
        kind: OperationKind,
    ) -> Checked<Value> {
        let (result, value) = match result {
            Some(scalar_type) => {
                let value = self.value(scalar_type, ValueOrigin::Operation(operation));
                (
                    OperationResult::Scalar(declaration(value, scalar_type)),
                    Value::Scalar(value),
                )
            }
            None => (OperationResult::Unit, Value::Unit),
        };
        self.emit_operation(operation, result, kind)?;
        Ok(value)
    }

    pub(super) fn emit_operation(
        &mut self,
        operation: OperationId,
        result: OperationResult,
        kind: OperationKind,
    ) -> Checked<()> {
        let block = self.current()?;
        let ordinal = self.draft()?.operations.len();
        let location = self.operation_locations.get_mut(operation).ok_or_else(|| {
            terminal_check_tables::Refusal {
                reason: terminal_check_tables::RefusalReason::ForeignIdentity,
                site: terminal_check_tables::RefusalSite::Machine(
                    self.id,
                    Construct::Operation(operation),
                ),
                source_span: None,
                detail: "an operation has no allocated source coordinate".to_owned(),
            }
        })?;
        if location.replace((block, ordinal)).is_some() {
            return Err(self.internal("an operation was emitted twice"));
        }
        self.draft()?
            .operations
            .push(Operation::unsuspended(operation, result, kind));
        self.point = Some(ProgramPoint::AfterOperation(operation));
        Ok(())
    }

    pub(super) fn scalar(&mut self, expression: &Expression) -> Evaluation<ValueId> {
        match continuing!(self.expression(expression)) {
            Value::Scalar(value) => Ok(Reached::Value(value)),
            Value::Record(_) => Err(self.internal("a record is used as a scalar value")),
            Value::Unit => Err(self.internal("a Unit call is used as a value")),
        }
    }

    /// The term an erased position names: a literal, a relevant scalar slot's
    /// current value, or the term another erased slot names. No operation is
    /// emitted; contracts.md, "Explicit erased bindings": the binding
    /// "contributes no runtime field, address, read, or cleanup."
    pub(super) fn witness(&mut self, expression: &Expression) -> Checked<Witness> {
        match (&expression.kind, expression.result) {
            (
                ExpressionKind::Integer(value),
                Some(ValueType::Scalar(ScalarType::Integer(t), Qualification::NONE)),
            ) => Ok(Witness::Integer(t, *value)),
            (ExpressionKind::Boolean(value), _) => Ok(Witness::Boolean(*value)),
            (ExpressionKind::ErasedApplication(term), _) => {
                use semantic_vocabulary::ScalarTerm;
                let mut term = term.clone();
                let mut pending = vec![&mut term];
                while let Some(current) = pending.pop() {
                    if let ScalarTerm::Value { id, scalar_type } = *current {
                        let slot = id.get() as usize - 1;
                        let bound = *self.slot_mut(slot)?;
                        *current = match bound {
                            Some(Bound::Scalar(id)) => ScalarTerm::Value { id, scalar_type },
                            Some(Bound::Erased(witness)) => witness.term(&self.erased_applications),
                            _ => {
                                return Err(refuse(
                                    self.id,
                                    self.current.map_or(Construct::Machine, Construct::Block),
                                    Unserved::Expression("erased term that reads storage"),
                                ));
                            }
                        };
                    } else {
                        pending.extend(current.operands_mut());
                    }
                }
                let index = self.erased_applications.len();
                self.erased_applications.push(term);
                Ok(Witness::Application(index))
            }
            (ExpressionKind::Slot(slot), Some(ValueType::Scalar(scalar_type, _))) => {
                match *self.slot_mut(*slot)? {
                    Some(Bound::Scalar(value)) => Ok(Witness::Value(value, scalar_type)),
                    Some(Bound::Erased(witness)) => Ok(witness),
                    Some(Bound::Home(_) | Bound::Record(_) | Bound::Observed(_)) | None => {
                        Err(refuse(
                            self.id,
                            self.current.map_or(Construct::Machine, Construct::Block),
                            Unserved::Expression("erased term that reads storage"),
                        ))
                    }
                }
            }
            _ => Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("erased term other than a literal or a binding"),
            )),
        }
    }

    /// A scalar field of a record formal, read as its current contents
    /// where the application's edge leaves. No operation is emitted.
    fn field_subject(
        &mut self,
        argument: &Expression,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        formal: ScalarType,
    ) -> Checked<semantic_vocabulary::ScalarTerm> {
        use semantic_vocabulary::{ContentPlaceVersion, ScalarTerm};
        let ExpressionKind::Slot(slot) = argument.kind else {
            return Err(self.internal("logical data subject is not a named record formal"));
        };
        let (Some(Bound::Record(root)) | Some(Bound::Home(root))) = *self.slot_mut(slot)? else {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("logical data subject that is not a record place"),
            ));
        };
        let path = path.to_vec();
        let version = ContentPlaceVersion::Current;
        Ok(match formal {
            ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            },
            ScalarType::Boolean => ScalarTerm::BooleanField {
                version,
                root,
                path,
            },
            ScalarType::IeeeFloat(format) => ScalarTerm::ieee_float_field_path(root, path, format),
            ScalarType::Callable(_) => {
                return Err(self.internal("logical data subject is a callable field"));
            }
        })
    }

    fn theorem_application(
        &mut self,
        theorem: terminal_psi::TheoremSource,
        arguments: &[Expression],
        fields: &[Vec<semantic_vocabulary::CanonicalStructuralPathSegment>],
        instantiation: &[terminal_psi::TheoremSource],
        source: SourceOrigin,
    ) -> Checked<()> {
        let (requires, arguments) = match theorem {
            terminal_psi::TheoremSource::Formal(proof_position) => {
                let parameter = self
                    .signatures
                    .theorems
                    .iter()
                    .find(|parameter| {
                        parameter.machine == self.id && parameter.proof_position == proof_position
                    })
                    .ok_or_else(|| {
                        self.internal("logical application has no declared proof formal")
                    })?;
                if arguments.len() != parameter.contract.parameters.len() {
                    return Err(self.internal("logical application has another subject telescope"));
                }
                let requires = parameter.contract.requires.len();
                let formal_types: Vec<_> = parameter
                    .contract
                    .parameters
                    .iter()
                    .map(|formal| formal.scalar_type)
                    .collect();
                if fields.len() != arguments.len() {
                    return Err(self.internal("logical application lost its subject paths"));
                }
                let arguments = arguments
                    .iter()
                    .zip(formal_types)
                    .zip(fields)
                    .map(|((argument, formal), path)| {
                        let subject = if path.is_empty() {
                            self.witness(argument)?.term(&self.erased_applications)
                        } else {
                            self.field_subject(argument, path, formal)?
                        };
                        if subject.scalar_type() != formal {
                            return Err(
                                self.internal("logical application has another subject carrier")
                            );
                        }
                        Ok(subject)
                    })
                    .collect::<Checked<Vec<_>>>()?;
                (requires, arguments)
            }
            // A named machine's subjects are its scalar formals and then its
            // erased ones, as Terminal Psi numbers them
            // (`terminal_semantics::theorems::named_contract`).
            terminal_psi::TheoremSource::Named(callee) => {
                let signature = terminal_check_tables::row(
                    self.signatures.machines,
                    callee,
                    self.id,
                    Construct::Machine,
                )?;
                if signature.result.is_some()
                    || !signature.borrowed.is_empty()
                    || arguments.len() != signature.parameters.len()
                {
                    return Err(self.internal("named theorem application has another telescope"));
                }
                let mut scalars = Vec::with_capacity(arguments.len());
                let mut erased = Vec::new();
                for (position, (argument, formal)) in
                    arguments.iter().zip(&signature.parameters).enumerate()
                {
                    let ValueType::Scalar(formal, _) = formal else {
                        return Err(self.internal("named theorem formal that is no scalar"));
                    };
                    let subject = self.witness(argument)?.term(&self.erased_applications);
                    if subject.scalar_type() != *formal {
                        return Err(
                            self.internal("logical application has another subject carrier")
                        );
                    }
                    if signature.is_erased(position) {
                        erased.push(subject);
                    } else {
                        scalars.push(subject);
                    }
                }
                scalars.extend(erased);
                (signature.requires.len(), scalars)
            }
        };
        let successor = self.block(source);
        let edge = self.edge(
            terminal_check_tables::edges::Destination::Block(successor),
            source,
        )?;
        let mut requirements = Vec::with_capacity(requires);
        for clause in 0..requires {
            let clause = self.position(clause)?;
            requirements.push(self.obligation(
                ObligationKind::TheoremRequirement { edge, clause },
                ProgramPoint::Edge(edge),
                source,
            ));
        }
        self.terminate(terminal_psi::Terminator::Jump {
            edge,
            target: successor,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        })?;
        self.theorem_applications
            .push(terminal_psi::TheoremApplication {
                machine: self.id,
                edge,
                successor,
                source: theorem,
                arguments,
                requirements,
                instantiation: instantiation
                    .iter()
                    .map(|source| terminal_psi::TheoremInstantiation {
                        source: *source,
                        derivations: Vec::new(),
                    })
                    .collect(),
            });
        self.enter(successor);
        Ok(())
    }

    fn result_type(&self, expression: &Expression) -> Checked<ScalarType> {
        expression
            .result
            .and_then(ValueType::scalar)
            .ok_or_else(|| self.internal("an operand has no scalar type"))
    }

    pub(super) fn expression(&mut self, expression: &Expression) -> Evaluation<Value> {
        let mut evaluated = self.expression_inner(expression)?;
        if let Reached::Value(Value::Scalar(value)) = evaluated
            && let Some(ValueType::Scalar(carrier, qualification)) = expression.result
        {
            // Core operator implementations compute on stored integer values
            // and establish their declared result policy through the ordinary
            // cast mechanism. A primitive operation cannot mint a domain,
            // whether it is one instruction or a composed saturating shift.
            let value = if !qualification.is_none()
                && matches!(
                    expression.kind,
                    ExpressionKind::Strict { .. } | ExpressionKind::BitwiseNot(_)
                ) {
                self.qualify(value, Qualification::NONE)?;
                self.establish_predicates(
                    value,
                    carrier,
                    qualification,
                    &[],
                    None,
                    expression.source,
                )?
            } else {
                value
            };
            self.qualify(value, qualification)?;
            evaluated = Reached::Value(Value::Scalar(value));
        }
        Ok(evaluated)
    }

    fn expression_inner(&mut self, expression: &Expression) -> Evaluation<Value> {
        let source = expression.source;
        // A record-typed expression names storage; its value is a root that
        // holds it. Everything below is scalar or Unit.
        let result = match expression.result {
            Some(ValueType::Record(_, _, _)) => {
                return self
                    .record(expression)
                    .map(|reached| reached.map(Value::Record));
            }
            Some(ValueType::Scalar(scalar_type, _)) => Some(scalar_type),
            // A declared call may transport an opaque result, as a slot
            // transports its formal. Neither grants operations of its stand-in.
            Some(ValueType::Rigid(_)) => {
                if !matches!(
                    expression.kind,
                    ExpressionKind::Slot(_) | ExpressionKind::Call { .. }
                ) {
                    return Err(self.internal("an operation over a value of a rigid type"));
                }
                None
            }
            None => None,
        };
        match &expression.kind {
            ExpressionKind::TestMembership {
                subject,
                domain,
                predicate,
            } => {
                continuing!(self.observe_membership(subject, *domain, source));
                self.expression(predicate)
            }
            // Only the mathematical signature reads such a body.
            ExpressionKind::Applied { .. } => {
                Err(self.internal("a mathematical application has no operation"))
            }
            ExpressionKind::TheoremApplication {
                source: theorem,
                arguments,
                fields,
                instantiation,
            } => {
                self.theorem_application(*theorem, arguments, fields, instantiation, source)?;
                Ok(Reached::Value(Value::Unit))
            }
            ExpressionKind::Atomic(operation) => match operation {
                crate::body::AtomicOperation::Access {
                    place,
                    event,
                    operands,
                } => self.atomic_access(place, *event, operands, result, source),
                crate::body::AtomicOperation::Fence(ordering) => {
                    let operation = self.operation_identity(source);
                    self.emit(
                        operation,
                        None,
                        OperationKind::AtomicFence {
                            ordering: *ordering,
                        },
                    )
                    .map(Reached::Value)
                }
            },
            ExpressionKind::Zero => {
                let scalar = self.result_type(expression)?;
                self.zero(scalar, source)
                    .map(|value| Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::Integer(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Float(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IeeeFloatConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Boolean(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::BooleanConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::MachineValue {
                contract,
                descriptor,
            } => {
                if result != Some(ScalarType::Callable(*contract))
                    || self
                        .callables
                        .descriptors
                        .get(*descriptor)
                        .is_none_or(|selection| selection.contract != *contract)
                {
                    return Err(self.internal("machine construction changes its retained contract"));
                }
                let operation = self.operation_identity(source);
                let value = self.emit(
                    operation,
                    result,
                    OperationKind::SelectMachine {
                        descriptor: *descriptor,
                    },
                )?;
                if let Some((owner, crate::source_state::SourceNode::Expression(node))) =
                    source.node()
                {
                    self.source_constructions
                        .push((self.id, operation, owner, node));
                }
                Ok(Reached::Value(value))
            }
            ExpressionKind::Slot(slot) => {
                let bound = *self.slot_mut(*slot)?;
                match bound {
                    Some(Bound::Scalar(value)) => Ok(Reached::Value(Value::Scalar(value))),
                    Some(Bound::Home(home)) => self.read_home(home, expression).map(Reached::Value),
                    Some(Bound::Record(_) | Bound::Observed(_)) => {
                        Err(self.internal("a record slot has a scalar type"))
                    }
                    // The reader refuses a runtime read of an erased
                    // binding, so reaching one here is a broken body.
                    Some(Bound::Erased(_)) => {
                        Err(self.internal("an erased binding is read as a runtime value"))
                    }
                    None => Err(self.internal("a slot is read before it is bound")),
                }
            }
            ExpressionKind::Index { base, index } => self.read_index(base, index, expression),
            ExpressionKind::Referent(_) => self.read_referent(expression),
            ExpressionKind::Field { base, field } => self.read_field(base, *field, expression),
            ExpressionKind::StorageLocation {
                query,
                place,
                first_element,
                ..
            } => self.storage_location(
                *query,
                place,
                *first_element,
                self.result_type(expression)?,
                source,
            ),
            ExpressionKind::CollectionMeasure { base, measure } => {
                self.measure_collection(base, *measure, source)
            }
            ExpressionKind::ByteSequencePredicate { base, predicate } => {
                self.byte_sequence_predicate(base, *predicate, source)
            }
            ExpressionKind::ErasedApplication(_) => {
                Err(self.internal("an erased application is read as a runtime value"))
            }
            ExpressionKind::ScalarApplication { application } => {
                let scalar_type = self.result_type(expression)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    Some(scalar_type),
                    OperationKind::ReadScalarApplication {
                        application: *application,
                    },
                )
                .map(Reached::Value)
            }
            ExpressionKind::ByteElement { base, index, .. } => {
                self.read_byte_element(base, index, source)
            }
            ExpressionKind::CollectionSubslice { .. } | ExpressionKind::CollectionView { .. } => {
                Err(self.internal("a byte descriptor is used as a scalar"))
            }
            ExpressionKind::Payload { base, case, field } => {
                self.read_payload(base, *case, *field, expression)
            }
            ExpressionKind::IsCase { subject, case } => {
                self.case_membership(subject, *case, source)
            }
            ExpressionKind::Match {
                capture,
                subject,
                arms,
                default,
            } => {
                // A Unit match has no result parameter. Each returning arm
                // continues at the same block.
                let result_type = match expression.result {
                    None => None,
                    Some(_) => Some(ValueType::plain(self.result_type(expression)?)),
                };
                self.match_value(
                    subject,
                    arms,
                    default.as_deref(),
                    result_type,
                    source,
                    *capture,
                )
            }
            // A block is always an arm's value. The arm's dispatch restores
            // the slots its statements bind, and the join's ownership plan
            // ends the storage they hold where the arm leaves, so a block
            // local ends with its block.
            ExpressionKind::Block { statements, value } => {
                for statement in statements {
                    continuing!(self.statement(statement));
                }
                match value {
                    Some(value) => self.expression(value),
                    None => Ok(Reached::Value(Value::Unit)),
                }
            }
            ExpressionKind::SavedSubject(capture) => {
                let value = self
                    .scalar_subjects
                    .iter()
                    .rev()
                    .find(|(owner, _)| owner == capture)
                    .map(|(_, value)| *value)
                    .ok_or_else(|| {
                        self.internal("a scalar saved subject is outside its match capture")
                    })?;
                Ok(Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::ByteLiteral(_)
            | ExpressionKind::Array { .. }
            | ExpressionKind::PackedInteger { .. }
            | ExpressionKind::Record { .. }
            | ExpressionKind::Case { .. } => Err(self.internal("a data literal has a scalar type")),
            ExpressionKind::Borrow { .. } | ExpressionKind::OwnedTemporary { .. } => {
                Err(self.internal("a loan outside a call's arguments"))
            }
            ExpressionKind::Not(operand) => {
                let operand = continuing!(self.scalar(operand));
                let operation = self.operation_identity(source);
                self.emit(operation, result, OperationKind::BooleanNot { operand })
                    .map(Reached::Value)
            }
            ExpressionKind::BitwiseNot(operand) => {
                let operand = continuing!(self.scalar(operand));
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerBitwiseNot { operand },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Cast(operand_expression) => {
                let operand = continuing!(self.scalar(operand_expression));
                let (ScalarType::Integer(from), Some(ScalarType::Integer(to))) =
                    (self.result_type(operand_expression)?, result)
                else {
                    return Err(self.internal("a cast between carriers that are not integers"));
                };
                let operation = self.operation_identity(source);
                // Widening is total. Any other conversion is exact only
                // where the value is representable, which is its obligation.
                let kind = if from.can_widen_to(to) {
                    OperationKind::IntegerWiden { operand }
                } else if from != to && from.can_exact_cast_to(to) {
                    OperationKind::IntegerExactCast {
                        operand,
                        obligation: self.safety(operation, source)?,
                    }
                } else {
                    return Err(self.internal("a cast with no exact conversion"));
                };
                self.emit(operation, result, kind).map(Reached::Value)
            }
            // The operand's value under another policy: nothing is computed.
            ExpressionKind::Requalify(operand) => {
                let scalar_type = self.result_type(operand)?;
                let operand = continuing!(self.scalar(operand));
                self.establish_predicates(
                    operand,
                    scalar_type,
                    expression
                        .result
                        .ok_or_else(|| self.internal("a cast has no result type"))?
                        .qualification(),
                    &[],
                    None,
                    source,
                )
                .map(|value| Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::Establish {
                operand,
                clauses,
                stated,
            } => {
                let scalar_type = self.result_type(operand)?;
                let operand = continuing!(self.scalar(operand));
                self.establish_predicates(
                    operand,
                    scalar_type,
                    expression
                        .result
                        .ok_or_else(|| self.internal("a cast has no result type"))?
                        .qualification(),
                    clauses,
                    Some(stated),
                    source,
                )
                .map(|value| Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::Strict {
                left: left_expression,
                right,
                ..
            } => {
                // The reader admits intrinsic storage equality for byte
                // views and payload-free sums after ordinary access checks.
                if matches!(left_expression.result, Some(ValueType::Record(..))) {
                    return self.structural_equal(left_expression, right, source);
                }
                // A `true ->` arm of a transition on a Boolean reaches here as
                // `subject == true`, which is the subject itself. Branching on
                // the subject lets native selection fuse its comparison into
                // the branch instead of materializing and re-testing it.
                if let ExpressionKind::Strict {
                    operation: Strict::Equal,
                    policy: None,
                    ..
                } = &expression.kind
                    && matches!(right.kind, ExpressionKind::Boolean(true))
                    && matches!(
                        left_expression.result,
                        Some(ValueType::Scalar(ScalarType::Boolean, _))
                    )
                    && matches!(
                        expression.result,
                        Some(ValueType::Scalar(ScalarType::Boolean, _))
                    )
                {
                    let subject = continuing!(self.scalar(left_expression));
                    return Ok(Reached::Value(Value::Scalar(subject)));
                }
                let left = continuing!(self.scalar(left_expression));
                let right = continuing!(self.scalar(right));
                self.strict_expression(expression, left, right)
                    .map(Reached::Value)
            }
            ExpressionKind::ShortCircuit {
                conjunction,
                left,
                right,
            } => self.short_circuit(*conjunction, left, right),
            ExpressionKind::Call {
                callee,
                required_contract,
                static_reach_binding,
                static_reach_arguments,
                proof_arguments,
                arguments,
                designated,
            } => self.call(
                callee,
                *required_contract,
                arguments,
                *designated,
                expression.result,
                (
                    source,
                    *static_reach_binding,
                    static_reach_arguments,
                    proof_arguments,
                ),
                expression.call_context.unwrap_or_default(),
            ),
        }
    }

    /// One arithmetic or shift operation under its selected policy.
    ///
    /// numeric_values.md, "Overflow policies": "Wrapping reduces
    /// representable-range overflow modulo the selected carrier; Saturating
    /// clamps it to carrier bounds", and "An overflow policy does not invent
    /// a result for undefined operands such as an integer zero divisor". So
    /// a Wrapping or Saturating add, subtract and multiply is total and owes
    /// nothing, a divide and a remainder still owe their divisor, and a
    /// Wrapping shift reduces its count and is total. What each one owes is
    /// Terminal semantics' to say (`integer_policy_bridge`); this names the
    /// operation and mints the obligation the operation carries. A
    /// Saturating shift keeps the Exact count proof while selecting its
    /// clamped value through the shared primitive computation.
    fn policy_operation(
        &mut self,
        strict: Strict,
        policy: Policy,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // Wrapping reduces modulo `2^N` and saturating clamps to the
        // carrier's range (`numeric_values.md`, "Width-indexed integers");
        // at a parameter width no term states either yet.
        if result
            .and_then(|scalar| scalar.as_integer())
            .is_some_and(|integer| integer.bits().is_none())
        {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("policy arithmetic at a parameter width"),
            ));
        }
        if policy == Policy::Saturating && crate::policy::shifts(strict) {
            let carrier = result
                .and_then(|scalar| scalar.as_integer())
                .ok_or_else(|| self.internal("a saturating shift has no integer result"))?;
            if strict == Strict::ShiftRight {
                return self.strict(
                    strict,
                    ScalarType::Integer(carrier),
                    left,
                    right,
                    result,
                    source,
                );
            }
            let closed = carrier
                .try_into()
                .map_err(|_| self.internal("a saturating shift has no closed carrier"))?;
            let mut computation = IntegerOperations {
                lowering: self,
                source,
            };
            return crate::policy::saturating_shift_left(&mut computation, closed, left, right)
                .map(Value::Scalar);
        }
        let operation = self.operation_identity(source);
        // The total operations are the ones a clause's term names too.
        if let Some(kind) = crate::policy::total(strict, policy, left, right) {
            return self.emit(operation, result, kind);
        }
        if policy == Policy::Trapping {
            return self.trapping(operation, strict, left, right, result, source);
        }
        let kind = match (strict, policy) {
            (Strict::Divide, Policy::Wrapping) => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Divide, Policy::Saturating) => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Remainder, Policy::Wrapping) => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Remainder, Policy::Saturating) => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            // The reader gives no other operation a policy-qualified result,
            // `policy::total` named every total one above, and a Trapping
            // operation is `trapping`'s.
            (_, Policy::Trapping)
            | (
                Strict::Add
                | Strict::Subtract
                | Strict::Multiply
                | Strict::ShiftLeft
                | Strict::ShiftRight
                | Strict::BitwiseAnd
                | Strict::BitwiseOr
                | Strict::BitwiseXor
                | Strict::Equal
                | Strict::NotEqual
                | Strict::Less
                | Strict::LessOrEqual
                | Strict::Greater
                | Strict::GreaterOrEqual,
                _,
            ) => return Err(self.internal("a policy on an operation that has none")),
        };
        self.emit(operation, result, kind)
    }

    /// One Trapping operation: its own `Trap` site.
    ///
    /// numeric_values.md, "Overflow policies": "Terminal carries it as one
    /// `TrappingInteger` operation per primitive ... that is its own `Trap`
    /// site", whose predicate is the exact primitive's ("result outside the
    /// carrier, a zero divisor or signed `MIN / -1` ..., and a shift count
    /// outside `0..width`"). The site owes coverage by the machine's
    /// published `Trap` route like a `crash Trap;` (effects.md, "Guarded
    /// crashes"); path facts that disprove the predicate discharge it with
    /// no route. What the predicate is and what the result equals on normal
    /// return are Terminal semantics' to say.
    fn trapping(
        &mut self,
        operation: OperationId,
        strict: Strict,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        use terminal_psi::TrappingIntegerOperation as Trapping;
        let primitive = match strict {
            Strict::Add => Trapping::Add { left, right },
            Strict::Subtract => Trapping::Subtract { left, right },
            Strict::Multiply => Trapping::Multiply { left, right },
            Strict::Divide => Trapping::Divide { left, right },
            Strict::Remainder => Trapping::Remainder { left, right },
            Strict::ShiftLeft => Trapping::ShiftLeft {
                value: left,
                count: right,
            },
            Strict::ShiftRight => Trapping::ShiftRight {
                value: left,
                count: right,
            },
            Strict::BitwiseAnd
            | Strict::BitwiseOr
            | Strict::BitwiseXor
            | Strict::Equal
            | Strict::NotEqual
            | Strict::Less
            | Strict::LessOrEqual
            | Strict::Greater
            | Strict::GreaterOrEqual => {
                return Err(self.internal("a Trapping policy on an operation that has none"));
            }
        };
        let before = self.here()?;
        self.obligation(
            ObligationKind::CrashCoverage {
                site: terminal_check_tables::obligations::CrashSite::Operation(operation),
                cause: semantic_vocabulary::CrashCause::Trap,
            },
            before,
            source,
        );
        self.emit(
            operation,
            result,
            OperationKind::TrappingInteger {
                operation: primitive,
            },
        )
    }

    /// Body expressions and ranking observations use the same selected
    /// arithmetic policy. Reconstructing an observation must not turn a
    /// wrapping or saturating operation into exact arithmetic.
    pub(super) fn strict_expression(
        &mut self,
        expression: &Expression,
        left: ValueId,
        right: ValueId,
    ) -> Checked<Value> {
        let ExpressionKind::Strict {
            operation,
            policy,
            left: operand,
            ..
        } = &expression.kind
        else {
            return Err(self.internal("a strict operation has no expression selection"));
        };
        let result = match expression.result {
            Some(ValueType::Scalar(scalar_type, _)) => Some(scalar_type),
            None => None,
            _ => return Err(self.internal("a strict operation has a non-scalar result")),
        };
        if let Some(policy) = policy {
            self.policy_operation(*operation, *policy, left, right, result, expression.source)
        } else {
            self.strict(
                *operation,
                self.result_type(operand)?,
                left,
                right,
                result,
                expression.source,
            )
        }
    }

    pub(super) fn strict(
        &mut self,
        strict: Strict,
        operand_type: ScalarType,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let operation = self.operation_identity(source);
        let kind = match strict {
            Strict::Add => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Subtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Multiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Divide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Remainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Strict::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Strict::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Strict::Equal | Strict::NotEqual => match operand_type {
                ScalarType::Boolean => OperationKind::BooleanEqual { left, right },
                ScalarType::Integer(_) => OperationKind::IntegerEqual { left, right },
                ScalarType::IeeeFloat(_) => {
                    return Err(self.internal("a float equality reached lowering"));
                }
                ScalarType::Callable(_) => {
                    return Err(refuse(
                        self.id,
                        Construct::Operation(operation),
                        Unserved::Expression("callable equality"),
                    ));
                }
            },
            Strict::Less => OperationKind::IntegerLessThan { left, right },
            Strict::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
            // `a > b` is `b < a`: both operands are already evaluated in
            // authored order, so swapping the operand identities changes no
            // evaluation.
            Strict::Greater => OperationKind::IntegerLessThan {
                left: right,
                right: left,
            },
            Strict::GreaterOrEqual => OperationKind::IntegerLessOrEqual {
                left: right,
                right: left,
            },
        };
        if strict != Strict::NotEqual {
            return self.emit(operation, result, kind);
        }
        // `!=` is the negation of the equality: Terminal Psi has no
        // inequality operation.
        let Value::Scalar(equal) = self.emit(operation, Some(ScalarType::Boolean), kind)? else {
            return Err(self.internal("an equality produced no value"));
        };
        let negation = self.operation_identity(source);
        self.emit(
            negation,
            result,
            OperationKind::BooleanNot { operand: equal },
        )
    }
}

/// Realize the same primitive computation that defines a saturating shift's
/// contract term. Each partial node gets the existing safety obligation.
struct IntegerOperations<'a, 'program> {
    lowering: &'a mut Lowering<'program>,
    source: SourceOrigin,
}

impl crate::policy::IntegerComputation for IntegerOperations<'_, '_> {
    type Value = ValueId;
    type Error = terminal_check_tables::Refusal;

    fn constant(
        &mut self,
        carrier: semantic_vocabulary::IntegerType,
        value: semantic_vocabulary::IntegerValue,
    ) -> Checked<ValueId> {
        let operation = self.lowering.operation_identity(self.source);
        let value = self.lowering.emit(
            operation,
            Some(ScalarType::Integer(carrier)),
            OperationKind::IntegerConstant { value },
        )?;
        match value {
            Value::Scalar(value) => Ok(value),
            _ => Err(self
                .lowering
                .internal("an integer constant has no scalar result")),
        }
    }

    fn binary(
        &mut self,
        operation: Strict,
        policy: Option<Policy>,
        carrier: semantic_vocabulary::IntegerType,
        left: ValueId,
        right: ValueId,
    ) -> Checked<ValueId> {
        let scalar = ScalarType::Integer(carrier);
        let value = match policy {
            Some(policy) => self.lowering.policy_operation(
                operation,
                policy,
                left,
                right,
                Some(scalar),
                self.source,
            )?,
            None => {
                self.lowering
                    .strict(operation, scalar, left, right, Some(scalar), self.source)?
            }
        };
        match value {
            Value::Scalar(value) => Ok(value),
            _ => Err(self
                .lowering
                .internal("an integer computation has no scalar result")),
        }
    }
}

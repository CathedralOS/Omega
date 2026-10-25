//! Exact invocation predicates over Boolean fields, total integer
//! comparisons, and IEEE comparisons with exact literal endpoints. Each
//! comparison operand is resolved once: its resolved carrier selects the
//! integer or IEEE family and the same resolution lowers it.
use crate::values::operator_is_builtin;

mod logical_matches;

use checked_trees::{
    CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use symbols::BuiltinTypeAtom;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn lower_machine_entry_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    expression: ExpressionHandle,
    _exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    // Unread static/evidence binders do not erase concrete entry clauses.
    // Reader rejoins every observed leaf to its exact formal type and meaning.
    let parameters = super::authored_entry_parameters(program, machine)?;
    let predicate = Reader {
        program,
        operators,
        machine: Some(machine),
        state: None,
        owner: machine.symbol,
        parameters,
        remaining: 4096,
        subjects: None,
        proof_context: true,
        logical_captures: Vec::new(),
    }
    .boolean(expression, 0)?;
    Some(predicate)
}

/// Read pure logical predicates against the current state's declared frame.
/// This supplies no entry-value equation and never rereads local initializers;
/// the caller binds observations at its actual statement and live contexts.
pub(crate) fn lower_current_state_predicate(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if statement_index
        > program
            .statement_table
            .statements(state.statement_nodes)
            .len()
    {
        return None;
    }
    Reader {
        program,
        operators,
        machine: Some(machine),
        state: Some(state),
        owner: state.symbol,
        parameters: program.state_parameters(state),
        remaining: 4096,
        subjects: None,
        proof_context: false,
        logical_captures: Vec::new(),
    }
    .boolean(expression, 0)
}

type EntrySubjectResolver<'program> = dyn FnMut(ExpressionHandle) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>
    + 'program;

/// Read the same predicate algebra while the caller supplies authenticated
/// invocation-entry subjects for observations made in a body state.
pub(crate) fn lower_entry_predicate(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    subjects: &mut EntrySubjectResolver<'_>,
) -> Option<CheckedBooleanExpression> {
    let parameters = super::authored_entry_parameters(program, machine)?;
    let predicate = Reader {
        program,
        operators,
        machine: Some(machine),
        state: Some(state),
        owner: machine.symbol,
        parameters,
        remaining: 4096,
        subjects: Some(subjects),
        proof_context: false,
        logical_captures: Vec::new(),
    }
    .boolean(expression, 0)?;
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
        .then_some(predicate)
}

/// Scalar actuals use the ordinary checked algebra with observations rebound
/// to authenticated invocation-entry subjects. This does not replay stored
/// initializer expressions or establish arithmetic safety by itself.
pub(crate) fn lower_entry_scalar(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &typed_trees::state::State,
    before_statement: usize,
    expression: ExpressionHandle,
    primitive_type: PrimitiveType,
    subjects: &impl Fn(
        &validation::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)>,
) -> Option<CheckedScalarExpression> {
    if primitive_type == PrimitiveType::Bool {
        return lower_entry_predicate(
            program,
            operators,
            machine,
            state,
            expression,
            &mut |leaf| {
                let place = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    before_statement,
                    leaf,
                )?;
                subjects(&place)
            },
        )
        .map(|value| CheckedScalarExpression::Boolean(Box::new(value)));
    }
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let mut reader = Reader {
        program,
        operators,
        machine: Some(machine),
        state: Some(state),
        owner: machine.symbol,
        parameters: super::authored_entry_parameters(program, machine)?,
        remaining: 4096,
        subjects: None,
        proof_context: true,
        logical_captures: Vec::new(),
    };
    crate::values::lower_unit_scalar_argument_for_places(
        program,
        operators,
        state,
        before_statement,
        expression,
        primitive_type,
        |place, primitive| {
            let (position, path) = subjects(place)?;
            let parameter_position = usize::try_from(position).ok()?;
            let parameter = reader.parameters.get(parameter_position)?;
            if parameter.is_const || parameter.relevance.is_erased() {
                return None;
            }
            let type_reference = super::super::structural_fields::readable_entry_parameter_path(
                program,
                reader.parameters,
                position,
                &path,
                &mut reader.remaining,
            )?;
            if reader.primitive(type_reference, 0)??.0 != primitive {
                return None;
            }
            Some(if path.is_empty() {
                CheckedScalarExpression::Parameter {
                    position: crate::values::scalar::scalar_position_count(
                        program,
                        &reader.parameters[..parameter_position],
                    ),
                    primitive_type: primitive,
                }
            } else {
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: position,
                    path,
                    primitive_type: primitive,
                }
            })
        },
    )
}

struct Reader<'program> {
    program: &'program dyn ::validation::CheckingInput,
    operators: &'program CheckedOperatorFacts,
    machine: Option<&'program Machine>,
    state: Option<&'program typed_trees::state::State>,
    owner: symbols::SymbolHandle,
    parameters: &'program [StateParameter],
    remaining: usize,
    subjects: Option<&'program mut EntrySubjectResolver<'program>>,
    proof_context: bool,
    logical_captures: Vec<LogicalMatchBinding>,
}

/// Bodyless requirements have an exact signature namespace, not a synthetic
/// machine body. Scalar and structural leaves retain that signature's exact
/// authored formal namespace.
pub(crate) fn lower_signature_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    signature: &typed_trees::signature::StateSignature,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if !signature.symbol.is_valid()
        || !program
            .state_signature_type_parameters(signature)
            .is_empty()
    {
        return None;
    }
    lower_declaration_crash_contract_expression(
        program,
        operators,
        signature.symbol,
        program.state_signature_parameters(signature),
        expression,
    )
}

/// One closed authored formal telescope lowers a declaration's crash routes;
/// both bodyless entrypoints supply only their own closedness precondition.
fn lower_declaration_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    owner: symbols::SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    for (position, parameter) in parameters.iter().enumerate() {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol)
        {
            return None;
        }
    }
    Reader {
        program,
        operators,
        machine: None,
        state: None,
        owner,
        parameters,
        remaining: 4096,
        subjects: None,
        proof_context: true,
        logical_captures: Vec::new(),
    }
    .boolean(expression, 0)
}

/// An operator declaration publishes its crash routes over its own formal
/// parameters, exactly as a bodyless signature does: the structured form
/// binds scalar formals by dense scalar position, which the Terminal
/// operation-contract telescope reads as formal `k + 1`. Structural leaves
/// retain authored formal positions. A generic operator has no closed
/// telescope yet, so it keeps identity without inventing executable operands.
pub(crate) fn lower_operator_crash_contract_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    operator: &typed_trees::operator::OperatorDefinition,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if !operator.symbol.is_valid()
        || !program.operator_type_parameters(operator).is_empty()
        || !operator.lifetime_parameters.is_empty()
    {
        return None;
    }
    lower_declaration_crash_contract_expression(
        program,
        operators,
        operator.symbol,
        program.operator_parameters(operator),
        expression,
    )
}

#[derive(Clone)]
struct FieldPath {
    parameter_position: usize,
    path: Vec<CheckedStructuralPredicatePathSegment>,
    type_reference: TypeReferenceHandle,
}

struct LogicalMatchBinding {
    capture: validation::LogicalRecordCapture,
    observed: FieldPath,
}

/// One comparison operand, resolved once: its carrier selects the comparison
/// family and the same resolution lowers it.
enum ComparisonOperand {
    FloatLiteral {
        value: CheckedScalarExpression,
        reference: TypeReferenceHandle,
    },
    /// A subject read; `static_subject` is present when the read needs no
    /// runtime selector, the only read form IEEE comparisons admit.
    Subject {
        static_subject: Option<FieldPath>,
        value: CheckedScalarExpression,
        reference: TypeReferenceHandle,
        integer: bool,
        ieee: bool,
    },
    IntegerLiteral,
    Unsupported,
}

impl ComparisonOperand {
    fn is_ieee(&self) -> bool {
        matches!(self, ComparisonOperand::FloatLiteral { .. })
            || matches!(self, ComparisonOperand::Subject { ieee: true, .. })
    }

    fn is_integer(&self) -> bool {
        matches!(self, ComparisonOperand::IntegerLiteral)
            || matches!(self, ComparisonOperand::Subject { integer: true, .. })
    }

    /// The IEEE operand this resolution supplies.
    fn ieee_operand(self) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
        match self {
            ComparisonOperand::FloatLiteral { value, reference } => Some((value, reference)),
            ComparisonOperand::Subject {
                static_subject: Some(_),
                ieee: true,
                value,
                reference,
                ..
            } => Some((value, reference)),
            _ => None,
        }
    }
}

impl<'program> Reader<'program> {
    fn charge(&mut self, depth: usize) -> Option<()> {
        if depth >= 64 || self.remaining == 0 {
            self.remaining = 0;
            return None;
        }
        self.remaining -= 1;
        Some(())
    }

    fn boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        self.charge(depth)?;
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        // Proof context selects Id on complete data. Runtime guards never
        // enter this branch, even when their operands have entry provenance.
        if self.proof_context
            && let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && let Some(identity) = self.data_identity(binary.left, binary.right, depth + 1)
        {
            return Some(if binary.operator == BinaryOperator::NotEqual {
                CheckedBooleanExpression::Not(Box::new(identity))
            } else {
                identity
            });
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Match(_) => self.logical_match_boolean(expression, depth + 1),
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::SavedSubject(_) => {
                let subject = self.subject(expression, depth + 1)?;
                if self.primitive(subject.type_reference, depth + 1)?
                    != Some((PrimitiveType::Bool, BuiltinTypeAtom::Bool))
                {
                    return None;
                }
                if subject.path.is_empty()
                    && super::super::occupies_scalar_position(
                        self.program,
                        &self.parameters[subject.parameter_position],
                    )
                {
                    Some(CheckedBooleanExpression::Parameter {
                        position: self.scalar_position(subject.parameter_position, depth + 1)?,
                    })
                } else {
                    Some(CheckedBooleanExpression::StructuralParameterField {
                        parameter_position: u32::try_from(subject.parameter_position).ok()?,
                        path: subject.path,
                    })
                }
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) && operator_is_builtin(self.operators, expression)
                    && self.machine.is_none_or(|machine| {
                        validation::has_builtin_binary_expression_meaning(
                            self.program,
                            machine,
                            self.state
                                .or_else(|| self.program.machine_states(machine).first()),
                            expression,
                        )
                    }) =>
            {
                if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    // Each comparison operand is resolved once, and its
                    // carrier selects the family that lowers it.
                    let operands = [
                        self.comparison_operand(binary.left, depth + 1)?,
                        self.comparison_operand(binary.right, depth + 1)?,
                    ];
                    let ieee = operands.iter().any(ComparisonOperand::is_ieee);
                    if ieee
                        || operands.iter().any(ComparisonOperand::is_integer)
                        || matches!(
                            binary.operator,
                            BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                        )
                    {
                        self.comparison_telescope(depth + 1)?;
                        return if ieee {
                            self.float_comparison(expression, operands)
                        } else {
                            self.integer_comparison(expression, operands)
                        };
                    }
                }
                let left = Box::new(self.boolean(binary.left, depth + 1)?);
                let right = Box::new(self.boolean(binary.right, depth + 1)?);
                if self.machine.is_none()
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                    && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.owner,
                        expression,
                        if binary.operator == BinaryOperator::Equal {
                            language_core::OperatorSpelling::Equal
                        } else {
                            language_core::OperatorSpelling::NotEqual
                        },
                        &[None, None],
                    )
                {
                    return None;
                }
                Some(match binary.operator {
                    BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                    BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                    BinaryOperator::Equal => CheckedBooleanExpression::Equal { left, right },
                    BinaryOperator::NotEqual => {
                        CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Equal {
                            left,
                            right,
                        }))
                    }
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    fn data_identity(
        &mut self,
        left: ExpressionHandle,
        right: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        use super::super::machine_parameter_booleans::structural_equality::subject_type;
        let left = self.subject(left, depth)?;
        let right = self.subject(right, depth)?;
        let left_type = subject_type(self.program, left.type_reference, left.path.is_empty())?;
        let right_type = subject_type(self.program, right.type_reference, right.path.is_empty())?;
        if left_type != right_type {
            return None;
        }
        let subject = |field: FieldPath| {
            Some(checked_trees::CheckedStructuralParameterField {
                parameter_position: u32::try_from(field.parameter_position).ok()?,
                path: field.path,
            })
        };
        Some(CheckedBooleanExpression::StructuralIdentity {
            left: subject(left)?,
            right: subject(right)?,
        })
    }

    /// One comparison operand resolved once; the family its carrier selects
    /// also lowers it.
    fn comparison_operand(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ComparisonOperand> {
        self.charge(depth)?;
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Match(_) => self.logical_match_operand(expression, depth + 1),
            ExpressionNode::Float(literal) => {
                let reference = validation::parameter_expression_result_type_reference(
                    self.program,
                    self.owner,
                    self.parameters,
                    expression,
                )?;
                let value = CheckedScalarExpression::IeeeFloatLiteral {
                    value: typed_trees::expression::ieee_float_value(literal, literal.landing()?),
                };
                Some(ComparisonOperand::FloatLiteral { value, reference })
            }
            ExpressionNode::Integer(_) => Some(ComparisonOperand::IntegerLiteral),
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::SavedSubject(_) => {
                // Only a read with no runtime selector has an IEEE form.
                let static_subject = self.subject(expression, depth + 2);
                let (value, reference, atom) =
                    self.resolved_subject(expression, static_subject.as_ref(), depth + 1)?;
                Some(ComparisonOperand::Subject {
                    static_subject,
                    value,
                    reference,
                    integer: atom.is_some_and(fixed_integer_atom),
                    ieee: atom.is_some_and(ieee_float_atom),
                })
            }
            _ => Some(ComparisonOperand::Unsupported),
        }
    }

    /// Dense scalar positions depend on the entire entry telescope, including
    /// unread formals; both families share this validation.
    fn comparison_telescope(&mut self, depth: usize) -> Option<()> {
        for parameter in self.parameters {
            if parameter.is_const
                || self.program.symbols.name(parameter.symbol) != parameter.name.as_str()
            {
                return None;
            }
            self.primitive(parameter.type_reference, depth + 1)?;
        }
        Some(())
    }

    fn integer_comparison(
        &self,
        expression: ExpressionHandle,
        operands: [ComparisonOperand; 2],
    ) -> Option<CheckedBooleanExpression> {
        let mut subjects = [None, None];
        for (position, operand) in operands.into_iter().enumerate() {
            match operand {
                ComparisonOperand::Subject {
                    value,
                    reference,
                    integer: true,
                    ..
                } => subjects[position] = Some((value, reference)),
                ComparisonOperand::IntegerLiteral => {}
                // This slice establishes no totality for arithmetic, calls,
                // casts, result values, or current body storage.
                _ => return None,
            }
        }
        // Literal landing, same-carrier checks, and selected operator meaning
        // stay with the existing numeric contract owner. Trapping-qualified
        // inputs are legal: the comparison itself is a total operation.
        super::super::result_contract::lower_integer_contract_comparison(
            self.program,
            self.operators,
            self.owner,
            expression,
            subjects,
        )
    }

    /// IEEE relations retain their own meaning, including unordered inputs.
    /// Read exact formal identities and already-landed literal bits; never
    /// reinterpret them as integer order or round them through a host float.
    /// The shared contract owner checks both carriers and the meaning.
    fn float_comparison(
        &self,
        expression: ExpressionHandle,
        operands: [ComparisonOperand; 2],
    ) -> Option<CheckedBooleanExpression> {
        let [left, right] = operands;
        super::super::result_contract::lower_float_contract_comparison(
            self.program,
            self.owner,
            expression,
            [left.ieee_operand(), right.ieee_operand()],
        )
    }

    /// Read ordered selections with the ordinary scalar projection owner.
    /// Authenticate the collection and every selector separately: neither a
    /// stable collection nor a saved selector alone authenticates their read.
    /// This translation supplies meaning, never an indexing bound; the
    /// caller's already-resolved static subject keeps the authored walk to
    /// one pass per operand.
    fn resolved_subject(
        &mut self,
        expression: ExpressionHandle,
        subject: Option<&FieldPath>,
        depth: usize,
    ) -> Option<(
        CheckedScalarExpression,
        TypeReferenceHandle,
        Option<BuiltinTypeAtom>,
    )> {
        self.charge(depth)?;
        if let Some(subject) = subject {
            let (primitive, atom) = self.primitive(subject.type_reference, depth + 1)??;
            let value = self.subject_scalar(subject, primitive, depth + 1)?;
            return Some((value, subject.type_reference, Some(atom)));
        }
        let source_parameters = self
            .state
            .map(|state| self.program.state_parameters(state))
            .unwrap_or(self.parameters);
        let program = self.program;
        let operators = self.operators;
        let (mut value, _) = super::super::scalar_lowering::scalar_storage_read_with_selectors(
            program,
            operators,
            expression,
            source_parameters,
            |_, selector, _| {
                let selector_subject = self.subject(selector, depth + 2);
                self.resolved_subject(selector, selector_subject.as_ref(), depth + 1)
                    .map(|(value, _, _)| value)
                    .or_else(|| {
                        super::super::scalar_lowering::land_anonymous_scalar_expression(
                            program,
                            operators,
                            selector,
                            PrimitiveType::U64,
                        )
                    })
            },
        )?;
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            primitive_type,
            ..
        } = &mut value
        else {
            return None;
        };
        // Find the source root by exact expression identity, then let the
        // entry-provenance resolver authenticate it at the evaluation site.
        let mut cursor = expression;
        loop {
            self.charge(depth)?;
            match program.expression_table.expression(cursor) {
                ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
                ExpressionNode::Member(member) => cursor = member.receiver,
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        let subject = self.subject(cursor, depth + 1)?;
        let mut entry_path = subject.path;
        entry_path.append(path);
        *path = entry_path;
        *root = checked_trees::CheckedStorageRoot::Parameter {
            index: u32::try_from(subject.parameter_position).ok()?,
        };
        let reference = if let (Some(machine), Some(state)) = (self.machine, self.state) {
            validation::declared_place_type_raw(program, machine, Some(state), expression)?
        } else {
            validation::parameter_expression_result_type_reference(
                program,
                self.owner,
                source_parameters,
                expression,
            )?
        };
        let reference = validation::field_domains::readable_type_reference(program, reference)?;
        let (primitive, atom) = self.primitive(reference, depth + 1)??;
        (primitive == *primitive_type).then_some((value, reference, Some(atom)))
    }

    // Scalar positions are dense; structural roots retain the authored formal
    // position. Both integer and IEEE leaves use this same entry namespace.
    fn subject_scalar(
        &mut self,
        subject: &FieldPath,
        primitive_type: PrimitiveType,
        depth: usize,
    ) -> Option<CheckedScalarExpression> {
        Some(
            if subject.path.is_empty()
                && super::super::occupies_scalar_position(
                    self.program,
                    &self.parameters[subject.parameter_position],
                )
            {
                CheckedScalarExpression::Parameter {
                    position: self.scalar_position(subject.parameter_position, depth + 1)?,
                    primitive_type,
                }
            } else {
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: u32::try_from(subject.parameter_position).ok()?,
                    path: subject.path.clone(),
                    primitive_type,
                }
            },
        )
    }

    fn scalar_position(&mut self, position: usize, depth: usize) -> Option<usize> {
        let mut scalar_position = 0;
        for parameter in &self.parameters[..position] {
            self.primitive(parameter.type_reference, depth + 1)?;
            if super::super::occupies_scalar_position(self.program, parameter) {
                scalar_position += 1;
            }
        }
        Some(scalar_position)
    }

    fn subject(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
        if !self.logical_captures.is_empty() && self.is_saved_projection(expression) {
            return self.logical_saved_path(expression, depth + 1);
        }
        if let Some(resolve) = self.subjects.as_deref_mut() {
            let (position, path) = resolve(expression)?;
            let parameter = self.parameters.get(usize::try_from(position).ok()?)?;
            if parameter.is_const {
                return None;
            }
            let type_reference = super::super::structural_fields::readable_entry_parameter_path(
                self.program,
                self.parameters,
                position,
                &path,
                &mut self.remaining,
            )?;
            return Some(FieldPath {
                parameter_position: usize::try_from(position).ok()?,
                path,
                type_reference,
            });
        }
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            let position = self.parameter(expression)?;
            Some(FieldPath {
                parameter_position: position,
                path: Vec::new(),
                // A bare reference formal observes its readable referent in
                // the structural lane, including an empty projection path.
                // It never becomes an invented dense scalar parameter.
                type_reference: validation::field_domains::readable_type_reference(
                    self.program,
                    super::super::structural_fields::readable_entry_parameter_path(
                        self.program,
                        self.parameters,
                        u32::try_from(position).ok()?,
                        &[],
                        &mut self.remaining,
                    )?,
                )?,
            })
        } else {
            self.field_path(expression, depth)
        }
    }

    fn parameter(&self, expression: ExpressionHandle) -> Option<usize> {
        let name = self.program.expression_table.name_path_node(expression)?;
        let [spelling] = self
            .program
            .expression_table
            .name_path_members(name.members)
        else {
            return None;
        };
        self.parameters.iter().position(|parameter| {
            !parameter.is_const
                && parameter.name.as_str() == spelling.as_str()
                && self.program.symbols.name(parameter.symbol) == parameter.name.as_str()
                && { name.symbol == parameter.symbol && name.head_symbol == parameter.symbol }
        })
    }

    fn field_path(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
        // Bound the authored place before the shared canonical walk. Body
        // observations use the authenticated subject callback above instead,
        // so writes cannot turn current storage into an entry observation.
        let mut current = expression;
        let mut depth = depth;
        loop {
            self.charge(depth)?;
            if !self.program.expression_table.expression_is_valid(current) {
                return None;
            }
            match self.program.expression_table.expression(current) {
                ExpressionNode::Member(member) => current = member.receiver,
                ExpressionNode::Indexed(indexed) => current = indexed.collection,
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
            depth += 1;
        }
        let (position, path, selected_type) =
            super::super::structural_fields::structural_parameter_place(
                self.program,
                self.parameters,
                expression,
            )?;
        let type_reference = super::super::structural_fields::readable_entry_parameter_path(
            self.program,
            self.parameters,
            position,
            &path,
            &mut self.remaining,
        )?;
        (type_reference == selected_type).then_some(FieldPath {
            parameter_position: usize::try_from(position).ok()?,
            path,
            type_reference,
        })
    }

    fn primitive(
        &mut self,
        mut type_reference: TypeReferenceHandle,
        mut depth: usize,
    ) -> Option<Option<(PrimitiveType, BuiltinTypeAtom)>> {
        loop {
            self.charge(depth)?;
            if !self
                .program
                .type_reference_table
                .contains_type_reference(type_reference)
            {
                return None;
            }
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, name } => {
                    let Some(primitive) = PrimitiveType::from_name(name) else {
                        return Some(None);
                    };
                    let atom = self.program.symbols.builtin_type_atom(*symbol)?;
                    if self.program.symbols.name(*symbol) != name.as_str()
                        || PrimitiveType::from_name(self.program.symbols.name(*symbol))
                            != Some(primitive)
                    {
                        return None;
                    }
                    let expected = match atom {
                        BuiltinTypeAtom::Bool | BuiltinTypeAtom::AtomicBool => PrimitiveType::Bool,
                        BuiltinTypeAtom::I8 => PrimitiveType::I8,
                        BuiltinTypeAtom::I16 => PrimitiveType::I16,
                        BuiltinTypeAtom::I32 => PrimitiveType::I32,
                        BuiltinTypeAtom::I64 => PrimitiveType::I64,
                        BuiltinTypeAtom::U8 => PrimitiveType::U8,
                        BuiltinTypeAtom::U16 => PrimitiveType::U16,
                        BuiltinTypeAtom::U32 | BuiltinTypeAtom::AtomicU32 => PrimitiveType::U32,
                        BuiltinTypeAtom::U64 | BuiltinTypeAtom::AtomicU64 => PrimitiveType::U64,
                        BuiltinTypeAtom::F32 => PrimitiveType::F32,
                        BuiltinTypeAtom::F64 => PrimitiveType::F64,
                        BuiltinTypeAtom::Address => PrimitiveType::Addr,
                        _ => return None,
                    };
                    return (primitive == expected).then_some(Some((primitive, atom)));
                }
                _ => return Some(None),
            }
            depth += 1;
        }
    }
}

fn fixed_integer_atom(atom: BuiltinTypeAtom) -> bool {
    matches!(
        atom,
        BuiltinTypeAtom::I8
            | BuiltinTypeAtom::I16
            | BuiltinTypeAtom::I32
            | BuiltinTypeAtom::I64
            | BuiltinTypeAtom::U8
            | BuiltinTypeAtom::U16
            | BuiltinTypeAtom::U32
            | BuiltinTypeAtom::U64
    )
}

fn ieee_float_atom(atom: BuiltinTypeAtom) -> bool {
    matches!(atom, BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64)
}

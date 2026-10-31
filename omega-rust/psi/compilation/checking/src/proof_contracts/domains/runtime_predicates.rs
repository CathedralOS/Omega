//! Exact predicate projection of a scalar membership requirement. The source
//! domain and its subject remain separate from the projected Boolean clauses:
//! equal predicates do not identify domains, and predicates cannot mint routed
//! membership. Both checked production and receiving replay use this owner.

use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind as Comparison, CheckedScalarExpression as Scalar,
};
use typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// The exact declared carrier of a membership whose complete meaning fits this
/// predicate package. Closed indices retain their exact application. Routed
/// theory needs separate evidence; declining never drops unsupported rows.
pub fn scalar_membership_carrier(
    program: &dyn crate::CheckingInput,
    membership: &ProofMembershipFact,
) -> Option<PrimitiveType> {
    if crate::field_domains::domain_requires_provenance(program, membership.domain_symbol) {
        return None;
    }
    scalar_membership_value_carrier(program, membership)
}

/// The payload carrier of the exact application, independently of provenance.
/// Callers retaining membership must separately preserve its nominal obligation.
pub fn scalar_membership_value_carrier(
    program: &dyn crate::CheckingInput,
    membership: &ProofMembershipFact,
) -> Option<PrimitiveType> {
    let arguments = program
        .type_reference_table
        .type_reference_handles(membership.domain_arguments);
    if arguments.len() != membership.domain_arguments.len() {
        return None;
    }
    Some(
        scalar_application(
            program,
            membership.domain_symbol,
            membership.semantic_domain,
            arguments,
        )?
        .primitive,
    )
}

struct ScalarApplication<'program> {
    declaration: &'program DomainDefinition,
    primitive: PrimitiveType,
    indices: Vec<(symbols::SymbolHandle, numerics::bignum::BigInt)>,
}

/// Only the declaration's own value clauses. Every skipped same-subject
/// membership must be retained separately by the caller's domain catalog;
/// this projection supplies no parent membership or establishment authority.
pub fn scalar_domain_local_predicate(
    program: &dyn crate::CheckingInput,
    domain: &DomainDefinition,
) -> Option<Boolean> {
    typed_trees::domain::required_subject_memberships(program.typed_program(), domain)?;
    let facts = program.proof_facts(domain);
    if facts
        .iter()
        .all(|fact| matches!(fact, ProofFact::Membership(_)))
    {
        return Some(Boolean::Constant(true));
    }
    // Open index operands are not closed by their name or receiving carrier.
    if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
        return None;
    }
    let primitive = program.primitive_type_reference(domain.target_type)?;
    let subject = Scalar::Parameter {
        position: 0,
        primitive_type: primitive,
    };
    let spelling = Predicate {
        program,
        domain,
        subject: &subject,
        primitive,
        indices: Vec::new(),
        substituted_formal: symbols::SymbolHandle::invalid(),
    };
    let mut predicates = Vec::new();
    for fact in facts {
        match fact {
            ProofFact::Expression(expression) => {
                predicates.push(spelling.boolean(*expression, 0)?);
            }
            ProofFact::Membership(_) => {}
            ProofFact::Proposition(_) => return None,
        }
    }
    Some(
        predicates
            .into_iter()
            .reduce(conjunction)
            .unwrap_or(Boolean::Constant(true)),
    )
}

/// Canonical full value component of one retained closed application. Unlike
/// a use-site restriction, its argument pack comes from the source instance
/// owner. Nominal and establishment duties remain separate from this value.
pub fn scalar_domain_definition_predicate(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> Option<Boolean> {
    let definition = program
        .typed_program()
        .domain_definition_for_instance(symbol, identity)?;
    let arguments = program
        .typed_program()
        .domain_arguments_for_instance(symbol, identity)?;
    let primitive_type = program.primitive_type_reference(definition.target_type)?;
    let subject = Scalar::Parameter {
        position: 0,
        primitive_type,
    };
    let predicates = project(
        program,
        symbol,
        identity,
        arguments,
        &subject,
        &mut Vec::new(),
    )?;
    Some(
        predicates
            .into_iter()
            .reduce(conjunction)
            .unwrap_or(Boolean::Constant(true)),
    )
}

/// Select the exact application once before projecting any of its predicates.
/// Closed index values keep their declaration identities and primitive bounds.
fn scalar_application<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    arguments: &[TypeReferenceHandle],
) -> Option<ScalarApplication<'program>> {
    let domain = typed_trees::domain::domain_by_symbol(program, symbol)?;
    if program
        .domain_definitions()
        .iter()
        .filter(|row| row.symbol == domain.symbol)
        .count()
        != 1
        || !domain.symbol.is_valid()
        || program.symbols.get(domain.symbol).kind != symbols::SymbolKind::Domain
        || domain.alias.is_some()
        || !super::scalar_tags::exact_instance(program, domain, arguments, semantic_domain)
    {
        return None;
    }
    let indices = if arguments.is_empty() {
        Vec::new()
    } else {
        super::closed_integer_index_bindings(program, domain, arguments, semantic_domain)?
    };
    Some(ScalarApplication {
        declaration: domain,
        primitive: crate::value_custody::recasts::exact_primitive_type(
            program,
            domain.target_type,
        )?,
        indices,
    })
}

/// Project every authored predicate, in declaration order, under one already
/// checked scalar subject. The caller separately rejoins that subject to the
/// membership's source expression; this query grants no value or provenance.
pub fn scalar_membership_predicates(
    program: &dyn crate::CheckingInput,
    membership: &ProofMembershipFact,
    subject: &Scalar,
) -> Option<Vec<Boolean>> {
    scalar_membership_carrier(program, membership)?;
    scalar_membership_value_predicates(program, membership, subject)
}

/// Project only the complete value predicate. Unlike predicate-only membership,
/// this projection grants no routed identity or establishment authority.
pub fn scalar_membership_value_predicates(
    program: &dyn crate::CheckingInput,
    membership: &ProofMembershipFact,
    subject: &Scalar,
) -> Option<Vec<Boolean>> {
    scalar_membership_value_carrier(program, membership)?;
    let arguments = program
        .type_reference_table
        .type_reference_handles(membership.domain_arguments);
    if arguments.len() != membership.domain_arguments.len() {
        return None;
    }
    project(
        program,
        membership.domain_symbol,
        membership.semantic_domain,
        arguments,
        subject,
        &mut Vec::new(),
    )
}

fn project(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    arguments: &[TypeReferenceHandle],
    subject: &Scalar,
    active: &mut Vec<symbols::SymbolHandle>,
) -> Option<Vec<Boolean>> {
    let application = scalar_application(program, symbol, semantic_domain, arguments)?;
    if subject.primitive_type() != Some(application.primitive) || active.contains(&symbol) {
        return None;
    }
    active.push(symbol);
    let domain = application.declaration;
    let spelling = Predicate {
        program,
        domain,
        subject,
        primitive: application.primitive,
        indices: application.indices,
        substituted_formal: symbols::SymbolHandle::invalid(),
    };
    let mut predicates = Vec::new();
    for fact in program.proof_facts.span(domain.facts)? {
        match fact {
            ProofFact::Expression(expression) => predicates.push(spelling.boolean(*expression, 0)?),
            ProofFact::Membership(nested) => {
                // Nested routed membership needs its own provenance theory;
                // flattening only its predicates would lose that obligation.
                scalar_membership_carrier(program, nested)?;
                if crate::exact_domain_subject_type(program, domain, nested.value).is_none() {
                    return None;
                }
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(nested.domain_arguments);
                if arguments.len() != nested.domain_arguments.len() {
                    return None;
                }
                predicates.extend(project(
                    program,
                    nested.domain_symbol,
                    nested.semantic_domain,
                    arguments,
                    subject,
                    active,
                )?);
            }
            ProofFact::Proposition(_) => return None,
        }
    }
    active.pop();
    Some(predicates)
}

struct Predicate<'program> {
    program: &'program dyn crate::CheckingInput,
    domain: &'program DomainDefinition,
    subject: &'program Scalar,
    primitive: PrimitiveType,
    indices: Vec<(symbols::SymbolHandle, numerics::bignum::BigInt)>,
    /// Callee formal currently standing for the domain subject. Zero means the
    /// expression is still in the domain's own predicate.
    substituted_formal: symbols::SymbolHandle,
}

impl Predicate<'_> {
    fn boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        if self
            .program
            .prepared_mathematics()
            .is_some_and(|view| view.application(expression).is_some())
        {
            return crate::mathematical_applications::scalar_predicate(
                self.program,
                expression,
                |capture, _| self.scalar(capture, depth + 1),
            );
        }
        if matches!(self.primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            return self.float_boolean(expression, depth);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(Boolean::Constant(*value)),
            ExpressionNode::Name(path)
                if self.primitive == PrimitiveType::Bool
                    && (crate::exact_domain_subject_type(
                        self.program,
                        self.domain,
                        expression,
                    )
                    .is_some()
                        || path.symbol == self.substituted_formal) =>
            {
                self.subject_boolean()
            }
            // A predicate call is the selected declaration's boolean body under
            // the exact subject. The declaration owns that body; a matching
            // name or a later caller guard does not.
            ExpressionNode::Call(call) => self.declaration_call(call, depth),
            // A field of that call is the same body under the subject. The
            // receiver of the projection is the call, not a second value.
            ExpressionNode::Member(member) => self.declaration_projection(member, depth),
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && crate::has_builtin_domain_decomposed_guard_meaning(
                        self.program,
                        self.domain,
                        expression,
                    ) =>
            {
                Some(Boolean::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary)
                if crate::has_builtin_domain_decomposed_guard_meaning(
                    self.program,
                    self.domain,
                    expression,
                ) =>
            {
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let left = Box::new(self.boolean(binary.left, depth + 1)?);
                    let right = Box::new(self.boolean(binary.right, depth + 1)?);
                    return Some(if binary.operator == BinaryOperator::And {
                        Boolean::And { left, right }
                    } else {
                        Boolean::Or { left, right }
                    });
                }
                if self.primitive == PrimitiveType::Bool
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                {
                    let value = Boolean::Equal {
                        left: Box::new(self.boolean(binary.left, depth + 1)?),
                        right: Box::new(self.boolean(binary.right, depth + 1)?),
                    };
                    return Some(if binary.operator == BinaryOperator::NotEqual {
                        Boolean::Not(Box::new(value))
                    } else {
                        value
                    });
                }
                let (kind, left, right, negated) = match binary.operator {
                    BinaryOperator::Equal => (Comparison::Equal, binary.left, binary.right, false),
                    BinaryOperator::NotEqual => {
                        (Comparison::Equal, binary.left, binary.right, true)
                    }
                    BinaryOperator::Less => {
                        (Comparison::LessThan, binary.left, binary.right, false)
                    }
                    BinaryOperator::LessOrEqual => {
                        (Comparison::LessOrEqual, binary.left, binary.right, false)
                    }
                    BinaryOperator::Greater => {
                        (Comparison::LessThan, binary.right, binary.left, false)
                    }
                    BinaryOperator::GreaterOrEqual => {
                        (Comparison::LessOrEqual, binary.right, binary.left, false)
                    }
                    _ => return None,
                };
                let comparison = Boolean::IntegerComparison {
                    kind,
                    left: Box::new(self.scalar(left, depth + 1)?),
                    right: Box::new(self.scalar(right, depth + 1)?),
                };
                Some(if negated {
                    Boolean::Not(Box::new(comparison))
                } else {
                    comparison
                })
            }
            _ => None,
        }
    }

    fn subject_boolean(&self) -> Option<Boolean> {
        match self.subject {
            Scalar::Boolean(value) => Some((**value).clone()),
            Scalar::Parameter { position, .. } => Some(Boolean::Parameter {
                position: *position,
            }),
            Scalar::ErasedParameter { position, .. } => Some(Boolean::ErasedParameter {
                position: *position,
            }),
            _ => None,
        }
    }

    /// Project `callee(subject)` through the selected checked body. One formal
    /// receives the subject. A declaration namespace is not a runtime receiver.
    /// Static arguments, evidence, and value receivers stay outside this
    /// scalar predicate.
    fn declaration_call(
        &self,
        call: &typed_trees::expression::TableCallExpression,
        depth: usize,
    ) -> Option<Boolean> {
        let (body, formal) = self.declaration_body(call)?;
        self.clone_with_formal(formal).boolean(body, depth + 1)
    }

    /// `callee(subject).field` reads that field of the selected body. Nested
    /// fields follow the literal; a case payload is a different observation.
    fn declaration_projection(
        &self,
        member: &typed_trees::expression::TableMemberExpression,
        depth: usize,
    ) -> Option<Boolean> {
        if member.case_variant.is_some() {
            return None;
        }
        let mut fields = Vec::new();
        let mut receiver = member.receiver;
        fields.push((member.member_symbol, member.member.as_str().to_owned()));
        let call = loop {
            match self.program.expression_table.expression(receiver) {
                ExpressionNode::Member(inner) if inner.case_variant.is_none() => {
                    fields.push((inner.member_symbol, inner.member.as_str().to_owned()));
                    receiver = inner.receiver;
                }
                ExpressionNode::Call(call) => break call.clone(),
                _ => return None,
            }
        };
        let (mut body, formal) = self.declaration_body(&call)?;
        for (symbol, name) in fields.iter().rev() {
            body = self.literal_field(body, *symbol, name)?;
        }
        self.clone_with_formal(formal).boolean(body, depth + 1)
    }

    fn literal_field(
        &self,
        expression: ExpressionHandle,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Option<ExpressionHandle> {
        let literal = self
            .program
            .expression_table
            .struct_literal_node(expression)?;
        if literal.case_name.is_some() {
            return None;
        }
        let mut selected = None;
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let matches = if symbol.is_valid() && field.field_symbol.is_valid() {
                field.field_symbol == symbol
            } else {
                field.name.as_str() == name
            };
            if !matches {
                continue;
            }
            if selected.is_some() {
                return None;
            }
            selected = Some(field.value);
        }
        selected
    }

    /// The selected body's value and the formal that stands for the subject.
    fn declaration_body(
        &self,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<(ExpressionHandle, symbols::SymbolHandle)> {
        if !self.call_receiver_is_declaration(call)
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return None;
        }
        // A direct conformance call's target is the selected body's entry.
        // An evidence binder has no body here; the public requirement stays
        // the contract and this projector does not invent one.
        if let Some(dispatch) = &call.static_requirement_dispatch {
            if dispatch.evidence_parameter.is_valid()
                || !dispatch.realization_state.is_valid()
                || dispatch.realization_state != call.target_symbol
            {
                return None;
            }
        }
        let (machine, state) =
            crate::machine_calls::calls::machine_state_by_symbol(self.program, call.target_symbol)?;
        if self.program.machine_states(machine).len() != 1 || !machine.body_is_present {
            return None;
        }
        let parameters = self.program.state_parameters(state);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        let [parameter] = parameters else {
            return None;
        };
        let [argument] = arguments else {
            return None;
        };
        if parameter.is_mutable
            || parameter.is_const
            || self
                .program
                .primitive_type_reference(parameter.type_reference)
                != Some(self.primitive)
            || crate::exact_domain_subject_type(self.program, self.domain, *argument).is_none()
        {
            return None;
        }
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let [statement] = statements else {
            return None;
        };
        let body = if let Some(expression) = statement.as_expression() {
            *expression
        } else if let Some(transition) = statement.as_transition() {
            if transition.continuation.is_valid()
                || !matches!(
                    transition.guard,
                    typed_trees::statement::TransitionGuardNode::Always
                )
            {
                return None;
            }
            let typed_trees::statement::TransitionTargetNode::Value(value) = self
                .program
                .statement_table
                .transition_target(transition.target)
            else {
                return None;
            };
            *value
        } else {
            return None;
        };
        Some((body, parameter.symbol))
    }

    /// Module, trait, and implementation qualifiers name the declaration.
    /// A value, borrow, or projection is a runtime receiver and stays out.
    fn call_receiver_is_declaration(
        &self,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        if !call.receiver.is_valid() {
            return true;
        }
        let Some(path) = self.program.expression_table.name_path_node(call.receiver) else {
            return false;
        };
        matches!(
            self.program.symbols.get(path.symbol).kind,
            symbols::SymbolKind::Module
                | symbols::SymbolKind::Trait
                | symbols::SymbolKind::Conformance
                | symbols::SymbolKind::ConformanceParameter
        )
    }

    fn clone_with_formal(&self, formal: symbols::SymbolHandle) -> Self {
        Self {
            program: self.program,
            domain: self.domain,
            subject: self.subject,
            primitive: self.primitive,
            indices: self.indices.clone(),
            substituted_formal: formal,
        }
    }

    /// A predicate on `f32` or `f64` is an IEEE comparison, not an integer
    /// relation. The subject is the parameter; the other operand is a landed
    /// literal in that format. Token-lowered comparisons are calls whose
    /// machine binding is the sealed float operator.
    fn float_boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
        if depth >= 128 {
            return None;
        }
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
        {
            let left = Box::new(self.float_boolean(binary.left, depth + 1)?);
            let right = Box::new(self.float_boolean(binary.right, depth + 1)?);
            return Some(if binary.operator == BinaryOperator::And {
                Boolean::And { left, right }
            } else {
                Boolean::Or { left, right }
            });
        }
        let (operator, left, right) = self.float_comparison(expression)?;
        let (kind, left, right) = match operator {
            BinaryOperator::Equal => (
                checked_trees::CheckedIeeeFloatComparisonKind::Equal,
                left,
                right,
            ),
            BinaryOperator::NotEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::NotEqual,
                left,
                right,
            ),
            BinaryOperator::Less => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessThan,
                left,
                right,
            ),
            BinaryOperator::LessOrEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual,
                left,
                right,
            ),
            BinaryOperator::Greater => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessThan,
                right,
                left,
            ),
            BinaryOperator::GreaterOrEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual,
                right,
                left,
            ),
            _ => return None,
        };
        Some(Boolean::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(self.float_scalar(left)?),
            right: Box::new(self.float_scalar(right)?),
        })
    }

    fn float_comparison(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(BinaryOperator, ExpressionHandle, ExpressionHandle)> {
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        {
            return Some((binary.operator, binary.left, binary.right));
        }
        let call = self.program.expression_table.call_node(expression)?;
        let (machine, _) = crate::find_machine_by_entry_state(self.program, call.target_symbol)?;
        let binding = self
            .program
            .machine_token_binding_by_symbol(machine.symbol)?;
        let (operator, format) =
            typed_trees::operator::primitive_float_binary_semantics(self.program, binding)?;
        let expected = match self.primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        if format != expected {
            return None;
        }
        let [left, right] = self
            .program
            .expression_table
            .expression_handles(call.arguments)
        else {
            return None;
        };
        Some((operator, *left, *right))
    }

    fn float_scalar(&self, expression: ExpressionHandle) -> Option<Scalar> {
        if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some() {
            return Some(self.subject.clone());
        }
        let literal = self.program.expression_table.expression(expression);
        let bits = match literal {
            ExpressionNode::Float(literal) => match self.primitive {
                PrimitiveType::F32 => {
                    semantic_vocabulary::IeeeFloatValue::Binary32(literal.value_f32().to_bits())
                }
                PrimitiveType::F64 => {
                    semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
                }
                _ => return None,
            },
            _ => return None,
        };
        Some(Scalar::IeeeFloatLiteral { value: bits })
    }

    fn scalar(&self, expression: ExpressionHandle, depth: usize) -> Option<Scalar> {
        if depth >= 128
            || !self.primitive.accepts_integer_literal()
            || self.primitive == PrimitiveType::Addr
        {
            return None;
        }
        if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some() {
            return Some(self.subject.clone());
        }
        if let Some(symbol) = crate::single_segment_name_symbol(self.program, expression)
            && let Some((_, value)) = self.indices.iter().find(|(index, _)| *index == symbol)
        {
            // Static values come from the exact canonical application, not a
            // same-spelled runtime name or the receiving field's restriction.
            // Keep the index's own carrier; this projector cannot relabel an
            // incompatible landed value as the subject's primitive.
            let reference =
                crate::value_custody::expression_types::domain_expression_result_type_reference(
                    self.program,
                    self.domain,
                    expression,
                )?;
            if self.program.primitive_type_reference(reference) != Some(self.primitive) {
                return None;
            }
            return Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(value, self.primitive)?,
            });
        }
        if let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        {
            // These operators have no authored overload and are total at the
            // common fixed carrier. Preserve their children as expressions;
            // a domain is not restricted to comparing its bare subject.
            let kind = match binary.operator {
                BinaryOperator::BitwiseAnd => Some(CheckedIntegerBinaryKind::BitwiseAnd),
                BinaryOperator::BitwiseOr => Some(CheckedIntegerBinaryKind::BitwiseOr),
                BinaryOperator::BitwiseXor => Some(CheckedIntegerBinaryKind::BitwiseXor),
                _ => None,
            };
            if let Some(kind) = kind {
                let reference = crate::value_custody::expression_types::domain_expression_result_type_reference(
                    self.program, self.domain, expression,
                )?;
                if self.program.primitive_type_reference(reference) != Some(self.primitive) {
                    return None;
                }
                return Some(Scalar::IntegerBinary {
                    kind,
                    primitive_type: self.primitive,
                    left: Box::new(self.scalar(binary.left, depth + 1)?),
                    right: Box::new(self.scalar(binary.right, depth + 1)?),
                    formation_decided: false,
                    trap_free: false,
                });
            }
        }
        // Literals, typed constants and exact closed casts share the existing
        // declaration-owned numeric query. Never relabel an already-landed
        // incompatible carrier or evaluate a runtime name as a constant.
        let value = self
            .program
            .closed_integer_value_in(expression, self.domain.symbol)?;
        if value
            .primitive
            .is_some_and(|primitive| primitive != self.primitive)
        {
            return None;
        }
        Some(Scalar::IntegerLiteral {
            literal: crate::land_integer_value(&value.value, self.primitive)?,
        })
    }
}

/// Reconstruct a field restriction from its exact declaration. Closed numeric
/// operands retain their own carriers and selected operator meanings.
/// This is source authority, independent of a producer's retained field plan.
/// Compiler-known value refinements use the same subject-bound meaning as
/// callable contracts. Projecting that meaning does not prove membership or
/// change an invariant-window fact into a per-write storage restriction.
pub fn scalar_field_domain_predicate(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    primitive: PrimitiveType,
) -> Option<Boolean> {
    scalar_domain_predicate(
        program,
        constraint,
        Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        },
    )
}

/// Project the complete declaration onto the caller's exact scalar subject.
/// Fields and value contracts share this decoder; projection grants no membership.
pub fn scalar_domain_predicate(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    subject: Scalar,
) -> Option<Boolean> {
    if !constraint.establishment_routes.is_empty() {
        return None;
    }
    scalar_domain_value_predicate(program, constraint, subject)
}

/// The complete predicate component of a declared scalar theory, independently
/// of its issuer routes. Result contracts and the Terminal domain catalog use
/// this projection together with separate checked provenance, never as a
/// membership judgment. Predicate-only callers use `scalar_domain_predicate`.
pub fn scalar_domain_value_predicate(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    subject: Scalar,
) -> Option<Boolean> {
    let primitive = subject.primitive_type()?;
    if let typed_trees::types::DomainConstraintSubject::Value(domain) = constraint.subject {
        return crate::scalar_value_domain_predicate(domain, subject);
    }
    // An exact interval summary states the entire domain. Project it through
    // the same predicate route as other domains, retaining its closed bounds
    // rather than reconstructing selectable source tokens.
    if let Some((minimum, maximum)) =
        crate::exact_declared_domain_carrier_interval(program, primitive, constraint)
    {
        if minimum > maximum {
            return None;
        }
        let endpoint = |value: &numerics::bignum::BigInt| {
            Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(value, primitive)?,
            })
        };
        return Some(conjunction(
            Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(endpoint(&minimum)?),
                right: Box::new(subject.clone()),
            },
            Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(subject),
                right: Box::new(endpoint(&maximum)?),
            },
        ));
    }
    // Static tag identity and indexed arguments remain on the checked type.
    // With no predicates or routes, its storage restriction is exactly true;
    // use the same exact-instance classification as scalar signatures rather
    // than treating every indexed family as an unsupported predicate theory.
    if super::scalar_tags::scalar_tag_constraint(program, constraint, primitive) {
        return Some(Boolean::Constant(true));
    }
    if constraint.subject != typed_trees::types::DomainConstraintSubject::Declared {
        return None;
    }
    let predicates = project(
        program,
        constraint.symbol,
        constraint.semantic_id,
        &constraint.arguments,
        &subject,
        &mut Vec::new(),
    )?;
    Some(
        predicates
            .into_iter()
            .reduce(conjunction)
            .unwrap_or(Boolean::Constant(true)),
    )
}

fn conjunction(left: Boolean, right: Boolean) -> Boolean {
    Boolean::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

/// Assemble the complete restriction after source reconstruction has intersected
/// interval shells. A general predicate keeps both endpoints; interval-only
/// fields retain their compact representation.
pub fn scalar_field_restriction(
    primitive: PrimitiveType,
    bounds: Option<semantic_vocabulary::BoundedIntegerType>,
    predicate: Option<Boolean>,
) -> Option<checked_trees::CheckedUnitStructuralFieldType> {
    use checked_trees::CheckedUnitStructuralFieldType as Field;
    let Some(mut predicate) = predicate else {
        return Some(bounds.map_or(Field::Scalar(primitive), Field::BoundedInteger));
    };
    if let Some(bounds) = bounds {
        let subject = Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        };
        let endpoint = |value| {
            use numerics::bignum::BigInt;
            let value = match value {
                semantic_vocabulary::IntegerValue::Signed(value) => BigInt::from_i128(value),
                semantic_vocabulary::IntegerValue::Unsigned(value) => BigInt::from_u128(value),
            };
            Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(&value, primitive)?,
            })
        };
        predicate = conjunction(
            predicate,
            Boolean::And {
                left: Box::new(Boolean::IntegerComparison {
                    kind: Comparison::LessOrEqual,
                    left: Box::new(endpoint(bounds.minimum())?),
                    right: Box::new(subject.clone()),
                }),
                right: Box::new(Boolean::IntegerComparison {
                    kind: Comparison::LessOrEqual,
                    left: Box::new(subject),
                    right: Box::new(endpoint(bounds.maximum())?),
                }),
            },
        );
    }
    Some(Field::RefinedScalar {
        primitive,
        predicate,
    })
}

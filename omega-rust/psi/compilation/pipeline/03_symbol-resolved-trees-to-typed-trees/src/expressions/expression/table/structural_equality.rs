//! Inline structural-equality expansion (frozen decisions 8 + 11): `==` on a
//! record or payload-bearing sum whose one Equatable implementation carries the
//! compiler-synthesized `equals` row lowers to an AND/OR tree of field
//! compares -- the typed-tree shape the author would have written by hand --
//! so every existing backend and interpreter comparison path is reused with
//! zero new runtime surface.
//!
//! Records expand to a conjunction of per-field compares. Sums expand to a
//! disjunction over cases, each arm a conjunction whose TAG compares come
//! FIRST: under the interpreter's short-circuit AND a mismatched tag skips
//! the payload reads (which would otherwise read another case's payload),
//! while the native backend's eager evaluation reads in-allocation payload
//! bytes whose garbage compare is masked by the false tag compare.
//!
//! An authored `equals` row wins: `==` lowers to an ordinary static call
//! through the selected implementation (`Impl::equals(&left, &right)`), with
//! both compared values as explicit shared operands.
//!
//! Generated tag guards use typed CaseMembership rather than value equality.
//! Their RHS is a classifier, not a payload constructor, and their subject is
//! observed rather than moved. The operation survives ordinary expression
//! copies without fabricating authored membership-selection rows.
//!
//! `[erased]` fields are skipped: they have no runtime value, and the
//! relevance pass (psi/semantics/validation/src/relevance/runtime_uses.rs)
//! would otherwise reject member reads the author never wrote. A record
//! whose every field is erased rejects rather than comparing trivially
//! equal, per the structural_predicates.md equality table.

use super::lowerer::ExpressionTableLowerer;
use crate::expressions::equatable::{
    DataEqualityShape, EqualitySelection, FieldEquality, data_definition_by_name_from,
    data_equality_shape, equality_selection, field_equality, implementation_name,
    suggested_equality_declaration, synthesized_equality_row, value_type_base_name,
};
use crate::lowerer::name::lower_name;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use resolved::data::{DataDefinition, DataField, DataMember, DataVariant};
use symbol_resolved_trees as resolved;
use typed_trees as typed;

/// One `==` operand, classified for expansion. A PLACE is lowered once and
/// its handle shared by every synthesized member read; a LITERAL is never
/// materialized -- its field values are compared directly.
enum Operand {
    Place(typed::expression::ExpressionHandle),
    Literal {
        case_name: Option<String>,
        fields: HandleSpan<resolved::expression::TableStructLiteralField>,
    },
}

impl<'program, 'target, 'scope> ExpressionTableLowerer<'program, 'target, 'scope> {
    /// Expand a call through a synthesized `equals` row
    /// (`PointEquality::equals(&a, &b)`) in the caller's storage scope,
    /// exactly like `==`. Stage 02 routed the call to the row's realization
    /// state; the row identifies the compared data type, so the operator
    /// route's selection plays no part and a second implementation for the
    /// same type does not make this explicit call ambiguous.
    pub(super) fn try_lower_synthesized_equatable_call(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
        call: &resolved::expression::TableCallExpression,
    ) -> Result<Option<typed::expression::ExpressionHandle>, Diagnostic> {
        let Some(program) = self.program else {
            return Ok(None);
        };
        if !call.target_symbol.is_valid() {
            return Ok(None);
        }
        let Some(data) = synthesized_equality_row(program, call.target_symbol) else {
            return Ok(None);
        };
        let [left, right] = self.source.expression_handles(call.arguments) else {
            return Ok(None);
        };
        let (left, right) = (*left, *right);
        // Row selection is not operand typing. An unrelated pair must stay
        // an ordinary call so its declared parameter checks can reject it.
        for operand in [left, right] {
            // A mutable loan may attenuate at the ordinary shared formal, but
            // expanding it into reads would erase its authored formation.
            if matches!(self.source.expression(operand),
                resolved::expression::ExpressionNode::Borrow(borrow)
                    if borrow.access != language_semantics::ReferenceAccess::Shared)
            {
                return Ok(None);
            }
            let Some(type_name) = self.operand_data_type_name(operand) else {
                return Ok(None);
            };
            if data_definition_by_name_from(program, &type_name, self.source.source_span(operand))
                .is_none_or(|operand_data| operand_data.symbol != data.symbol)
            {
                return Ok(None);
            }
        }
        self.finalize_synthesized_equatable_call_selection(expression, call.target_symbol)?;
        // The call observes the referents of its shared arguments. This is
        // one expansion after operand normalization, with each place lowered
        // once by structural equality's ordinary operand owner.
        let observed_operand = |operand| match self.source.expression(operand) {
            resolved::expression::ExpressionNode::Borrow(borrow)
                if borrow.access == language_semantics::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            _ => operand,
        };
        let equality = resolved::expression::TableBinaryExpression {
            left: observed_operand(left),
            operator: resolved::expression::BinaryOperator::Equal,
            right: observed_operand(right),
        };
        if data.quotient.is_none()
            && data_equality_shape(program, data) == DataEqualityShape::Structural
        {
            return self.expand_structural_equality(data, &equality).map(Some);
        }

        // Empty records and payload-less sums use their existing direct value
        // equality. Lower that comparison in the caller too: besides avoiding
        // a needless call, this preserves the value width when a small shared
        // argument is content-spilled into a pointer-sized parameter slot.
        let left = self.lower(equality.left)?;
        let right = self.lower(equality.right)?;
        Ok(Some(self.target().insert(
            typed::expression::ExpressionNode::Binary(typed::expression::TableBinaryExpression {
                left,
                operator: typed::expression::BinaryOperator::Equal,
                right,
            }),
        )))
    }

    fn finalize_synthesized_equatable_call_selection(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
        synthesized_target: symbols::SymbolHandle,
    ) -> Result<(), Diagnostic> {
        let occurrences = self
            .source
            .authored_selection_occurrences(expression)
            .filter(|occurrence| {
                self.target_trees
                    .authored_declaration_selections()
                    .get(*occurrence)
                    .is_some_and(|selection| {
                        selection.kind()
                            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                    })
            })
            .collect::<Vec<_>>();
        let [occurrence] = occurrences.as_slice() else {
            return Err(Diagnostic::error(format!(
                "authored synthesized equality call retains {} call selections; expected one",
                occurrences.len()
            )));
        };

        let mut selections = self.target_trees.authored_declaration_selections().clone();
        let selection = selections.get(*occurrence).ok_or_else(|| {
            Diagnostic::error("authored synthesized equality call lost its selection custody")
        })?;
        match selection.target() {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::LateBound(
                language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding::CheckedCall,
            ) => selections
                .finalize_late_bound(
                    *occurrence,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    synthesized_target,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to bind authored synthesized equality call: {error:?}"
                    ))
                })?,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(
                target,
            ) if target.selected_symbol() == synthesized_target => {}
            _ => {
                return Err(Diagnostic::error(
                    "authored synthesized equality call retains contradictory selection custody",
                ));
            }
        }
        self.target_trees
            .retain_authored_declaration_selections(selections);
        Ok(())
    }

    /// Expand `==`/`!=` when an operand types to a structural (record /
    /// payload-bearing sum) data definition. Returns `Ok(None)` when the
    /// compare is not structural (primitives, payload-less sums, untypable
    /// operands), leaving the ordinary binary lowering in charge.
    pub(super) fn try_lower_structural_equality(
        &mut self,
        binary: &resolved::expression::TableBinaryExpression,
    ) -> Result<Option<typed::expression::ExpressionHandle>, Diagnostic> {
        // Logical identity has its own complete-carrier formation judgment;
        // executable equality selection and its shape checks do not apply.
        if self.fact_position {
            return Ok(None);
        }
        let Some(program) = self.program else {
            return Ok(None);
        };

        // A multi-member name path operand is a CLASSIFIER reference (a case
        // or domain, e.g. the guard-layer desugar of match arms comparing the
        // subject against `Player::Alive`), not a value: the existing tag /
        // domain machinery owns that compare. User-written bare
        // payload-bearing case names were already rejected by the resolved
        // pre-pass (`crate::expressions::equality`).
        if self.classifier_reference_operand(binary.left)
            || self.classifier_reference_operand(binary.right)
        {
            // A classifier reference `Enum::Case` compared against a value lowers to
            // an ordinary TAG compare. If the value is a DIFFERENT enum type, that
            // compares tags across unrelated types (`color == Direction::North` --
            // both tag 0 -> spuriously equal), a silent miscompile. Resolve each
            // side's data type (a classifier's first path segment, or a value's
            // declared type) and reject when both are data definitions that differ.
            // A same-type match-arm desugar (`subject == Player::Alive` for a
            // `Player`) has equal names and is untouched; domain references (not
            // data definitions) are skipped.
            let left_type = self
                .classifier_reference_type(binary.left)
                .or_else(|| self.operand_data_type_name(binary.left));
            let right_type = self
                .classifier_reference_type(binary.right)
                .or_else(|| self.operand_data_type_name(binary.right));
            if let Some((left_name, right_name)) = Self::different_data_type_names(
                program,
                &left_type,
                &right_type,
                self.source.source_span(binary.left),
                self.source.source_span(binary.right),
            ) {
                return Err(Diagnostic::error(format!(
                    "cannot compare `{left_name}` and `{right_name}` with `==`/`!=`: the operands \
                     are different data types (a case belongs to its own type only)"
                )));
            }
            return Ok(None);
        }

        // Reject a compare between two DIFFERENT data types BEFORE expansion.
        // Equality is synthesized by pairing the operands' fields by POSITION using
        // ONE resolved type name, so `P == Q` would silently read `q`'s bytes as if
        // they were a `P` (`p.x == q.<first field> && ..`) and run to a garbage bool.
        // Both operands must name the same type; when only one (or neither) types,
        // fall through to the existing single-name behavior. Case literals resolve to
        // their BASE type name (`Event::Score { .. }` -> `Event`), so a value-vs-case
        // compare of the same sum stays same-named and is not flagged.
        let left_type = self.operand_data_type_name(binary.left);
        let right_type = self.operand_data_type_name(binary.right);
        // Both names must be actual DATA definitions -- `operand_data_type_name` also
        // resolves primitive names (`i32`, `String`), and a primitive-vs-primitive or
        // primitive-vs-text mismatch (`n == s`) is owned by the validation-phase
        // cross-class / text-operator checks with their own diagnostics. This gate
        // fires only on the data-type positional-expansion hole (`P == Q`, `Color ==
        // Direction`).
        if let Some((left_name, right_name)) = Self::different_data_type_names(
            program,
            &left_type,
            &right_type,
            self.source.source_span(binary.left),
            self.source.source_span(binary.right),
        ) {
            return Err(Diagnostic::error(format!(
                "cannot compare `{left_name}` and `{right_name}` with `==`/`!=`: the operands are \
                 different data types (equality requires both operands to be the same type)"
            )));
        }
        let type_name = match left_type.or(right_type) {
            Some(name) => name,
            None => return Ok(None),
        };
        let Some(data) =
            data_definition_by_name_from(program, &type_name, self.source.source_span(binary.left))
        else {
            return Ok(None);
        };
        if data.quotient.is_some() {
            return Err(Diagnostic::error(format!(
                "cannot compare quotient `{type_name}` values with `==`/`!=`: retained representatives are opaque and have no synthesized structural equality; call a named lifted equality operation"
            )));
        }
        if data_equality_shape(program, data) != DataEqualityShape::Structural {
            return Ok(None);
        }

        if self
            .equality_scope
            .is_some_and(|scope| scope.synthesized_row_data == data.symbol)
        {
            return self.expand_structural_equality(data, binary).map(Some);
        }
        let implementation = match equality_selection(program, data) {
            EqualitySelection::Selected(implementation) => implementation,
            EqualitySelection::Missing => {
                return Err(Diagnostic::error(format!(
                    "cannot compare `{type_name}` values with `==`: structural equality is synthesized only through a named implementation -- declare `{}` to synthesize structural equality",
                    suggested_equality_declaration(&type_name)
                )));
            }
            EqualitySelection::Ambiguous(implementations) => {
                return Err(Diagnostic::error(format!(
                    "cannot compare `{type_name}` values with `==`: {} Equatable implementations satisfy `Equatable<{type_name}>` (`{}`) and the operator selects at most one -- call the intended implementation's `equals` explicitly",
                    implementations.len(),
                    implementations.join("`, `")
                )));
            }
        };
        if implementation.synthesized {
            return self.expand_structural_equality(data, binary).map(Some);
        }
        if !implementation.equals_state.is_valid() {
            return Err(Diagnostic::error(format!(
                "cannot compare `{type_name}` values with `==`: Equatable implementation `{}` retains no `equals` row; declare it as a closed block",
                implementation_name(program, implementation.conformance)
            )));
        }

        // The authored row is selected exactly as `Impl::equals(&left,
        // &right)` selects it: the implementation is the call's declaration
        // namespace and the row's realization state is its target.
        let conformance = implementation.conformance;
        let mut members = HandleSpan::empty();
        let receiver_name = conformance.alias.as_ref().map_or_else(
            || typed::name::Identifier::generated(implementation_name(program, conformance)),
            lower_name,
        );
        self.target()
            .push_name_path_member(&mut members, receiver_name);
        let mut member_symbols = HandleSpan::empty();
        self.target()
            .push_name_path_member_symbol(&mut member_symbols, conformance.symbol);
        let receiver = self
            .target()
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath::new(
                    members,
                    member_symbols,
                    conformance.symbol,
                    conformance.symbol,
                ),
            ));
        // Each operand is lowered once in authored order. Shared loans
        // express equality's observation; the ordinary call and borrow
        // checks still validate the selected declaration and live places.
        let mut arguments = Vec::with_capacity(2);
        for operand in [binary.left, binary.right] {
            let target = self.lower(operand)?;
            let argument = self
                .target()
                .insert(typed::expression::ExpressionNode::Borrow(
                    typed::expression::TableBorrowExpression {
                        target,
                        access: language_semantics::ReferenceAccess::Shared,
                    },
                ));
            arguments.push(argument);
        }
        let arguments = self.target().insert_expression_handles(arguments);
        let call = self
            .target()
            .insert(typed::expression::ExpressionNode::Call(
                typed::expression::TableCallExpression {
                    authored_machine_application: Default::default(),
                    requirement_application: Default::default(),
                    exact_coercion_target: typed::types::TypeReferenceHandle::invalid(),
                    receiver,
                    target_symbol: implementation.equals_state,
                    target: typed::name::Identifier::generated_static("equals"),
                    static_machine_parameter: symbols::SymbolHandle::invalid(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    layout_query: None,
                    arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement:
                        language_semantics::CallOperationalAcknowledgement {
                            origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                            ..Default::default()
                        },
                },
            ));
        Ok(Some(self.with_equality_polarity(call, binary.operator)))
    }

    /// The structural expansion of `left == right` (or `!=`) over `data`,
    /// once an implementation's synthesized row selected it.
    fn expand_structural_equality(
        &mut self,
        data: &'program DataDefinition,
        binary: &resolved::expression::TableBinaryExpression,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let type_name = data.name.as_str().to_owned();
        let mut visiting = vec![type_name.clone()];
        let left = self.classify_operand(binary.left, &type_name)?;
        let right = self.classify_operand(binary.right, &type_name)?;
        let equality = self.structural_equality(data, &left, &right, &mut visiting)?;
        Ok(self.with_equality_polarity(equality, binary.operator))
    }

    /// `!=` is the negated expansion. Unary logical-not has no runtime
    /// lowering path yet, so negation is spelled `equality == false`, which
    /// stays inside the supported boolean-compare vocabulary.
    fn with_equality_polarity(
        &mut self,
        equality: typed::expression::ExpressionHandle,
        operator: resolved::expression::BinaryOperator,
    ) -> typed::expression::ExpressionHandle {
        match operator {
            resolved::expression::BinaryOperator::NotEqual => {
                let negated = self
                    .target()
                    .insert(typed::expression::ExpressionNode::Boolean(false));
                self.target()
                    .insert(typed::expression::ExpressionNode::Binary(
                        typed::expression::TableBinaryExpression {
                            left: equality,
                            operator: typed::expression::BinaryOperator::Equal,
                            right: negated,
                        },
                    ))
            }
            _ => equality,
        }
    }

    fn classifier_reference_operand(
        &self,
        operand: resolved::expression::ExpressionHandle,
    ) -> bool {
        if !operand.is_valid() {
            return false;
        }
        match self.source.expression(operand) {
            resolved::expression::ExpressionNode::Name(path) => {
                self.source.name_path_members(path.members).len() >= 2
            }
            resolved::expression::ExpressionNode::Borrow(inner) => {
                self.classifier_reference_operand(inner.target)
            }
            _ => false,
        }
    }

    /// The declared TYPE name of a classifier reference (`Direction::North` ->
    /// `Direction`): the FIRST segment of a multi-member name path. `None`
    /// for a value place, a bare local, or any non-classifier operand.
    fn classifier_reference_type(
        &self,
        operand: resolved::expression::ExpressionHandle,
    ) -> Option<String> {
        if !operand.is_valid() {
            return None;
        }
        match self.source.expression(operand) {
            resolved::expression::ExpressionNode::Name(path) => {
                let members = self.source.name_path_members(path.members);
                (members.len() >= 2).then(|| members[0].as_str().to_owned())
            }
            resolved::expression::ExpressionNode::Borrow(inner) => {
                self.classifier_reference_type(inner.target)
            }
            _ => None,
        }
    }

    /// The base data-type name of an `==` operand, resolved through the
    /// state's typing scope. `None` for operands the scope cannot type;
    /// those compares lower unchanged.
    /// `Some((left, right))` when both operands name real data types that differ —
    /// the positional-expansion hazard each compare gate rejects with its own
    /// message.
    fn different_data_type_names<'a>(
        program: &resolved::SymbolResolvedTrees,
        left_type: &'a Option<String>,
        right_type: &'a Option<String>,
        left_span: source::SourceSpan,
        right_span: source::SourceSpan,
    ) -> Option<(&'a String, &'a String)> {
        let (Some(left_name), Some(right_name)) = (left_type, right_type) else {
            return None;
        };
        (left_name != right_name
            && data_definition_by_name_from(program, left_name, left_span).is_some()
            && data_definition_by_name_from(program, right_name, right_span).is_some())
        .then_some((left_name, right_name))
    }

    fn operand_data_type_name(
        &self,
        operand: resolved::expression::ExpressionHandle,
    ) -> Option<String> {
        if !operand.is_valid() {
            return None;
        }
        let program = self.program?;
        match self.source.expression(operand) {
            resolved::expression::ExpressionNode::StructLiteral(literal) => {
                Some(literal.type_name.as_str().to_owned())
            }
            resolved::expression::ExpressionNode::Borrow(inner) => {
                self.operand_data_type_name(inner.target)
            }
            resolved::expression::ExpressionNode::Name(path) => {
                // Declaration subjects and abstract parameters do not belong
                // to a machine's local inference scope. Their resolved symbol
                // carries the same declared type used by member binding.
                if let Some(reference) =
                    super::member_binding::declared_symbol_type(program, path.symbol)
                    && let Some(name) = value_type_base_name(program, reference)
                {
                    return Some(name);
                }
                let members = self.source.name_path_members(path.members);
                let scope = self.equality_scope?;
                let [name] = members else {
                    return None;
                };
                scope.value_type(name.as_str()).map(str::to_owned)
            }
            resolved::expression::ExpressionNode::Member(member) => {
                let receiver_type = self.operand_data_type_name(member.receiver)?;
                let data = data_definition_by_name_from(
                    program,
                    &receiver_type,
                    self.source.source_span(member.receiver),
                )?;
                let field = program
                    .data_members(data.members)
                    .iter()
                    .find_map(|data_member| match data_member {
                        DataMember::Field(field)
                            if field.name.as_str() == member.member.as_str() =>
                        {
                            Some(field)
                        }
                        _ => None,
                    })?;
                value_type_base_name(program, &field.type_reference)
            }
            _ => None,
        }
    }

    fn classify_operand(
        &mut self,
        operand: resolved::expression::ExpressionHandle,
        type_name: &str,
    ) -> Result<Operand, Diagnostic> {
        match self.source.expression(operand) {
            resolved::expression::ExpressionNode::StructLiteral(literal) => Ok(Operand::Literal {
                case_name: literal
                    .case_name
                    .as_ref()
                    .map(|name| name.as_str().to_owned()),
                fields: literal.fields,
            }),
            resolved::expression::ExpressionNode::Borrow(inner) => {
                self.classify_operand(inner.target, type_name)
            }
            resolved::expression::ExpressionNode::Name(_)
            | resolved::expression::ExpressionNode::Member(_)
            | resolved::expression::ExpressionNode::Indexed(_) => {
                Ok(Operand::Place(self.lower(operand)?))
            }
            _ => Err(Diagnostic::error(format!(
                "cannot synthesize structural `==` for this `{type_name}` operand: bind the value to a local first"
            ))),
        }
    }

    fn structural_equality(
        &mut self,
        data: &'program DataDefinition,
        left: &Operand,
        right: &Operand,
        visiting: &mut Vec<String>,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let program = self
            .program
            .expect("structural equality requires program context");
        let members = program.data_members(data.members);

        let variants: Vec<&DataVariant> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                _ => None,
            })
            .collect();

        let fields: Vec<&DataField> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect();

        if variants.is_empty() {
            if !fields.is_empty() && fields.iter().all(|field| field.relevance.is_erased()) {
                return Err(Diagnostic::error(format!(
                    "cannot synthesize structural `==` for `{}`: every field is `[erased]`, so the record has no runtime value to compare",
                    data.name
                )));
            }
            let compares = self.field_compares(data, &fields, None, left, right, visiting)?;
            return Ok(self.conjunction(compares));
        }

        // MIXED shape: equal when the COMMON fields match AND the case part
        // matches (tag AND matching payload -- the ordinary sum expansion).
        // Common fields compare unconditionally: they exist in every case.
        if !fields.is_empty() {
            let mut compares = self.field_compares(data, &fields, None, left, right, visiting)?;
            compares.push(self.sum_equality(data, &variants, left, right, visiting)?);
            return Ok(self.conjunction(compares));
        }

        self.sum_equality(data, &variants, left, right, visiting)
    }

    /// The per-field compares of a record or one case's payload.
    fn field_compares(
        &mut self,
        data: &'program DataDefinition,
        fields: &[&DataField],
        case_variant: Option<&DataVariant>,
        left: &Operand,
        right: &Operand,
        visiting: &mut Vec<String>,
    ) -> Result<Vec<typed::expression::ExpressionHandle>, Diagnostic> {
        let mut compares = Vec::new();
        for field in fields {
            if field.relevance.is_erased() {
                continue;
            }
            compares.push(self.field_compare(data, field, case_variant, left, right, visiting)?);
        }
        Ok(compares)
    }

    /// Sum equality. Two places: a disjunction over the cases, each arm
    /// `left == Type::Case && right == Type::Case && payload compares`. A
    /// constructed case literal pins the arm statically.
    fn sum_equality(
        &mut self,
        data: &'program DataDefinition,
        variants: &[&DataVariant],
        left: &Operand,
        right: &Operand,
        visiting: &mut Vec<String>,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let program = self
            .program
            .expect("structural equality requires program context");

        match (left, right) {
            (
                Operand::Literal {
                    case_name: Some(left_case),
                    ..
                },
                Operand::Literal {
                    case_name: Some(right_case),
                    ..
                },
            ) => {
                if left_case != right_case {
                    return Ok(self
                        .target()
                        .insert(typed::expression::ExpressionNode::Boolean(false)));
                }
                let variant = self.sum_variant(data, variants, left_case)?;
                let payload: Vec<&DataField> = program
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect();
                let compares =
                    self.field_compares(data, &payload, Some(variant), left, right, visiting)?;
                Ok(self.conjunction(compares))
            }
            (
                Operand::Literal {
                    case_name: None, ..
                },
                Operand::Literal { .. },
            )
            | (
                Operand::Literal { .. },
                Operand::Literal {
                    case_name: None, ..
                },
            ) => Err(Diagnostic::error(format!(
                "cannot compare `{}` values against a record literal: the type is a sum of cases",
                data.name
            ))),
            (Operand::Place(_), Operand::Literal { case_name, fields })
            | (Operand::Literal { case_name, fields }, Operand::Place(_)) => {
                let Some(case_name) = case_name else {
                    return Err(Diagnostic::error(format!(
                        "cannot compare `{}` values against a record literal: the type is a sum of cases",
                        data.name
                    )));
                };
                let place = match (left, right) {
                    (Operand::Place(place), _) | (_, Operand::Place(place)) => *place,
                    _ => unreachable!("one side is a place in this arm"),
                };
                let literal = Operand::Literal {
                    case_name: Some(case_name.clone()),
                    fields: *fields,
                };
                let variant = self.sum_variant(data, variants, case_name)?;
                let tag_compare = self.tag_compare(data, variant, place);
                let payload: Vec<&DataField> = program
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect();
                let place_operand = Operand::Place(place);
                let mut compares = vec![tag_compare];
                compares.extend(self.field_compares(
                    data,
                    &payload,
                    Some(variant),
                    &place_operand,
                    &literal,
                    visiting,
                )?);
                Ok(self.conjunction(compares))
            }
            (Operand::Place(left_place), Operand::Place(right_place)) => {
                let (left_place, right_place) = (*left_place, *right_place);
                let mut arms = Vec::new();
                for variant in variants {
                    // Tag compares come FIRST so short-circuit evaluation
                    // never reads another case's payload fields.
                    let left_tag = self.tag_compare(data, variant, left_place);
                    let right_tag = self.tag_compare(data, variant, right_place);
                    let payload: Vec<&DataField> = program
                        .data_payload_fields(variant.payload)
                        .iter()
                        .collect();
                    let mut compares = vec![left_tag, right_tag];
                    compares.extend(self.field_compares(
                        data,
                        &payload,
                        Some(variant),
                        &Operand::Place(left_place),
                        &Operand::Place(right_place),
                        visiting,
                    )?);
                    arms.push(self.conjunction(compares));
                }
                Ok(self.disjunction(arms))
            }
        }
    }

    fn sum_variant(
        &self,
        data: &'program DataDefinition,
        variants: &[&'program DataVariant],
        case_name: &str,
    ) -> Result<&'program DataVariant, Diagnostic> {
        variants
            .iter()
            .copied()
            .find(|variant| variant.name.as_str() == case_name)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "`{}` has no case `{case_name}` to compare against",
                    data.name
                ))
            })
    }

    fn field_compare(
        &mut self,
        data: &'program DataDefinition,
        field: &DataField,
        case_variant: Option<&DataVariant>,
        left: &Operand,
        right: &Operand,
        visiting: &mut Vec<String>,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let program = self
            .program
            .expect("structural equality requires program context");
        let context = || format!("structural equality of `{}`", visiting[0]);
        match field_equality(program, context, data, field)? {
            // A `String` field compares by CONTENT (length AND bytes) through
            // the backend's value-position text-equals operand, which reads
            // two stored {ptr, len} descriptor PLACES. A literal operand's
            // field value has no stored descriptor at this point, so it stays
            // rejected with a bindable workaround.
            equality @ (FieldEquality::Direct | FieldEquality::Text) => {
                if matches!(equality, FieldEquality::Text)
                    && (matches!(left, Operand::Literal { .. })
                        || matches!(right, Operand::Literal { .. }))
                {
                    return Err(Diagnostic::error(format!(
                        "cannot synthesize structural `==` for `String` field `{}` of `{}` against a constructed literal: text content equality compares stored values -- bind the literal to a value first",
                        field.name, data.name
                    )));
                }
                let left_value = self.operand_field_value(field, case_variant, left)?;
                let right_value = self.operand_field_value(field, case_variant, right)?;
                Ok(self
                    .target()
                    .insert(typed::expression::ExpressionNode::Binary(
                        typed::expression::TableBinaryExpression {
                            left: left_value,
                            operator: typed::expression::BinaryOperator::Equal,
                            right: right_value,
                        },
                    )))
            }
            FieldEquality::Structural(nested) => {
                let nested_name = nested.name.as_str();
                if visiting.iter().any(|name| name == nested_name) {
                    return Err(Diagnostic::error(format!(
                        "{}: type `{nested_name}` is recursive through field `{}` of `{}`: synthesized structural equality would not terminate, so recursive Equatable implementations are not supported yet",
                        context(),
                        field.name,
                        data.name
                    )));
                }
                let left_nested =
                    self.operand_field_operand(field, case_variant, left, nested_name)?;
                let right_nested =
                    self.operand_field_operand(field, case_variant, right, nested_name)?;
                visiting.push(nested_name.to_owned());
                let equality =
                    self.structural_equality(nested, &left_nested, &right_nested, visiting)?;
                visiting.pop();
                Ok(equality)
            }
        }
    }

    /// A field of an operand as a typed VALUE: a synthesized member read on
    /// a place, or the literal's field value (falling back to the declared
    /// field default when the literal omits it).
    fn operand_field_value(
        &mut self,
        field: &DataField,
        case_variant: Option<&DataVariant>,
        operand: &Operand,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        match operand {
            Operand::Place(place) => Ok(self.member_read(*place, field, case_variant)),
            Operand::Literal { .. } => {
                let value = self.literal_field_expression(field, operand)?;
                self.lower(value)
            }
        }
    }

    /// A field of an operand as a nested OPERAND for recursive expansion: a
    /// member read stays a place; a literal field value is re-classified
    /// (it may itself be a nested literal or a place name).
    fn operand_field_operand(
        &mut self,
        field: &DataField,
        case_variant: Option<&DataVariant>,
        operand: &Operand,
        field_type_name: &str,
    ) -> Result<Operand, Diagnostic> {
        match operand {
            Operand::Place(place) => Ok(Operand::Place(self.member_read(
                *place,
                field,
                case_variant,
            ))),
            Operand::Literal { .. } => {
                let value = self.literal_field_expression(field, operand)?;
                self.classify_operand(value, field_type_name)
            }
        }
    }

    fn literal_field_expression(
        &self,
        field: &DataField,
        operand: &Operand,
    ) -> Result<resolved::expression::ExpressionHandle, Diagnostic> {
        let Operand::Literal { fields, .. } = operand else {
            unreachable!("literal field access on a place operand")
        };
        if let Some(literal_field) = self
            .source
            .struct_fields(*fields)
            .iter()
            .find(|literal_field| literal_field.name.as_str() == field.name.as_str())
        {
            return Ok(literal_field.value);
        }
        Err(Diagnostic::error(format!(
            "cannot synthesize structural `==`: the literal omits field `{}`",
            field.name
        )))
    }

    fn member_read(
        &mut self,
        place: typed::expression::ExpressionHandle,
        field: &DataField,
        case_variant: Option<&DataVariant>,
    ) -> typed::expression::ExpressionHandle {
        self.target()
            .insert(typed::expression::ExpressionNode::Member(
                typed::expression::TableMemberExpression {
                    receiver: place,
                    member_symbol: field.symbol,
                    member: lower_name(&field.name),
                    case_variant: case_variant.map(|variant| lower_name(&variant.name)),
                },
            ))
    }

    /// Retain tag observation as an operation, not an authored equality use.
    /// Generated tests have no authored membership selection to recover after
    /// lowering. A plain equality node would instead demand a payload value
    /// constructor or select a value operator. Copies retain this distinction
    /// in the operator itself; validation still checks both nominal identities.
    fn tag_compare(
        &mut self,
        data: &DataDefinition,
        variant: &DataVariant,
        place: typed::expression::ExpressionHandle,
    ) -> typed::expression::ExpressionHandle {
        let mut members = HandleSpan::empty();
        self.target()
            .push_name_path_member(&mut members, lower_name(&data.name));
        self.target()
            .push_name_path_member(&mut members, lower_name(&variant.name));
        let mut member_symbols = HandleSpan::empty();
        self.target()
            .push_name_path_member_symbol(&mut member_symbols, data.symbol);
        self.target()
            .push_name_path_member_symbol(&mut member_symbols, variant.symbol);
        let case_reference = self
            .target()
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath::new(
                    members,
                    member_symbols,
                    data.symbol,
                    variant.symbol,
                ),
            ));
        self.target()
            .insert(typed::expression::ExpressionNode::Binary(
                typed::expression::TableBinaryExpression {
                    left: place,
                    operator: typed::expression::BinaryOperator::CaseMembership,
                    right: case_reference,
                },
            ))
    }

    /// Left-fold an AND chain; empty input is the vacuous `true`.
    fn conjunction(
        &mut self,
        compares: Vec<typed::expression::ExpressionHandle>,
    ) -> typed::expression::ExpressionHandle {
        self.fold_binary(compares, typed::expression::BinaryOperator::And, true)
    }

    /// Left-fold an OR chain; empty input is `false` (no case can match).
    fn disjunction(
        &mut self,
        arms: Vec<typed::expression::ExpressionHandle>,
    ) -> typed::expression::ExpressionHandle {
        self.fold_binary(arms, typed::expression::BinaryOperator::Or, false)
    }

    fn fold_binary(
        &mut self,
        parts: Vec<typed::expression::ExpressionHandle>,
        operator: typed::expression::BinaryOperator,
        empty_value: bool,
    ) -> typed::expression::ExpressionHandle {
        let mut parts = parts.into_iter();
        let Some(mut combined) = parts.next() else {
            return self
                .target()
                .insert(typed::expression::ExpressionNode::Boolean(empty_value));
        };
        for part in parts {
            combined = self
                .target()
                .insert(typed::expression::ExpressionNode::Binary(
                    typed::expression::TableBinaryExpression {
                        left: combined,
                        operator,
                        right: part,
                    },
                ));
        }
        combined
    }
}

//! Descriptor observations use the same captured roster as scalar values.
//! A range's endpoints are captured when the descriptor is formed; later
//! endpoint stores cannot enlarge that descriptor. Construction supplies no
//! bounds authority: selected builtin meaning and the current hypotheses must
//! independently establish the range. Returning a descriptor binds its actual
//! observation to the exact contract owner, never to the backing capacity.

use super::{
    CurrentScalarValues, Length, LengthSubject, admitted, fixed_length, integer_carrier, scoped,
};
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::types::TypeReferenceHandle;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

pub(super) struct CapturedFieldLength {
    pub(super) path: Vec<SymbolHandle>,
    pub(super) name: String,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) value: ScopedArithmeticValue,
}

impl CurrentScalarValues<'_> {
    pub(crate) fn postcondition(
        &mut self,
        expression: ExpressionHandle,
        returned: ExpressionHandle,
        origins: impl Iterator<Item = (SymbolHandle, SymbolHandle)>,
    ) -> Option<ScopedArithmeticExpression> {
        if let Some(value) = self.collection_length(returned, 0) {
            let entry = self.program.machine_states(self.machine).first()?;
            self.lengths.push(Length {
                subject: LengthSubject::Result(self.machine.symbol),
                name: String::new(),
                stable: true,
                type_reference: entry.return_type,
                value: Some(value),
            });
        }
        self.record_returned_field_lengths(returned);
        // The declaration's postcondition names invocation parameters, while
        // the live hypotheses name the exit state's parameters. Bind those
        // namespaces through the independently established incoming origins;
        // spelling, slot order and a write-free body are not identity evidence.
        let entry = self.program.machine_states(self.machine).first()?;
        let mut bindings = self.current.clone();
        let mut lengths = self.lengths.clone();
        if entry.symbol != self.state.symbol {
            bindings.clear();
            lengths.retain(|length| {
                matches!(
                    length.subject,
                    LengthSubject::Result(_) | LengthSubject::ResultField { .. }
                )
            });
            for (entry_parameter, state_parameter) in origins {
                if let Some(current) = self
                    .current
                    .iter()
                    .find(|binding| binding.symbol == state_parameter)
                {
                    let mut binding = current.clone();
                    binding.symbol = entry_parameter;
                    binding.name = self.program.symbols.name(entry_parameter).to_owned();
                    bindings.push(binding);
                }
                if let Some(current) = self
                    .lengths
                    .iter()
                    .find(|length| length.subject == LengthSubject::Storage(state_parameter))
                {
                    let mut length = current.clone();
                    length.subject = LengthSubject::Storage(entry_parameter);
                    length.name = self.program.symbols.name(entry_parameter).to_owned();
                    lengths.push(length);
                }
            }
        }
        if !admitted(
            self.program,
            self.machine,
            entry,
            expression,
            &bindings,
            &lengths,
            0,
        ) {
            return None;
        }
        let mut goal = scoped(
            self.machine,
            entry,
            expression,
            &bindings,
            &lengths,
            self.program,
        )?;
        if let Some(returned) = self.current(returned) {
            goal.bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Result,
                value: ScopedArithmeticValue::Term(returned),
            });
        }
        Some(goal)
    }

    pub(super) fn capture_collection(
        &mut self,
        symbol: SymbolHandle,
        name: &str,
        type_reference: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement: usize,
    ) {
        let value = self.collection_length(expression, 0);
        self.lengths
            .retain(|length| length.subject != LengthSubject::Storage(symbol));
        // Only descriptors of reference collection types participate. Bounded
        // owners have live counts, not a frozen length equal to their capacity.
        if fixed_length(self.program, type_reference, String::new).is_some() {
            self.lengths.push(Length {
                subject: LengthSubject::Storage(symbol),
                name: name.to_owned(),
                stable: false,
                type_reference,
                // Completing a descriptor always produces its own length
                // observation, even when no endpoint equation is available.
                // A later guard may constrain that observation without gaining
                // any unproved relationship to its backing or source values.
                value: Some(value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
                    identity: format!(
                        "state:{:?}:store:{statement}:descriptor:{symbol:?}:occurrence:{expression:?}:length",
                        self.state.symbol
                    ),
                    unsigned: true,
                })),
            });
        }
    }

    pub(crate) fn byte_length_bound(&self, expression: ExpressionHandle) -> Option<u64> {
        let value = self.collection_length(expression, 0)?;
        let interval = validation::scoped_arithmetic_value_bounds(
            self.program,
            self.machine,
            &self.hypotheses,
            &value,
        )?;
        interval.high?.to_u64()
    }

    pub(super) fn collection_length(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.collection_length(borrow.target, depth + 1),
            ExpressionNode::Name(name) if name.symbol == name.head_symbol => self
                .lengths
                .iter()
                .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                .and_then(|length| length.value.clone())
                .or_else(|| self.declared_extent(expression)),
            ExpressionNode::Indexed(indexed)
                if validation::has_builtin_subslice_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                ) =>
            {
                let range = self.program.expression_table.range_node(indexed.index)?;
                let source = self.collection_length(indexed.collection, depth + 1)?;
                let zero = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero());
                let start = if range.start.is_valid() {
                    self.capture(self.program, self.machine, self.state, range.start)?
                } else {
                    zero.clone()
                };
                let authored_end = if range.end.is_valid() {
                    self.capture(self.program, self.machine, self.state, range.end)?
                } else {
                    source.clone()
                };
                let end = if range.end_inclusive {
                    if !range.end.is_valid() {
                        return None;
                    }
                    ScopedArithmeticValue::IntegerOperation {
                        operator: BinaryOperator::Add,
                        left: Box::new(authored_end.clone()),
                        right: Box::new(ScopedArithmeticValue::Integer(
                            numerics::bignum::BigInt::from_u64(1),
                        )),
                    }
                } else {
                    authored_end.clone()
                };
                for (left, right) in [
                    (&zero, &start),
                    (&start, &end),
                    (&end, &source),
                    (&zero, &authored_end),
                ] {
                    if !self.proves_values_relation(
                        self.program,
                        self.machine,
                        BinaryOperator::LessOrEqual,
                        left,
                        right,
                    ) {
                        return None;
                    }
                }
                Some(ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Subtract,
                    left: Box::new(end),
                    right: Box::new(start),
                })
            }
            ExpressionNode::Match(dispatch) => self.matched_length(dispatch, depth),
            ExpressionNode::Member(member) => self
                .saved_field_length(member, depth)
                .or_else(|| {
                    super::field_collection_length(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    )
                })
                .or_else(|| self.declared_extent(expression)),
            ExpressionNode::Call(call) => {
                let parameters =
                    validation::call_target_parameters(self.program, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let mut bindings = Vec::new();
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let value = if integer_carrier(self.program, parameter.type_reference).is_some()
                    {
                        self.capture(self.program, self.machine, self.state, *argument)
                    } else if !parameter.is_mutable
                        && fixed_length(self.program, parameter.type_reference, String::new)
                            .is_some()
                    {
                        self.collection_length(*argument, depth + 1)
                    } else {
                        None
                    };
                    if let Some(value) = value {
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(*argument),
                            value,
                        });
                    }
                }
                Some(ScopedArithmeticValue::CollectionLength(
                    ScopedArithmeticExpression {
                        owner: self.state.symbol,
                        expression,
                        bindings,
                    },
                ))
            }
            _ => super::field_collection_length(self.program, self.machine, self.state, expression)
                .or_else(|| self.declared_extent(expression)),
        }
    }

    /// Every arm contributes its own length. One shared observation is the
    /// result. Identical observations agree immediately. Arms that are not
    /// identical still agree when each folds to the same integer, so
    /// `row[0..2]` and `row[2..4]` both publish length 2. A value arm may use
    /// the subject: `match which { 0 -> row[0..which + 2] }` is length 2
    /// because that arm's subject is 0. The failed-pattern path does not
    /// receive that value. Arms whose lengths are proved equal publish the
    /// first arm's length, so `bytes[0..count]` and `bytes[16..16 + count]`
    /// both publish that length when `count` is one value. An arm without a
    /// length, or two arms with different lengths, publishes nothing.
    fn matched_length(
        &self,
        dispatch: &TableMatchExpression,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        let arms = self.program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return None;
        }
        let subject = self.capture(self.program, self.machine, self.state, dispatch.subject);
        let mut lengths = Vec::new();
        for arm in arms {
            let replacement = match arm.pattern {
                MatchPattern::Value(pattern) => self.pattern_integer(pattern),
                MatchPattern::Wildcard => None,
            };
            lengths.push(self.arm_length(
                arm.value,
                depth + 1,
                subject.as_ref(),
                replacement.as_ref(),
            )?);
        }
        if lengths.iter().skip(1).all(|length| length == &lengths[0]) {
            return Some(lengths[0].clone());
        }
        let folded: Option<Vec<_>> = lengths
            .iter()
            .map(|length| self.exact_length(length))
            .collect();
        if let Some(folded) = folded {
            let first = &folded[0];
            return folded
                .iter()
                .all(|length| length == first)
                .then(|| ScopedArithmeticValue::Integer(first.clone()));
        }
        lengths
            .iter()
            .skip(1)
            .all(|length| {
                self.proves_values_relation(
                    self.program,
                    self.machine,
                    BinaryOperator::Equal,
                    length,
                    &lengths[0],
                )
            })
            .then(|| lengths[0].clone())
    }

    fn pattern_integer(&self, pattern: ExpressionHandle) -> Option<ScopedArithmeticValue> {
        if let ExpressionNode::Integer(literal) = self.program.expression_table.expression(pattern)
        {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i64(literal.value_i64()?),
            ));
        }
        let captured = self.capture(self.program, self.machine, self.state, pattern)?;
        Some(ScopedArithmeticValue::Integer(
            self.exact_length(&captured)?,
        ))
    }

    fn exact_length(&self, value: &ScopedArithmeticValue) -> Option<numerics::bignum::BigInt> {
        let interval = validation::scoped_arithmetic_value_bounds(
            self.program,
            self.machine,
            &self.hypotheses,
            value,
        )?;
        let low = interval.low?;
        let high = interval.high?;
        (low == high).then_some(low)
    }

    /// Slice fields of the returned record. A match publishes a field only
    /// when every arm builds that same field with one shared length.
    fn record_returned_field_lengths(&mut self, returned: ExpressionHandle) {
        for field in self.captured_field_lengths(returned, 0) {
            self.lengths.push(Length {
                subject: LengthSubject::ResultField {
                    owner: self.machine.symbol,
                    path: field.path,
                },
                name: format!("result.{}", field.name),
                stable: true,
                type_reference: field.type_reference,
                value: Some(field.value),
            });
        }
    }

    pub(super) fn captured_field_lengths(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Vec<CapturedFieldLength> {
        if depth >= 64 {
            return Vec::new();
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none() => {
                self.literal_field_lengths(literal, depth + 1)
            }
            ExpressionNode::Borrow(borrow) => self.captured_field_lengths(borrow.target, depth + 1),
            ExpressionNode::Call(call) => self.call_field_lengths(expression, call),
            ExpressionNode::Member(member)
                if super::field_storage_path(self.program, expression).is_none() =>
            {
                self.captured_field_lengths(member.receiver, depth + 1)
                    .into_iter()
                    .filter_map(|mut field| {
                        let selected = *field.path.first()?;
                        if field.path.len() <= 1 || !self.selects_field(member, selected) {
                            return None;
                        }
                        field.path.remove(0);
                        field.name = field
                            .path
                            .iter()
                            .map(|symbol| self.program.symbols.name(*symbol))
                            .collect::<Vec<_>>()
                            .join(".");
                        Some(field)
                    })
                    .collect()
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                let Some((owner, prefix)) = super::field_storage_path(self.program, expression)
                else {
                    return Vec::new();
                };
                self.lengths
                    .iter()
                    .filter_map(|length| {
                        let LengthSubject::Field {
                            owner: existing,
                            path,
                        } = &length.subject
                        else {
                            return None;
                        };
                        if *existing != owner
                            || !path.starts_with(&prefix)
                            || path.len() <= prefix.len()
                        {
                            return None;
                        }
                        let path = path[prefix.len()..].to_vec();
                        let name = path
                            .iter()
                            .map(|symbol| self.program.symbols.name(*symbol))
                            .collect::<Vec<_>>()
                            .join(".");
                        Some(CapturedFieldLength {
                            path,
                            name,
                            type_reference: length.type_reference,
                            value: length.value.clone()?,
                        })
                    })
                    .collect()
            }
            ExpressionNode::Match(dispatch) => {
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                let mut joined: Option<Vec<CapturedFieldLength>> = None;
                for arm in arms {
                    let fields = self.captured_field_lengths(arm.value, depth + 1);
                    joined = Some(match joined {
                        None => fields,
                        Some(existing) => existing
                            .into_iter()
                            .filter(|field| {
                                fields.iter().any(|candidate| {
                                    candidate.path == field.path
                                        && self.slice_lengths_agree(&candidate.value, &field.value)
                                })
                            })
                            .map(|mut field| {
                                if let Some(length) = self.exact_length(&field.value) {
                                    field.value = ScopedArithmeticValue::Integer(length);
                                }
                                field
                            })
                            .collect(),
                    });
                }
                joined.unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    fn literal_field_lengths(
        &self,
        literal: &typed_trees::expression::TableStructLiteral,
        depth: usize,
    ) -> Vec<CapturedFieldLength> {
        let mut fields = Vec::new();
        for field in self.program.expression_table.struct_fields(literal.fields) {
            if !field.field_symbol.is_valid() {
                continue;
            }
            for mut nested in self.captured_field_lengths(field.value, depth + 1) {
                nested.path.insert(0, field.field_symbol);
                nested.name = format!("{}.{}", field.name, nested.name);
                fields.push(nested);
            }
            let Some(value) = self.collection_length(field.value, depth + 1) else {
                continue;
            };
            let Some(type_reference) = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                field.value,
            ) else {
                continue;
            };
            if fixed_length(self.program, type_reference, String::new).is_none() {
                continue;
            }
            let value = self
                .exact_length(&value)
                .map(ScopedArithmeticValue::Integer)
                .unwrap_or(value);
            fields.push(CapturedFieldLength {
                path: vec![field.field_symbol],
                name: field.name.as_str().to_owned(),
                type_reference,
                value,
            });
        }
        fields
    }

    fn slice_lengths_agree(
        &self,
        left: &ScopedArithmeticValue,
        right: &ScopedArithmeticValue,
    ) -> bool {
        left == right
            || matches!(
                (self.exact_length(left), self.exact_length(right)),
                (Some(left), Some(right)) if left == right
            )
            || self.proves_values_relation(
                self.program,
                self.machine,
                BinaryOperator::Equal,
                left,
                right,
            )
    }

    /// Length of one match arm. A window whose endpoint mentions the subject
    /// is measured after that arm's pattern replaces the subject, then checked
    /// for a non-negative ordered span. Other arms keep `collection_length`.
    fn arm_length(
        &self,
        expression: ExpressionHandle,
        depth: usize,
        subject: Option<&ScopedArithmeticValue>,
        replacement: Option<&ScopedArithmeticValue>,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64 || !expression.is_valid() {
            return None;
        }
        if let ExpressionNode::Borrow(borrow) = self.program.expression_table.expression(expression)
        {
            return self.arm_length(borrow.target, depth + 1, subject, replacement);
        }
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression)
        {
            return self.matched_length(dispatch, depth);
        }
        let Some(indexed) = self.program.expression_table.indexed_node(expression) else {
            return self.collection_length(expression, depth);
        };
        if !validation::has_builtin_subslice_meaning(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        ) {
            return self.collection_length(expression, depth);
        }
        let range = self.program.expression_table.range_node(indexed.index)?;
        let source = self.collection_length(indexed.collection, depth + 1)?;
        let zero = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero());
        let start = self.replaced_endpoint(range.start, &zero, subject, replacement)?;
        let authored_end = if range.end.is_valid() {
            self.replaced_endpoint(range.end, &source, subject, replacement)?
        } else {
            source.clone()
        };
        let end = if range.end_inclusive {
            if !range.end.is_valid() {
                return None;
            }
            ScopedArithmeticValue::IntegerOperation {
                operator: BinaryOperator::Add,
                left: Box::new(authored_end.clone()),
                right: Box::new(ScopedArithmeticValue::Integer(
                    numerics::bignum::BigInt::from_u64(1),
                )),
            }
        } else {
            authored_end.clone()
        };
        for (left, right) in [
            (&zero, &start),
            (&start, &end),
            (&end, &source),
            (&zero, &authored_end),
        ] {
            if !self.proves_values_relation(
                self.program,
                self.machine,
                BinaryOperator::LessOrEqual,
                left,
                right,
            ) {
                return None;
            }
        }
        Some(ScopedArithmeticValue::IntegerOperation {
            operator: BinaryOperator::Subtract,
            left: Box::new(end),
            right: Box::new(start),
        })
    }

    fn replaced_endpoint(
        &self,
        endpoint: ExpressionHandle,
        omitted: &ScopedArithmeticValue,
        subject: Option<&ScopedArithmeticValue>,
        replacement: Option<&ScopedArithmeticValue>,
    ) -> Option<ScopedArithmeticValue> {
        let captured = if endpoint.is_valid() {
            self.capture(self.program, self.machine, self.state, endpoint)?
        } else {
            omitted.clone()
        };
        // A call in the endpoint may store the field before a later read.
        // The pattern still describes the match subject, not that later value.
        let call_may_store_field = matches!(subject, Some(ScopedArithmeticValue::Atom { .. }))
            && endpoint.is_valid()
            && expression_contains_call(self.program, endpoint);
        Some(match (subject, replacement) {
            (Some(subject), Some(replacement)) if !call_may_store_field => {
                substitute_matched_subject(&captured, subject, replacement)
            }
            _ => captured,
        })
    }

    /// Read result guarantees once at the exact completed call occurrence.
    fn call_field_lengths(
        &self,
        occurrence: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Vec<CapturedFieldLength> {
        let Some((machine, _)) = validation::find_state(self.program, call.target_symbol)
            .or_else(|| validation::find_machine_head(self.program, call.target_symbol))
        else {
            return Vec::new();
        };
        let mut fields = Vec::new();
        for contract in self.program.machine_contracts(machine) {
            if contract.kind != typed_trees::signature::SignatureContractKind::Ensures {
                continue;
            }
            for fact in self.program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                let Some(comparison) = self.program.expression_table.binary_node(*expression)
                else {
                    continue;
                };
                let candidates = match comparison.operator {
                    BinaryOperator::Equal => vec![
                        (comparison.left, comparison.right, false),
                        (comparison.right, comparison.left, false),
                    ],
                    BinaryOperator::GreaterOrEqual => {
                        vec![(comparison.left, comparison.right, true)]
                    }
                    BinaryOperator::LessOrEqual => vec![(comparison.right, comparison.left, true)],
                    _ => continue,
                };
                for (measure, bound, lower_bound) in candidates {
                    let Some(length) = self.program.expression_table.member_node(measure) else {
                        continue;
                    };
                    let Some(place) =
                        validation::reserved_result_place(self.program, length.receiver)
                    else {
                        continue;
                    };
                    let Some(path) = super::declaration_field_path(&place.segments) else {
                        continue;
                    };
                    let reference = place.type_reference;
                    if place.machine_symbol != machine.symbol
                        || path.is_empty()
                        || validation::collection_operations::collection_measure_member(
                            self.program,
                            length,
                            validation::collection_operations::MeasureReceiver::Declared(reference),
                        ) != Some(
                            language_semantics::declaration_selection::CollectionMeasure::Length,
                        )
                    {
                        continue;
                    }
                    let Some(value) = self
                        .program
                        .expression_table
                        .integer_node(bound)
                        .and_then(|literal| literal.value_i64())
                    else {
                        continue;
                    };
                    if value < 0 {
                        continue;
                    }
                    let value = numerics::bignum::BigInt::from_i64(value);
                    let value = if lower_bound {
                        ScopedArithmeticValue::BoundedInteger {
                            value: Box::new(ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "state:{:?}:call:{occurrence:?}:field:{path:?}:length",
                                    self.state.symbol
                                ),
                                unsigned: true,
                            }),
                            minimum: Some(value),
                            maximum: None,
                        }
                    } else {
                        ScopedArithmeticValue::Integer(value)
                    };
                    let name = path
                        .iter()
                        .map(|symbol| self.program.symbols.name(*symbol))
                        .collect::<Vec<_>>()
                        .join(".");
                    fields.push(CapturedFieldLength {
                        path,
                        name,
                        type_reference: reference,
                        value,
                    });
                }
            }
        }
        fields
    }

    /// Read only the current captured field roster, never declaration initializers.
    fn saved_field_length(
        &self,
        member: &typed_trees::expression::TableMemberExpression,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        self.captured_field_lengths(member.receiver, depth + 1)
            .into_iter()
            .find(|field| field.path.len() == 1 && self.selects_field(member, field.path[0]))
            .map(|field| field.value)
    }

    fn selects_field(
        &self,
        member: &typed_trees::expression::TableMemberExpression,
        field: SymbolHandle,
    ) -> bool {
        if member.member_symbol.is_valid() {
            member.member_symbol == field
        } else {
            self.program.symbols.name(field) == member.member.as_str()
        }
    }

    pub(super) fn capture_record_fields(
        &mut self,
        owner: SymbolHandle,
        path: &[SymbolHandle],
        owner_name: &str,
        initializer: ExpressionHandle,
    ) {
        let fields = self.captured_field_lengths(initializer, 0);
        self.replace_record_fields(owner, path, owner_name, fields);
    }

    pub(super) fn replace_record_fields(
        &mut self,
        owner: SymbolHandle,
        prefix: &[SymbolHandle],
        owner_name: &str,
        fields: Vec<CapturedFieldLength>,
    ) {
        self.lengths.retain(|length| !matches!(&length.subject,
            LengthSubject::Field { owner: existing, path } if *existing == owner && path.starts_with(prefix)));
        for field in fields {
            let mut path = prefix.to_vec();
            path.extend(field.path);
            self.lengths.push(Length {
                subject: LengthSubject::Field { owner, path },
                name: if field.name.is_empty() {
                    owner_name.to_owned()
                } else {
                    format!("{owner_name}.{}", field.name)
                },
                stable: false,
                type_reference: field.type_reference,
                value: Some(field.value),
            });
        }
    }

    fn declared_extent(&self, expression: ExpressionHandle) -> Option<ScopedArithmeticValue> {
        // Exact fixed extent belongs to the declared place. Qualification and
        // reference access do not supply a separate live-length word.
        let extent = validation::declared_collection_extent(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        )?;
        Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(extent).ok()?),
        ))
    }
}

/// Replace the matched subject with the arm's pattern value. Only the
/// selected arm's copy is rewritten; the subject's other occurrences stay.
fn substitute_matched_subject(
    value: &ScopedArithmeticValue,
    subject: &ScopedArithmeticValue,
    replacement: &ScopedArithmeticValue,
) -> ScopedArithmeticValue {
    if value == subject || same_symbol_term(value, subject) {
        return replacement.clone();
    }
    match value {
        ScopedArithmeticValue::IntegerOperation {
            operator,
            left,
            right,
        } => ScopedArithmeticValue::IntegerOperation {
            operator: *operator,
            left: Box::new(substitute_matched_subject(left, subject, replacement)),
            right: Box::new(substitute_matched_subject(right, subject, replacement)),
        },
        ScopedArithmeticValue::BoundedInteger {
            value,
            minimum,
            maximum,
        } => ScopedArithmeticValue::BoundedInteger {
            value: Box::new(substitute_matched_subject(value, subject, replacement)),
            minimum: minimum.clone(),
            maximum: maximum.clone(),
        },
        other => other.clone(),
    }
}

/// Two name occurrences of one parameter are different expression handles.
/// They are the same subject when each term binds that parameter and nothing else.
fn same_symbol_term(value: &ScopedArithmeticValue, subject: &ScopedArithmeticValue) -> bool {
    let (ScopedArithmeticValue::Term(value), ScopedArithmeticValue::Term(subject)) =
        (value, subject)
    else {
        return false;
    };
    let value_symbols = symbol_binders(value);
    let subject_symbols = symbol_binders(subject);
    !subject_symbols.is_empty() && value_symbols == subject_symbols
}

fn expression_contains_call(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(program, binary.left)
                || expression_contains_call(program, binary.right)
        }
        ExpressionNode::Unary(unary) => expression_contains_call(program, unary.operand),
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Borrow(borrow) => expression_contains_call(program, borrow.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(program, indexed.collection)
                || expression_contains_call(program, indexed.index)
        }
        ExpressionNode::Range(range) => {
            expression_contains_call(program, range.start)
                || expression_contains_call(program, range.end)
        }
        _ => false,
    }
}

fn symbol_binders(expression: &ScopedArithmeticExpression) -> Vec<symbols::SymbolHandle> {
    expression
        .bindings
        .iter()
        .filter_map(|binding| match binding.binder {
            ScopedArithmeticBinder::Symbol(symbol) => Some(symbol),
            _ => None,
        })
        .collect()
}

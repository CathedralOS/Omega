//! Preserve an operator choice made by the actual body or contract reader.

use language_core::OperatorSpelling;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as Binding,
    AuthoredDeclarationSelectionTarget as Target,
};
use semantic_vocabulary::{MachineId, ScalarType};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::TypeReferenceHandle;

use super::Program;
use crate::body::{Expression, ExpressionKind, Policy, Qualification, ValueType};
use crate::{Unserved, refuse};

impl Program<'_> {
    /// Complete only the static proposition this reader actually evaluated.
    /// Folding its result must retain the operator questions of its operands,
    /// including a guard whose known-false branch is never read as a value.
    pub(super) fn read_static_truth(
        &mut self,
        machine: MachineId,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        reading: super::types::Reading,
    ) -> Checked<Option<super::types::TypeTruth>> {
        let (truth, operators) = self.type_truth_operators(expression, reading);
        if !matches!(truth, Some(super::types::TypeTruth::Known(_))) {
            return Ok(truth);
        }
        let boolean = self.operator_operand_reference(ValueType::plain(ScalarType::Boolean));
        for operator in operators {
            self.complete_operator(
                machine,
                owner,
                operator.expression(),
                &[boolean; 2],
                operator.is_type_equation(),
            )?;
        }
        Ok(truth)
    }

    pub(super) fn complete_anonymous_operators(
        &mut self,
        machine: MachineId,
        owner: SymbolHandle,
        operators: &[ExpressionHandle],
    ) -> Checked<()> {
        for &expression in operators {
            self.complete_builtin_operator(machine, owner, expression, &[None, None])?;
        }
        Ok(())
    }

    /// Reuse the retained source type of a successfully read operand. No type
    /// or declaration is manufactured when its source reference is absent.
    pub(super) fn operator_operand_reference(
        &self,
        operand: ValueType,
    ) -> Option<TypeReferenceHandle> {
        if let ValueType::Record(record, Qualification::NONE, _) = operand {
            return self
                .record_sources
                .get(record)
                .and_then(|source| source.application.as_ref())
                .map(|application| application.reference)
                .filter(|reference| {
                    self.typed
                        .type_reference_table
                        .contains_type_reference(*reference)
                });
        }
        let ValueType::Scalar(scalar, qualification) = operand else {
            return None;
        };
        if !qualification.domains.is_empty() {
            return None;
        }
        let atom = match scalar {
            ScalarType::Boolean => BuiltinTypeAtom::Bool,
            ScalarType::Integer(integer) if integer.is_address() => BuiltinTypeAtom::Address,
            ScalarType::Integer(integer) => {
                BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::new(
                    integer.sign(),
                    integer.bits()?.into(),
                )?)
            }
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
                BuiltinTypeAtom::F32
            }
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
                BuiltinTypeAtom::F64
            }
            ScalarType::Callable(_) => return None,
        };
        let symbol = self.typed.symbols.builtin_atom_symbol(atom)?;
        self.typed
            .type_reference_table
            .find_arithmetic_result_type_reference(
                symbol,
                match qualification.policy {
                    None => numerics::arithmetic::ArithmeticDomain::Exact,
                    Some(Policy::Wrapping) => numerics::arithmetic::ArithmeticDomain::Wrapping,
                    Some(Policy::Saturating) => numerics::arithmetic::ArithmeticDomain::Saturating,
                    Some(Policy::Trapping) => numerics::arithmetic::ArithmeticDomain::Trapping,
                },
            )
    }

    /// Publish the operation this reader actually formed. Reads, writable
    /// places and explicit view adapters share the same source association.
    pub(super) fn complete_formed_operator(
        &mut self,
        machine: MachineId,
        expression: ExpressionHandle,
        value: &Expression,
    ) -> Checked<()> {
        // Inequality is formed as the negation of its checked equality.
        // The source question still belongs to that binary operation, whose
        // actual operand types are on the equality rather than the outer Not.
        let value = match &value.kind {
            ExpressionKind::Not(equal)
                if matches!(self.typed.expression_table.expression(expression),
                    ExpressionNode::Binary(binary)
                        if binary.operator == typed_trees::expression::BinaryOperator::NotEqual) =>
            {
                equal
            }
            _ => value,
        };
        let owner = self.symbols.get(machine).copied().unwrap_or_default();
        match &value.kind {
            ExpressionKind::CollectionSubslice { base, .. }
            | ExpressionKind::CollectionView { base, .. }
                if matches!(
                    self.typed.expression_table.expression(expression),
                    ExpressionNode::Indexed(_)
                ) =>
            {
                let Some(ValueType::Record(record, qualification, _)) = base.result else {
                    return Ok(());
                };
                if qualification != Qualification::NONE {
                    // byte_subslice forms a qualified range only after typing
                    // retained its exact builtin selection and this reader
                    // validated the selected plain shared-view result. Keep
                    // that selection; the storage row erases operand domains.
                    let selected = self
                        .typed
                        .expression_table
                        .constructed_value_type_reference(expression);
                    let range = matches!(
                        self.typed.expression_table.expression(expression),
                        ExpressionNode::Indexed(indexed)
                            if matches!(
                                self.typed.expression_table.expression(indexed.index),
                                ExpressionNode::Range(_)
                            )
                    );
                    if range
                        && matches!(value.kind, ExpressionKind::CollectionSubslice { .. })
                        && self.is_readable_byte_view(machine, record)?
                        && self
                            .typed
                            .type_reference_table
                            .contains_type_reference(selected)
                    {
                        return self.complete_operator(machine, owner, expression, &[], true);
                    }
                    return Ok(());
                }
                let [collection, count] = self.collection_index_operand_types(machine, record);
                let operands = [collection, count, count];
                self.complete_builtin_operator(machine, owner, expression, &operands)
            }
            _ => {
                // A case test has already resolved its case within the subject's
                // exact sum, so both operands have that retained carrier.
                let operands = match &value.kind {
                    ExpressionKind::Strict { left, right, .. }
                    | ExpressionKind::ShortCircuit { left, right, .. } => {
                        [left.result, right.result]
                    }
                    ExpressionKind::Index { base, index }
                    | ExpressionKind::ByteElement { base, index } => [base.result, index.result],
                    ExpressionKind::IsCase { subject, .. } => [subject.result; 2],
                    _ => [None, None],
                }
                .map(|operand| {
                    operand.and_then(|operand| self.operator_operand_reference(operand))
                });
                self.complete_builtin_operator(machine, owner, expression, &operands)
            }
        }
    }

    pub(super) fn mathematical_operator_operands(&self) -> [Option<TypeReferenceHandle>; 2] {
        let reference = self
            .typed
            .symbols
            .builtin_atom_symbol(BuiltinTypeAtom::Int)
            .and_then(|symbol| {
                self.typed
                    .type_reference_table
                    .find_named_type_reference(symbol)
            });
        [reference; 2]
    }
    /// The reader has successfully formed a primitive operation. Complete only
    /// its attached source question, retaining any other demanded selection.
    pub(super) fn complete_builtin_operator(
        &mut self,
        machine: MachineId,
        source_owner: SymbolHandle,
        expression: ExpressionHandle,
        operands: &[Option<TypeReferenceHandle>],
    ) -> Checked<()> {
        self.complete_operator(machine, source_owner, expression, operands, false)
    }

    fn complete_operator(
        &mut self,
        machine: MachineId,
        source_owner: SymbolHandle,
        expression: ExpressionHandle,
        operands: &[Option<TypeReferenceHandle>],
        known_intrinsic: bool,
    ) -> Checked<()> {
        let typed = self.typed;
        let refuse = |detail: String| Refusal {
            source_span: None,
            machine,
            concerns: Construct::Machine,
            reason: RefusalReason::ForeignIdentity,
            detail,
        };
        if !typed.expression_table.expression_is_valid(expression) {
            return Err(refuse(format!(
                "completed operator has a stale expression {expression:?}"
            )));
        }
        let node = typed.expression_table.expression(expression);
        let spelling = match node {
            ExpressionNode::Binary(binary) => {
                typed_trees::operator::binary_operator_spelling(binary.operator)
            }
            ExpressionNode::Unary(_) => None,
            ExpressionNode::Indexed(indexed) => Some(
                if matches!(
                    typed.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    OperatorSpelling::Range
                } else {
                    OperatorSpelling::Index
                },
            ),
            _ => return Ok(()),
        };
        let mut pending = false;
        for occurrence in typed
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let selection = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    refuse(format!(
                        "completed operator {expression:?} lost occurrence {occurrence:?}"
                    ))
                })?;
            if selection.kind() != Kind::Operator {
                continue;
            }
            match selection.target() {
                Target::LateBound(Binding::CheckedOperator) => pending = true,
                Target::Intrinsic(Intrinsic::BuiltinOperator) => {}
                _ => {
                    return Err(refuse(format!(
                        "primitive operation {expression:?} conflicts with its authored selection {:?}",
                        selection.target(),
                    )));
                }
            }
        }
        if !pending {
            return Ok(());
        }
        // Machine contracts name their entry State; source traversal owns the
        // same clause under its uniquely registered machine declaration.
        let owner = typed
            .unique_machine_by_entry_state_symbol(source_owner)
            .map_or(source_owner, |(declaration, _)| declaration.symbol);
        if !owner.is_valid() || !typed.symbols.symbols().nodes().is_valid(owner) {
            return Err(refuse(format!(
                "completed operator {expression:?} has no exact source owner {source_owner:?}"
            )));
        }
        // Suffixed operations have compiler-owned meaning and cannot bind a
        // declaration. Bare operations still check their actual operand types.
        let suffixed = matches!(node, ExpressionNode::Binary(binary)
            if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact);
        if !known_intrinsic && matches!(node, ExpressionNode::Indexed(_)) {
            let Some(spelling) = spelling else {
                return Ok(());
            };
            // A formed collection operation retains its actual source type.
            // Unknown collections and selected user operators stay pending.
            if operands.first().is_none_or(Option::is_none)
                || !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    typed,
                    spelling,
                    operands,
                    typed.expression_table.source_span(expression),
                )
                .is_empty()
            {
                return Ok(());
            }
        }
        // The static evaluator supplies exact type-equality receipts; typed
        // byte-range formation retains its already selected builtin meaning.
        // Every other bare operation still resolves its actual operand types.
        if !suffixed
            && !known_intrinsic
            && let Some(spelling) = spelling
            && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                typed, owner, expression, spelling, operands,
            )
        {
            // A declaration or abstract operator requirement still needs its
            // actual selection. A primitive read cannot answer on its behalf.
            return Ok(());
        }
        self.completed_operators
            .record_builtin(typed, owner, expression)
            .map_err(|error| refuse(error.to_string()))
    }
    /// Select only a successfully formed pure source operator. The canonical
    /// guarantee remains separate; this is not a semantic-call evaluator.
    pub(super) fn complete_typed_contract_operator(
        &mut self,
        id: MachineId,
        scope: &super::contracts::ClauseScope<'_>,
        formals: arena::RootRows<'_, typed_trees::signature::StateParameter>,
        return_type: TypeReferenceHandle,
        expression: ExpressionHandle,
    ) -> Checked<Option<TypeReferenceHandle>> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let typed = self.typed;
        match typed.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let reference = if path.reserved_result_of != symbols::SymbolHandle::invalid() {
                    Some(return_type)
                } else {
                    formals.iter().find_map(|formal| {
                        (path.resolved_symbol() == Some(formal.symbol))
                            .then_some(formal.type_reference)
                    })
                };
                if reference.is_some() {
                    // The ordinary term reader checks the exact result owner,
                    // formal value identity and retained qualification.
                    self.term(id, scope, expression, None)?;
                }
                if reference.is_some() {
                    return Ok(reference);
                }
                // A named constant supplies its declared type, not a scalar
                // interpretation of an opaque Format/Meaning value.
                Ok(path
                    .resolved_symbol()
                    .and_then(|symbol| typed.const_declaration_by_symbol(symbol))
                    .map(|constant| constant.declared_type))
            }
            ExpressionNode::Boolean(_) => {
                Ok(self.operator_operand_reference(ValueType::plain(ScalarType::Boolean)))
            }
            ExpressionNode::Call(call) => {
                let mut arguments = Vec::new();
                for argument in typed.expression_table.expression_handles(call.arguments) {
                    arguments.push(self.complete_typed_contract_operator(
                        id,
                        scope,
                        formals,
                        return_type,
                        *argument,
                    )?);
                }
                // Exact selected catalog headers supply source types only.
                // Other calls still keep their independent selection questions.
                if call.runtime_call.is_some()
                    || call.receiver.is_valid()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || !typed.symbols.symbols().nodes().is_valid(call.target_symbol)
                {
                    return Ok(None);
                }
                let header = typed
                    .bodyless_machine_signature(call.target_symbol)
                    .map_err(unavailable)?;
                let Some((owner, header)) = header else {
                    return Ok(None);
                };
                if !matches!(
                    typed.symbols.sealed_intrinsic(owner),
                    Some(language_core::sealed_intrinsics::SealedIntrinsic::CompilerPrimitive(_))
                ) {
                    return Ok(None);
                }
                let parameters = super::span(
                    |span| typed.state_parameters.span(span),
                    header.parameters,
                    id,
                    "typed source call parameter span",
                )?;
                if arguments.len() != call.arguments.len() || arguments.len() != parameters.len() {
                    return Err(unavailable("typed source call has another argument roster"));
                }
                for (actual, formal) in arguments.iter().zip(parameters) {
                    let Some(actual) = actual else {
                        return Ok(None);
                    };
                    if !typed.type_reference_table.contains_type_reference(*actual)
                        || !typed
                            .type_reference_table
                            .contains_type_reference(formal.type_reference)
                    {
                        return Err(unavailable("typed source call has a stale argument type"));
                    }
                    let actual = typed.normalized_type_identity(*actual);
                    let expected = typed.normalized_type_identity(formal.type_reference);
                    if !actual.is_valid() || !expected.is_valid() || actual != expected {
                        return Err(unavailable("typed source call changes its argument type"));
                    }
                }
                Ok(Some(header.return_type))
            }
            ExpressionNode::Binary(binary) => {
                let operands = [
                    self.complete_typed_contract_operator(
                        id,
                        scope,
                        formals,
                        return_type,
                        binary.left,
                    )?,
                    self.complete_typed_contract_operator(
                        id,
                        scope,
                        formals,
                        return_type,
                        binary.right,
                    )?,
                ];
                let [Some(left), Some(right)] = operands else {
                    return Err(unavailable(
                        "typed contract operator has no exact operand types",
                    ));
                };
                let left = self.result_type(id, left)?;
                let right = self.result_type(id, right)?;
                let pure = match (binary.operator, left, right) {
                    (
                        BinaryOperator::Equal | BinaryOperator::NotEqual,
                        Some(ValueType::Scalar(left, _)),
                        Some(ValueType::Scalar(right, _)),
                    ) => {
                        left == right
                            && matches!(left, ScalarType::Boolean | ScalarType::Integer(_))
                    }
                    (
                        BinaryOperator::And | BinaryOperator::Or,
                        Some(ValueType::Scalar(ScalarType::Boolean, _)),
                        Some(ValueType::Scalar(ScalarType::Boolean, _)),
                    ) => true,
                    _ => false,
                };
                if !pure {
                    return Err(unavailable(
                        "typed contract has an unformed source operator",
                    ));
                }
                self.complete_builtin_operator(id, scope.owner, expression, &operands)?;
                Ok(self.operator_operand_reference(ValueType::plain(ScalarType::Boolean)))
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                let operand = self.complete_typed_contract_operator(
                    id,
                    scope,
                    formals,
                    return_type,
                    unary.operand,
                )?;
                let Some(reference) = operand else {
                    return Err(unavailable("typed contract negation has no operand type"));
                };
                if self.scalar_type(id, reference)? != ScalarType::Boolean {
                    return Err(unavailable("typed contract negation is not Boolean"));
                }
                self.complete_builtin_operator(id, scope.owner, expression, &[operand])?;
                Ok(operand)
            }
            _ => Ok(None),
        }
    }
}

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
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

use super::Program;
use crate::body::{Expression, ExpressionKind, Policy, Qualification, ValueType};

impl Program<'_> {
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
        let owner = self.symbols.get(machine).copied().unwrap_or_default();
        match &value.kind {
            ExpressionKind::CollectionSubslice { base, .. }
            | ExpressionKind::CollectionView { base, .. }
                if matches!(
                    self.typed.expression_table.expression(expression),
                    ExpressionNode::Indexed(_)
                ) =>
            {
                let Some(ValueType::Record(record, Qualification::NONE, _)) = base.result else {
                    return Ok(());
                };
                let [collection, count] = self.collection_index_operand_types(machine, record);
                let operands = [collection, count, count];
                self.complete_builtin_operator(machine, owner, expression, &operands)
            }
            _ => {
                let operands = match &value.kind {
                    ExpressionKind::Strict { left, right, .. }
                    | ExpressionKind::ShortCircuit { left, right, .. } => {
                        [left.result, right.result]
                    }
                    ExpressionKind::Index { base, index }
                    | ExpressionKind::ByteElement { base, index } => [base.result, index.result],
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
        if matches!(node, ExpressionNode::Indexed(_)) {
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
        if !suffixed
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
}

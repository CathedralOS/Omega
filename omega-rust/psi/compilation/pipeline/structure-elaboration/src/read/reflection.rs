//! Declaration descriptions become ordinary values in the source compiler.
//! The selected declaration and its source stay explicit; no callable body is
//! instantiated, and these byte values grant no permission.

use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
};
use typed_trees::references::StaticArgumentCall;

use super::Program;
use crate::{Expression, ExpressionKind, SourceOrigin, ValueType};

fn invalid(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns: Construct::Machine,
        detail: detail.into(),
    }
}

impl Program<'_> {
    pub(super) fn declaration_query(
        &mut self,
        machine: MachineId,
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let declaration = self
            .typed
            .machine_by_symbol(call.target_symbol)
            .or_else(|| self.typed.machine_holding_state(call.target_symbol));
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        if self.typed.core_vocabulary(declaration.symbol)
            != Some(CoreVocabulary::Reflection(ReflectionDeclaration::Callable))
        {
            return Ok(None);
        }
        let fail = |detail| invalid(machine, detail);
        if call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.static_requirement_dispatch.is_some()
            || call.build_selection.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.static_machine_parameter.is_valid()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(fail(
                "callable description accepts one declaration path and no other call feature",
            ));
        }
        self.direct_selection(
            machine,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.visible_to(machine, declaration.symbol)?;
        let [operand] = self
            .typed
            .expression_table
            .expression_handles(call.arguments)
        else {
            return Err(fail(
                "callable description requires exactly one declaration path",
            ));
        };
        let ExpressionNode::Name(path) = self.node(machine, *operand)? else {
            return Err(fail(
                "callable description requires a declaration path, not an invocation or value",
            ));
        };
        let selected = if path.static_application.is_valid() {
            self.typed
                .expression_table
                .static_member(path.static_application)
                .cloned()
                .ok_or_else(|| fail("callable description lost its selected application"))?
        } else {
            StaticMachineArgument {
                symbol: path.symbol,
                ..Default::default()
            }
        };
        let author = StaticArgumentCall::Expression(ExpressionHandle::invalid(), call)
            .lexical_owner(self.typed);
        let author_span = self
            .typed
            .symbols
            .symbol_provenance_source_span(author)
            .ok_or_else(|| fail("callable description requires its retained lexical author"))?;
        if !selected.symbol.is_valid()
            || !self
                .typed
                .symbols
                .source_reference_may_select_symbol(author_span, selected.symbol)
        {
            return Err(fail(
                "callable description is outside its author's declaration scope",
            ));
        }
        let (requirement, identities) =
            typed_trees::reflection::construction::callable_description_identity(
                self.typed, &selected, author,
            )
            .map_err(|reason| invalid(machine, reason))?;
        let (_, entry) = self.declaration(machine, declaration.symbol)?;
        let result_type = entry.return_type;
        let data = self
            .typed
            .closed_data_definition(result_type)
            .ok_or_else(|| fail("callable description has no exact result declaration"))?;
        if self.typed.core_vocabulary(data.symbol)
            != Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::CallableInfo,
            ))
        {
            return Err(fail(
                "callable description lost its toolchain result vocabulary",
            ));
        }
        let Some(ValueType::Record(record)) = Some(self.value_type(machine, result_type)?) else {
            return Err(fail(
                "callable description result is not structured storage",
            ));
        };
        let alternative = if requirement {
            "Requirement"
        } else {
            "Machine"
        };
        let mut cases = self
            .data_type(machine, record)?
            .cases()
            .iter()
            .enumerate()
            .filter(|(_, case)| case.identity == alternative);
        let (case_index, case) = cases
            .next()
            .ok_or_else(|| fail("callable description lost its result alternative"))?;
        let case = case.clone();
        if cases.next().is_some() {
            return Err(fail("callable description repeats its result alternative"));
        }
        let mut fields = Vec::new();
        for (name, bytes) in [
            "declaration_identity",
            "callable_identity",
            "source_revision",
        ]
        .into_iter()
        .zip(identities)
        {
            let mut matches = case
                .fields
                .iter()
                .enumerate()
                .filter(|(_, field)| field.identity == name);
            let (position, field) = matches
                .next()
                .ok_or_else(|| fail("callable description lost an identity field"))?;
            if matches.next().is_some() {
                return Err(fail("callable description repeats an identity field"));
            }
            fields.push((
                position,
                self.byte_literal(machine, &bytes, Some(field.value_type), source)?,
            ));
        }
        if fields.len() != case.fields.len() {
            return Err(fail("callable description result has an unexpected field"));
        }
        Ok(Some(Expression {
            kind: ExpressionKind::Case {
                case: case_index,
                fields,
            },
            result: Some(ValueType::Record(record)),
            source,
        }))
    }
}

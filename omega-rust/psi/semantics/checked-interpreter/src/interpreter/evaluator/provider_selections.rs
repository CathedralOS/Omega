//! Execute a provider choice, not its product operands. Only the activation's
//! original Build cell can issue a request; an ordinary same-named machine
//! executes normally. Omega validates the retained operand identities and
//! coverage after evaluation, keeping target policy out of this interpreter.

use super::{
    Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall, Value,
    trap,
};
use crate::{ExecutedProviderSelection, ExecutedProviderSelectionSite};
use language_semantics::declaration_selection::BuildOperation;

impl<'program> Evaluator<'program> {
    pub(super) fn try_provider_selection_statement(
        &mut self,
        statement: typed_trees::statement::StatementHandle,
        call: &TableCall,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if BuildOperation::from_call_target(call.target.as_str())
            != Some(BuildOperation::ProviderSelection)
            || call.target_symbol.is_valid()
        {
            return Ok(None);
        }
        let receiver = self.statement_receiver_cell(call, frame)?;
        self.require_provider_selection_receiver(receiver)?;
        let composition_case = self.provider_composition_case(
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )?;
        self.record_provider_selection(
            ExecutedProviderSelectionSite::Statement(statement),
            composition_case,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    pub(super) fn try_provider_selection_value_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if BuildOperation::from_call_target(call.target.as_str())
            != Some(BuildOperation::ProviderSelection)
            || call.target_symbol.is_valid()
        {
            return Ok(None);
        }
        // Evaluate a returned reborrow once; speculative place lookup must not
        // replay a receiver-producing helper or discard its original cell.
        let receiver = self.eval_read_cell(call.receiver, frame)?;
        self.require_provider_selection_receiver(Some(self.deref_cell(receiver)))?;
        let composition_case = self.provider_composition_case(
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )?;
        self.record_provider_selection(
            ExecutedProviderSelectionSite::Expression(expression),
            composition_case,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    fn require_provider_selection_receiver(&self, receiver: Option<Cell>) -> EvalResult<()> {
        if receiver
            .as_ref()
            .zip(self.root_build.as_ref())
            .is_some_and(|(receiver, root)| Cell::ptr_eq(receiver, root))
        {
            Ok(())
        } else {
            trap("provider selection requires the current activation's original Build value")
        }
    }

    fn provider_composition_case(
        &mut self,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<SymbolHandle> {
        if arguments.is_empty() {
            return Ok(SymbolHandle::invalid());
        }
        let [argument] = arguments else {
            return trap("provider selection takes at most one CompositionMode value");
        };
        let Value::Enum {
            type_symbol,
            variant_name,
            payload,
        } = self.eval_expression(*argument, frame)?
        else {
            return trap("provider selection requires a compiler-owned CompositionMode case");
        };
        let definition = self.program.data_definitions().iter().find(|definition| {
            definition.symbol == type_symbol
                && definition.name.as_str() == "CompositionMode"
                && self.symbol_has_build_prelude_source(type_symbol)
        });
        let case = definition.and_then(|definition| {
            self.program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == variant_name && payload.is_empty() =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        });
        case.ok_or_else(|| Halt::Trap("provider selection requires an exact payload-free compiler-owned CompositionMode case".into()))
    }

    fn record_provider_selection(
        &mut self,
        site: ExecutedProviderSelectionSite,
        composition_case: SymbolHandle,
        frame: &Frame,
    ) -> EvalResult<()> {
        self.executed_provider_selections
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("provider-selection result allocation was refused".into())
            })?;
        self.executed_provider_selections
            .push(ExecutedProviderSelection {
                machine: frame.machine_symbol,
                site,
                composition_case,
            });
        Ok(())
    }
}

//! Execute product registration on the activation's original Build cell.
//! Provider choices and assumed relationships retain exact source coordinates;
//! product operands never execute. Relationship registration carries intent only,
//! leaving structural, trust and receiving-policy admission with Omega.

use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall, Value, trap,
};
use crate::{
    ExecutedAssumedBindingRegistration, ExecutedBuildDeclarationSite, ExecutedProviderSelection,
};
use language_semantics::declaration_selection::BuildOperation;

impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn try_build_registration_statement(
        &mut self,
        statement: typed_trees::statement::StatementHandle,
        call: &TableCall,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(
            operation
            @ (BuildOperation::ProviderSelection | BuildOperation::AssumedBindingContract),
        ) = BuildOperation::from_call_target(call.target.as_str())
        else {
            return Ok(None);
        };
        if call.target_symbol.is_valid() {
            return Ok(None);
        }
        self.record_build_registration(
            operation,
            ExecutedBuildDeclarationSite::Statement(statement),
            self.program
                .statement_table
                .expression_handles(call.arguments),
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    pub(in crate::interpreter::evaluator) fn try_build_registration_value_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(
            operation
            @ (BuildOperation::ProviderSelection | BuildOperation::AssumedBindingContract),
        ) = validation::compiler_operations::build_operation(self.program, expression)
        else {
            return Ok(None);
        };
        self.record_build_registration(
            operation,
            ExecutedBuildDeclarationSite::Expression(expression),
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    fn record_build_registration(
        &mut self,
        operation: BuildOperation,
        site: ExecutedBuildDeclarationSite,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<()> {
        let Some((builder, arguments)) = arguments.split_first() else {
            return trap("product registration requires an explicit mutable Build argument");
        };
        // Returned reborrows are read once and still identify the original cell.
        let builder = self.eval_read_cell(*builder, frame)?;
        self.require_root_build_argument(self.deref_cell(builder), operation.authored_spelling())?;
        match operation {
            BuildOperation::ProviderSelection => {
                let composition_case = self.provider_composition_case(arguments, frame)?;
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
            }
            BuildOperation::AssumedBindingContract => {
                if !arguments.is_empty() {
                    return trap(
                        "assumed-binding registration takes only its explicit Build argument",
                    );
                }
                self.executed_assumed_binding_registrations
                    .try_reserve(1)
                    .map_err(|_| {
                        Halt::Resource("assumed-binding registration allocation was refused".into())
                    })?;
                self.executed_assumed_binding_registrations.push(
                    ExecutedAssumedBindingRegistration {
                        machine: frame.machine_symbol,
                        site,
                    },
                );
            }
            _ => return trap("operation does not register a product selection"),
        }
        Ok(())
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
                        if *variant.name.as_str() == *variant_name && payload.is_empty() =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        });
        case.ok_or_else(|| Halt::Trap("provider selection requires an exact payload-free compiler-owned CompositionMode case".into()))
    }
}

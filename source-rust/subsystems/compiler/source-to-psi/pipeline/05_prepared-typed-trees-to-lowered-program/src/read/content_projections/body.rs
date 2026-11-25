//! Form the selected algebra constructor from the projection's single authored value.
use super::{Definition, Program, Header};
use crate::{Unserved, refuse};
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use semantic_vocabulary::{ContentAlgebraKind, ContentProjectionExpression, MachineId};
use terminal_check_tables::{Checked, Construct};

impl Program<'_> {
    pub(super) fn closed_projection_body(
        &mut self,
        id: MachineId,
        symbol: symbols::SymbolHandle,
        header: Header,
        carrier: crate::body::ValueType,
    ) -> Checked<Definition> {
        let missing = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let typed = self.typed;
        let (machine, state) = self.declaration(id, symbol)?;
        let algebra = header.result;
        let kind = header.algebra.kind;
        let exact = |reference, declaration| {
            crate::source_state::reflection::construction::schema_type_identity(
                typed,
                reference,
                declaration,
            )
            .map_err(|_| missing("content projection lost its exact constructor application"))
        };
        // A conformance can inherit a published interface with no termination promise.
        let termination = &machine.termination_plan;
        if termination
            .interface
            .published()
            .is_some_and(source_declarations::TerminationGuarantee::promises_termination)
            || termination.checked_summary.promises_termination()
            || termination.implementation_witness.is_some()
        {
            return Err(missing(
                "content projection has a termination guarantee or ranking witness that requires a separate judgment",
            ));
        }
        if machine.supply_mode != source_declarations::MachineSupplyMode::CheckedBody
            || machine.purpose != crate::source_state::machine::MachinePurpose::Runtime
            || machine.structural_type_equations_pending
            || machine.admitted_same_stack_bytes != 0
            || !machine.body_is_present
            || machine.body_status != crate::source_state::machine::MachineBodyStatus::Complete
            || !machine.type_parameters.is_empty()
            || !machine.conformance_bounds.is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.declared_type_equations.is_empty()
            || machine.assembly_target_requirement
                != language_core::inline_assembly::AsmTargetApplicability::Any
            || !machine.owned_data.is_empty()
            || !machine.invokes.is_empty()
            || !machine.contracts.is_empty()
            || !state.contracts.is_empty()
            || machine.suspends
            || machine.blocks
            || machine.target.is_some()
            || machine.spelling.is_some()
            || machine.lexical_region_origin.is_some()
            || machine.service_reach_is_installation_bound
            || machine.target_family_interface
            || machine.service_reach_row.0 != 0
                && !super::super::services::finite_reach(typed, id, machine.service_reach_row)?
                    .is_empty()
        {
            return Err(missing(
                "content projection must publish one closed effect-free body",
            ));
        }
        let body = state.returned_expression(typed).ok_or_else(|| {
            missing("content projection must return a closed algebra constructor")
        })?;
        let table = &typed.expression_table;
        if !table.expression_is_valid(body) {
            return Err(missing("content projection lost its returned expression"));
        }
        let ExpressionNode::StructLiteral(literal) = table.expression(body) else {
            return Err(missing(
                "content projection must construct its selected algebra",
            ));
        };
        if literal.zero_omitted_fields
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
            || literal.application_type.is_valid()
                && exact(literal.application_type, symbol)? != exact(algebra, symbol)?
            || typed
                .data_constructor_definition(algebra, literal.type_symbol)
                .is_none()
        {
            return Err(missing(
                "content projection must construct exactly its selected algebra application",
            ));
        }
        let fields = table.struct_fields(literal.fields);
        let value = |name| -> Checked<ExpressionHandle> {
            let mut matching = fields.iter().filter_map(|written| {
                let field = typed.data_constructor_field(
                    algebra,
                    literal.type_symbol,
                    written.field_symbol,
                )?;
                (field.name.as_str() == name && written.name == field.name).then_some(written.value)
            });
            let value = matching
                .next()
                .ok_or_else(|| missing("content algebra missing a declared constructor field"))?;
            if matching.next().is_some() {
                return Err(missing("content algebra repeats a constructor field"));
            }
            Ok(value)
        };
        let Some([subject]) = typed.state_parameters(state).as_array() else {
            return Err(missing("content projection lost its shared carrier"));
        };
        let expression = match kind {
            ContentAlgebraKind::CountedQuantity if fields.len() == 1 => {
                ContentProjectionExpression::CountedQuantity(self.content_natural(
                    id,
                    value("magnitude")?,
                    subject.symbol,
                    state.symbol,
                    carrier,
                )?)
            }
            ContentAlgebraKind::IntervalSet if fields.len() == 2 => {
                let lower = self.content_natural(
                    id,
                    value("start")?,
                    subject.symbol,
                    state.symbol,
                    carrier,
                )?;
                let upper =
                    self.content_natural(id, value("end")?, subject.symbol, state.symbol, carrier)?;
                // The checking stage proves endpoint order from the actual subject fields.
                ContentProjectionExpression::IntervalSet(if lower == upper {
                    Vec::new()
                } else {
                    vec![(lower, upper)]
                })
            }
            _ => {
                return Err(missing(
                    "content algebra constructor must supply every field once",
                ));
            }
        };
        Ok(Definition {
            algebra: header.algebra,
            expression,
        })
    }
}

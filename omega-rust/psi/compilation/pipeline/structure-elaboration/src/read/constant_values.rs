//! Constant values arrive as materialized source data, never as a source
//! checking verdict. Source completion joins computed values to their real
//! invocation receipts before freezing. Reading independently checks the
//! scalar carrier, value and exact retained initializer entry; that entry is
//! an ordinary subject machine whose body the lowered checker also judges.

use language_semantics::const_value::DecodedCanonicalConstValue;
use semantic_vocabulary::{IntegerValue, MachineId};
use terminal_check_tables::{Checked, Construct};
use typed_trees::constant::ConstDeclaration;

use super::Program;
use crate::body::{ExpressionKind, ValueType};
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(super) fn subject_constant(
        &mut self,
        id: MachineId,
        constant: &ConstDeclaration,
    ) -> Checked<()> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Declaration(what));
        let carrier = self.value_type(id, constant.declared_type)?;
        let ValueType::Scalar(_, qualification) = carrier else {
            return Err(unserved("constant requires its pure snapshot materializer"));
        };
        if !qualification.is_none() {
            return Err(unserved("qualified constant requires its declared value obligations"));
        }
        let value = typed_trees::canonical_const_declaration_value(self.typed, constant.symbol)
            .and_then(|value| value.decode_encoding())
            .ok_or_else(|| unserved("constant has no evaluated canonical value"))?;
        if !self.typed.expression_table.expression_is_valid(constant.materialized_initializer) {
            return Err(unserved("constant has no materialized initializer"));
        }
        let materialized = self.expression(id, &[], constant.materialized_initializer, Some(carrier))?;
        let agrees = match (&value, materialized.kind) {
            (DecodedCanonicalConstValue::Integer { value, .. }, ExpressionKind::Integer(IntegerValue::Signed(current))) =>
                *value == current,
            (DecodedCanonicalConstValue::Integer { value, .. }, ExpressionKind::Integer(IntegerValue::Unsigned(current))) =>
                i128::try_from(current).ok() == Some(*value),
            (DecodedCanonicalConstValue::Boolean(value), ExpressionKind::Boolean(current)) =>
                *value == current,
            _ => false,
        };
        if !agrees || materialized.result != Some(carrier) {
            return Err(unserved("constant encoding and materialized carrier differ"));
        }
        if constant.evaluation_entry.is_valid() {
            if !self.typed.tables.machines.is_valid(constant.evaluation_entry) {
                return Err(unserved("constant lost its retained initializer entry"));
            }
            let retained = self.typed.tables.machines.get(constant.evaluation_entry);
            let actual = self.typed.machine_by_symbol(retained.symbol)
                .filter(|actual| *actual == retained)
                .ok_or_else(|| unserved("constant initializer is not its actual source machine"))?;
            let [state] = self.typed.machine_states(actual) else {
                return Err(unserved("constant initializer has no unique entry"));
            };
            let [typed_trees::statement::StatementNode::Transition(transition)] =
                self.typed.statement_table.statements(state.statement_nodes) else {
                    return Err(unserved("constant initializer has no exact value transition"));
                };
            if state.return_type != constant.declared_type
                || !self.typed.state_parameters(state).is_empty()
                || self.typed.statement_table.transition_target(transition.target)
                    != &typed_trees::statement::TransitionTargetNode::Value(constant.evaluation_expression)
            {
                return Err(unserved("constant initializer differs from its retained computation"));
            }
        }
        Ok(())
    }
}

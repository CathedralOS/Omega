//! Transparent source mathematics in callable premises. This only translates
//! typed source expressions; the ordinary prover and kernel decide their truth.
//! Address bounds come from the exact invocation's retained mathematical model.

use semantic_vocabulary::{IntegerMathTerm, IntegerValue, MachineId, Proposition, ScalarType};
use symbols::{BuiltinFunction, BuiltinTypeMember, SymbolHandle};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::mathematical::{MathematicalBody, MathematicalDefinition, MathematicalType};
use typed_trees::types::TypeReferenceNode;

use super::{
    Program,
    contracts::{ClauseScope, ResultTerm},
    logical::MathematicalInputs,
};
use crate::{Unserved, refuse};

impl Program<'_> {
    fn mathematical_definition(&self, symbol: SymbolHandle) -> Option<&MathematicalDefinition> {
        if !self.typed.symbols.symbols().nodes().is_valid(symbol)
            || self.typed.symbols.get(symbol).kind != symbols::SymbolKind::MathematicalDefinition
        {
            return None;
        }
        self.typed
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    fn address_bound(&self, expression: ExpressionHandle) -> bool {
        let symbol = match self.typed.expression_table.expression(expression) {
            ExpressionNode::Name(name) => name.symbol,
            ExpressionNode::Member(member) => member.selection.field_symbol(),
            _ => return false,
        };
        self.typed.symbols.builtin_type_member(symbol) == Some(BuiltinTypeMember::AddressBound)
    }

    pub(super) fn is_mathematical_term(&self, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return false;
        }
        if self.address_bound(expression) {
            return true;
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.typed
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(BuiltinFunction::IntegerEmbed)
                    || self.mathematical_definition(call.target_symbol).is_some()
            }
            ExpressionNode::Binary(binary) => {
                self.is_mathematical_term(binary.left) || self.is_mathematical_term(binary.right)
            }
            ExpressionNode::Name(name) => self.mathematical_definition(name.symbol).is_some(),
            _ => false,
        }
    }

    fn mathematical_application<'a>(
        &self,
        id: MachineId,
        scope: &ClauseScope<'a>,
        call: &TableCallExpression,
    ) -> Checked<(
        &MathematicalDefinition,
        Vec<(SymbolHandle, semantic_vocabulary::ScalarTerm)>,
    )> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        let definition = self
            .mathematical_definition(call.target_symbol)
            .ok_or_else(|| {
                unserved("call in a premise without a transparent mathematical declaration")
            })?;
        if !definition.binders.is_empty()
            || call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return Err(unserved("open mathematical premise application"));
        }
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        if call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return Err(unserved(
                "mathematical premise has an unformed special application",
            ));
        }
        let parameters = self.typed.mathematical_parameters(definition.parameters);
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        if parameters.len() != definition.parameters.len()
            || arguments.len() != call.arguments.len()
            || parameters.len() != arguments.len()
        {
            return Err(unserved("mathematical premise argument correspondence"));
        }
        let mut bound = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !self.typed.mathematical_types.is_valid(parameter.ty) {
                return Err(unserved("mathematical premise input has no formed type"));
            }
            let MathematicalType::Ordinary(reference) = self.typed.mathematical_type(parameter.ty)
            else {
                return Err(unserved("higher-order mathematical premise input"));
            };
            let scalar = super::logical::scalar_type(
                self.typed,
                id,
                *reference,
                scope.logical.and_then(|logical| logical.model),
            )?;
            let (term, actual) = self.term(id, scope, *argument, Some(scalar))?;
            if actual != crate::body::ValueType::Scalar(scalar, crate::body::Qualification::NONE) {
                return Err(unserved("mathematical premise input carrier differs"));
            }
            bound.push((parameter.symbol, term));
        }
        Ok((definition, bound))
    }

    pub(super) fn mathematical_clause(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Proposition> {
        let (definition, bound) = self.mathematical_application(id, scope, call)?;
        if !self.predicate_result(definition) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("mathematical premise requires a formed Core Strict<0> result"),
            ));
        }
        let MathematicalBody::Definition(expression) = definition.body else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("uninterpreted mathematical premise theory"),
            ));
        };
        let logical = scope.logical.ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("mathematical premise without a logical input environment"),
            )
        })?;
        if logical.expanding.contains(&definition.symbol) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("recursive mathematical premise theory"),
            ));
        }
        let mut expanding = logical.expanding.to_vec();
        expanding.push(definition.symbol);
        // Declaration formation sees only its own formals, never the caller's
        // runtime values, domain subject or facts. A supported proposition is
        // a Strict<0> term by construction, validated at all formal inputs.
        let formal_inputs = MathematicalInputs::new(
            self.typed,
            id,
            self.typed.mathematical_parameters(definition.parameters),
            logical.model,
        )?;
        let declaration_scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner: definition.symbol,
            result: ResultTerm::Absent("result in a mathematical predicate"),
            subject: None,
            formal_fields: true,
            mathematical: super::contracts::MathematicalTerms::Served,
            logical: Some(formal_inputs.scope(id, logical.model, &expanding)),
        };
        let formed = self.clause(id, &declaration_scope, expression)?;
        formal_inputs.validate(id, &formed)?;
        let actual_inputs = MathematicalInputs::empty();
        let scope = ClauseScope {
            bound: &bound,
            logical: Some(actual_inputs.scope(id, logical.model, &expanding)),
            ..declaration_scope
        };
        self.clause(id, &scope, expression)
    }

    fn predicate_result(&self, definition: &MathematicalDefinition) -> bool {
        if !self.typed.mathematical_types.is_valid(definition.result) {
            return false;
        }
        let MathematicalType::Ordinary(reference) = self.typed.mathematical_type(definition.result)
        else {
            return false;
        };
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(*reference)
        {
            return false;
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = self.typed.type_reference_table.type_reference(*reference)
        else {
            return false;
        };
        if self
            .typed
            .symbols
            .fixed_core_declaration_symbol(symbols::FixedCoreDeclaration::Strict)
            != Some(*base_symbol)
            || !lifetime_arguments.is_empty()
            || arguments.len() != 1
        {
            return false;
        }
        let arguments = self
            .typed
            .type_reference_table
            .type_reference_handles(*arguments);
        let [argument] = arguments else { return false };
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(*argument)
        {
            return false;
        }
        let TypeReferenceNode::ConstExpression(expression) =
            self.typed.type_reference_table.type_reference(*argument)
        else {
            return false;
        };
        self.typed.expression_table.expression_is_valid(*expression)
            && matches!(self.typed.expression_table.expression(*expression), ExpressionNode::Integer(literal) if literal.value_u64() == Some(0))
    }

    pub(super) fn logical_mathematical_term(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<IntegerMathTerm> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        if !expression.is_valid() {
            return Err(unserved("missing mathematical premise term"));
        }
        if self.address_bound(expression) {
            let model = scope
                .logical
                .and_then(|logical| logical.model)
                .ok_or_else(|| {
                    unserved("address bound without the binding's mathematical model")
                })?;
            let bound = 1_u128
                .checked_shl(u32::from(model.carrier.bits()))
                .ok_or_else(|| unserved("address bound outside mathematical literal support"))?;
            return Ok(IntegerMathTerm::literal(IntegerValue::Unsigned(bound)));
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call)
                if self
                    .typed
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(BuiltinFunction::IntegerEmbed) =>
            {
                if call.receiver.is_valid()
                    || call.requirement_application.is_valid()
                    || call.exact_coercion_target.is_valid()
                    || call.build_selection.is_valid()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.layout_query.is_some()
                    || call.native_code_layout.is_some()
                {
                    return Err(unserved("integer embedding has an unformed application"));
                }
                self.applies_nothing(
                    id,
                    call.target_symbol,
                    call.authored_machine_application,
                    &call.machine_arguments,
                )?;
                let arguments = self
                    .typed
                    .expression_table
                    .expression_handles(call.arguments);
                let [argument] = arguments else {
                    return Err(unserved("integer embedding argument correspondence"));
                };
                let (term, carrier) = self.term(id, scope, *argument, None)?;
                if !matches!(
                    carrier,
                    crate::body::ValueType::Scalar(ScalarType::Integer(_), _)
                ) {
                    return Err(unserved("non-integer mathematical embedding"));
                }
                IntegerMathTerm::embed(term)
                    .ok_or_else(|| unserved("integer embedding without its carrier"))
            }
            ExpressionNode::Binary(binary) => {
                // Only explicit mathematical arithmetic is total here. A
                // fixed-width partial operand inside a mathematical clause
                // must not be reinterpreted as an unbounded operation.
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
                    || !(self.is_mathematical_term(binary.left)
                        || self.is_mathematical_term(binary.right))
                {
                    return Err(unserved(
                        "mathematical premise contains computational arithmetic",
                    ));
                }
                let left = Box::new(self.logical_mathematical_term(id, scope, binary.left)?);
                let right = Box::new(self.logical_mathematical_term(id, scope, binary.right)?);
                match binary.operator {
                    BinaryOperator::Add => Ok(IntegerMathTerm::Add(left, right)),
                    BinaryOperator::Subtract => Ok(IntegerMathTerm::Subtract(left, right)),
                    BinaryOperator::Multiply => Ok(IntegerMathTerm::Multiply(left, right)),
                    _ => Err(unserved("mathematical premise arithmetic")),
                }
            }
            ExpressionNode::Integer(literal) => {
                let (value, _) = self.literal(id, literal, None)?;
                Ok(IntegerMathTerm::literal(value))
            }
            _ => Err(unserved(
                "mathematical premise term without explicit integer embedding",
            )),
        }
    }
}

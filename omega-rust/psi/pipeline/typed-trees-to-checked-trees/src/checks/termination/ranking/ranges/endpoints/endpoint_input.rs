//! Exact scalar or direct-field inputs of an independently formed endpoint.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataField;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

pub(super) struct EndpointInput<'program> {
    pub argument_position: usize,
    parameter: &'program StateParameter,
    field: Option<&'program DataField>,
    owner: SymbolHandle,
}

impl<'program> EndpointInput<'program> {
    /// The caller has already proved this whole expression's invariant bounds.
    /// Resolve its leaves again for exact arrival identity, not value inference.
    pub fn resolve(
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        let member = match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => Some(member),
            ExpressionNode::Name(_) => None,
            _ => return None,
        };
        let receiver = member.map_or(expression, |member| member.receiver);
        let ExpressionNode::Name(name) = program.expression_table.expression(receiver) else {
            return None;
        };
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        let (argument_position, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| {
                name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && parameter.symbol == name.symbol
                    && parameter.name == *spelling
                    && !parameter.is_mutable
                    && !parameter.is_const
            })?;
        let mut input = Self {
            argument_position,
            parameter,
            field: None,
            owner: SymbolHandle::invalid(),
        };
        if let Some(member) = member {
            let TypeReferenceNode::Named { symbol: owner, .. } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return None;
            };
            if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                return None;
            }
            let declaration = program
                .data_definitions()
                .iter()
                .find(|declaration| owner.is_valid() && declaration.symbol == *owner)?;
            input.field = Some(validation::exact_data_member_field(
                program,
                declaration,
                member.member_symbol,
                member.member.as_str(),
                None,
            )?);
            input.owner = *owner;
        }
        Some(input)
    }

    pub fn path(&self) -> String {
        match self.field {
            Some(field) => format!("{}.{}", self.parameter.name.as_str(), field.name.as_str()),
            None => self.parameter.name.as_str().to_owned(),
        }
    }

    pub fn preserved_by(&self, program: &TypedTrees, actual: ExpressionHandle) -> bool {
        if self.is_parameter(program, actual) {
            return true;
        }
        let Some(field) = self.field else {
            return false;
        };
        let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(actual)
        else {
            return false;
        };
        if literal.type_symbol != self.owner
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
        {
            return false;
        }
        let mut matching = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|actual| actual.field_symbol == field.symbol && actual.name == field.name);
        let Some(actual) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        matches!(program.expression_table.expression(actual.value), ExpressionNode::Member(member)
            if member.member_symbol == field.symbol
                && member.member == field.name
                && member.case_variant.is_none()
                && self.is_parameter(program, member.receiver))
    }

    fn is_parameter(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        matches!(program.expression_table.expression(expression), ExpressionNode::Name(name)
            if name.symbol == self.parameter.symbol && name.head_symbol == name.symbol
                && matches!(program.expression_table.name_path_members(name.members),
                    [spelling] if *spelling == self.parameter.name))
    }
}

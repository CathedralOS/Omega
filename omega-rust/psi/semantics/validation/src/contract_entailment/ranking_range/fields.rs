//! A direct owned field is a distinct arithmetic coordinate, never its record.

use super::*;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{DataField, DataMember};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

/// Relational membership cannot bypass the existing endpoint formation owner.
/// Bare Exact parameters perform no arithmetic; computed scalar endpoints need
/// independently defined, representable intermediates before normalization.
pub(super) fn endpoints_formed(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    range: &typed_trees::expression::TableRangeExpression,
) -> Option<()> {
    for endpoint in [range.start, range.end] {
        if projected_type(program, state, endpoint).is_some_and(|reference| {
            exact_integer_parameter(program, reference) == Some(PrimitiveType::U64)
        }) {
            // An exact immutable field read performs no endpoint arithmetic.
            continue;
        }
        if parameter(program, state, endpoint).is_some_and(|parameter| {
            exact_integer_parameter(program, parameter.type_reference).is_some()
        }) {
            continue;
        }
        crate::immutable_integer_expression_bounds(program, machine, state, endpoint)?;
    }
    Some(())
}

pub(super) struct FieldCoordinate<'program> {
    pub parameter: &'program StateParameter,
    pub field: &'program DataField,
    pub owner: SymbolHandle,
    pub identity: String,
}

impl<'program> FieldCoordinate<'program> {
    pub(super) fn resolve(
        program: &'program TypedTrees,
        state: &State,
        subject: ExpressionHandle,
        field: SymbolHandle,
    ) -> Option<Self> {
        let parameter = parameter(program, state, subject)?;
        Self::for_parameter(program, parameter, field)
    }

    fn for_parameter(
        program: &'program TypedTrees,
        parameter: &'program StateParameter,
        field: SymbolHandle,
    ) -> Option<Self> {
        if parameter.is_self || parameter.is_mutable || parameter.is_const {
            return None;
        }
        let (owner, field) = declared_field(program, parameter, field)?;
        // The independently selected direct-field view produces builtin u64.
        // Other carriers need their own view-application proof.
        if exact_integer_parameter(program, field.type_reference) != Some(PrimitiveType::U64) {
            return None;
        }
        Some(Self {
            parameter,
            field,
            owner,
            identity: format!("\0ranking:field:{:?}:{:?}", parameter.symbol, field.symbol),
        })
    }

    /// A telescope names a role, not record compatibility or equality between
    /// copies. Only a unique owned formal with the exact declaration can carry
    /// this coordinate; duplicated record roles need independent field facts.
    pub(super) fn at_arrival(
        &self,
        program: &'program TypedTrees,
        arrival: RankingRangeState<'_>,
        entry_symbol: SymbolHandle,
    ) -> Option<Self> {
        if !entry_symbol.is_valid() {
            return None;
        }
        let mut parameters = program
            .state_parameters(arrival.state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arrival.entry_parameters)
            .filter_map(|(parameter, entry)| (*entry == entry_symbol).then_some(parameter));
        let parameter = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        let coordinate = Self::for_parameter(program, parameter, self.field.symbol)?;
        (coordinate.owner == self.owner).then_some(coordinate)
    }

    pub(super) fn value(&self) -> Polynomial {
        Polynomial::atom(self.identity.clone())
    }

    pub(super) fn comparisons(&self, program: &TypedTrees) -> Vec<Comparison> {
        let mut comparisons = vec![(
            BinaryOperator::GreaterOrEqual,
            self.value(),
            Polynomial::default(),
        )];
        if let Some((minimum, maximum)) =
            crate::enforced_integer_type_bounds(program, self.field.type_reference)
        {
            comparisons.extend([
                (
                    BinaryOperator::GreaterOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(minimum)),
                ),
                (
                    BinaryOperator::LessOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(maximum)),
                ),
            ]);
        }
        comparisons
    }

    pub(super) fn actual(
        &self,
        program: &TypedTrees,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
    ) -> Option<Polynomial> {
        if parameter(program, state, expression)
            .is_some_and(|parameter| parameter.symbol == self.parameter.symbol)
        {
            return Some(self.value());
        }
        let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        if literal.type_symbol != self.owner
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
        {
            return None;
        }
        let mut fields = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|field| {
                field.field_symbol == self.field.symbol && field.name == self.field.name
            });
        let value = fields.next()?.value;
        if fields.next().is_some() {
            return None;
        }
        engine.normalize(value)
    }
}

pub(super) fn projected_type(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameter = parameter(program, state, member.receiver)?;
    let (_, field) = declared_field(program, parameter, member.member_symbol)?;
    (member.case_variant.is_none()
        && member.member == field.name
        && exact_integer_parameter(program, field.type_reference).is_some())
    .then_some(field.type_reference)
}

fn parameter<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    program.state_parameters(state).iter().find(|parameter| {
        path.symbol.is_valid()
            && path.symbol == path.head_symbol
            && parameter.symbol == path.symbol
            && parameter.name == *name
            && !parameter.is_self
            && !parameter.is_mutable
            && !parameter.is_const
    })
}

fn declared_field<'program>(
    program: &'program TypedTrees,
    parameter: &StateParameter,
    symbol: SymbolHandle,
) -> Option<(SymbolHandle, &'program DataField)> {
    let TypeReferenceNode::Named { symbol: owner, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let declaration = program
        .data_definitions()
        .iter()
        .find(|data| owner.is_valid() && data.symbol == *owner)?;
    let selected = program.symbols.get(symbol);
    if !symbol.is_valid() || selected.kind != SymbolKind::Field || selected.parent != *owner {
        return None;
    }
    let mut fields = program
        .data_members(declaration)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.symbol == symbol => Some(field),
            _ => None,
        });
    let field = fields.next()?;
    fields.next().is_none().then_some((*owner, field))
}

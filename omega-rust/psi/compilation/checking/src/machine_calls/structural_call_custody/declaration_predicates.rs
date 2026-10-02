//! The declaration predicate roster: the `where` facts of every data
//! declaration a state's parameters reach, spelled in the Terminal
//! requirement vocabulary over exact parameter paths.
//!
//! [Mutable arguments and declared field domains](../../../../../../../wiki/spec/language/dependent_values.md#mutable-arguments-and-declared-field-domains):
//! an ordinary `value: &mut T` argument carries the declared default-domain
//! obligations of `T` at call, entry and return. The caller establishes them,
//! the callee assumes them, and returning execution restores them under the
//! ordinary [invariant-window](../../../../../../../wiki/spec/language/dependent_values.md#invariant-windows)
//! rules. Checking proves those obligations over the authored facts; this
//! roster is how Terminal carries them. Each row observes one parameter's
//! storage and is published three ways: as a call-entry requirement, as a
//! state-arrival obligation, and, for an exclusive borrow, as a normal-return
//! guarantee. Terminal re-proves every row at each consumption point it owns;
//! naming the roster grants no fact.
//!
//! A definition's facts enter the roster in one of three forms:
//!
//! - An interval-only fact set bounds each field's own Terminal carrier
//!   (`data_where_field_intervals`), which every store proves and every read
//!   assumes. It occupies no row.
//! - A fact the bounded vocabulary spells directly (integer, Boolean and IEEE
//!   comparisons over field leaves and literals, and their connectives) is one
//!   row with the same meaning.
//! - A [collection-predicate](../../../../../../../wiki/spec/language/dependent_values.md#collection-invariants)
//!   application `P(&field, ..) == true` contributes its checked elementwise
//!   consequence: one row per element bounding it to the interval it holds on
//!   every path of `P`'s own body that returns `true`. Naming `P` grants
//!   nothing; exploring its checked states does.
//!
//! A definition is *carried* (`declaration_facts_carried`) when every fact
//! has one of these forms and each collection consequence is exact: exploring
//! `P` again over the consequence intervals decides `true`, so the rows imply
//! the fact. Only a carried definition's storage accepts Terminal stores,
//! because then the rows Terminal re-proves before relying on the facts are
//! the complete default domain. Machines publish this roster independently of
//! their body topology. Failure to establish a row at a state arrival, call,
//! or normal return rejects the proof instead of weakening the contract.
//! A zero-gated definition stays out entirely: its zero image violates its
//! facts, so its construction owns them. Descent continues beneath it, and
//! through literal-length fixed arrays once per element at its exact index.

use super::super::super::proof_contracts::collection_predicates;
use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerComparisonKind,
    CheckedOperatorFacts, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use language_core::ReferenceAccess;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// One Terminal-carried consequence of a declared `where` fact standing on
/// storage one parameter reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclarationPredicateRow {
    /// The observed parameter's position in the state's authored parameter
    /// list, the `parameter_position` every structural leaf in `predicate`
    /// names.
    pub parameter_position: u32,
    /// The parameter is an exclusive borrow, so returning execution restores
    /// the row and a caller imports it after the call.
    pub restored_on_return: bool,
    pub predicate: CheckedBooleanExpression,
}

/// The roster of one state of `machine`, in parameter order, then the
/// depth-first declaration order of the storage each parameter reaches.
pub fn state_declaration_predicates(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Vec<DeclarationPredicateRow> {
    let mut rows = Vec::new();
    for (position, parameter) in program.state_parameters(state).iter().enumerate() {
        let Ok(parameter_position) = u32::try_from(position) else {
            break;
        };
        if parameter.is_const || parameter.relevance.is_erased() {
            continue;
        }
        let (referee, access) = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => (*referee, Some(*access)),
            _ => (parameter.type_reference, None),
        };
        // A write-only loan has no readable content to establish or assume.
        let restored_on_return = match access {
            Some(ReferenceAccess::Mutable) => true,
            Some(ReferenceAccess::Shared) => false,
            Some(ReferenceAccess::WriteOnly) => continue,
            None => parameter.is_self && parameter.is_mutable,
        };
        let mut emitter = RowEmitter {
            program,
            operators,
            parameter_position,
            restored_on_return,
            rows: &mut rows,
        };
        match parameter_data_definition(program, machine, referee) {
            // A receiver names its owner as `Self`, and a lifetime-only
            // application names the declaration itself; both own the
            // declaration's storage exactly as its plain name does.
            Some(definition) => {
                let mut visited = vec![definition.symbol];
                emitter.definition_storage(&mut Vec::new(), definition, &mut visited);
            }
            None => emitter.descend(&mut Vec::new(), referee, &mut Vec::new()),
        }
    }
    rows
}

/// The data declaration a parameter's referee names when the ordinary field
/// resolution cannot: the attached owner behind `Self`, or the declaration a
/// lifetime-only application instantiates.
fn parameter_data_definition<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
    referee: TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    let owner = match program
        .type_reference_table
        .type_reference(peel_constrained(program, referee))
    {
        TypeReferenceNode::Named { symbol, name }
            if *symbol == machine.symbol || name.as_str() == "Self" =>
        {
            machine.attached_data_symbol
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if arguments.is_empty() => *base_symbol,
        _ => return None,
    };
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == owner && definition.generic_instance.is_none())
}

/// The roster of a machine's entry state: its invocation-entry requirements
/// and, for exclusive borrows, its normal-return guarantees.
///
/// These rows describe obligations, independently of the body's topology or
/// call graph. An authored state transition proves its destination rows, and
/// every normal return proves the exclusive rows again. A loop therefore needs
/// checked arrival invariants; neither its first entry nor a callee's name can
/// manufacture restoration. Missing proof rejects publication instead of
/// silently deleting a machine's declared-domain contract.
pub fn machine_declaration_predicates(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
) -> Vec<DeclarationPredicateRow> {
    program
        .machine_states(machine)
        .first()
        .map(|entry| state_declaration_predicates(program, operators, machine, entry))
        .unwrap_or_default()
}

/// Whether every `where` fact of `definition` is carried by roster rows that
/// imply it, so Terminal re-proves its complete default domain.
pub fn declaration_facts_carried(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
) -> bool {
    if definition.where_facts.is_empty()
        || crate::data_where_field_intervals(program, definition).is_some()
    {
        return true;
    }
    if definition.zero_gated {
        return false;
    }
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .all(|fact| {
            let ProofFact::Expression(expression) = fact else {
                return false;
            };
            let spelled = FactSpelling {
                program,
                operators,
                parameter_position: 0,
                prefix: &[],
                definition,
            };
            spelled.boolean(*expression).is_some()
                || collection_predicates::data_fact_element_consequences(
                    program,
                    definition,
                    *expression,
                )
                .is_some_and(|consequence| consequence.exact)
        })
}

struct RowEmitter<'a, 'program> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    parameter_position: u32,
    restored_on_return: bool,
    rows: &'a mut Vec<DeclarationPredicateRow>,
}

impl RowEmitter<'_, '_> {
    /// Emit the rows of the storage `type_reference` owns at `prefix`:
    /// a data definition's own facts, then its owned fields; a literal-length
    /// fixed array's elements at each exact index. `visited` is the
    /// definition stack on this path, so a recursive carrier cannot loop.
    fn descend(
        &mut self,
        prefix: &mut Vec<CheckedStructuralPredicatePathSegment>,
        type_reference: TypeReferenceHandle,
        visited: &mut Vec<SymbolHandle>,
    ) {
        let carrier = peel_constrained(self.program, type_reference);
        match self.program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                if owned_data_definition(self.program, *element_type).is_none() {
                    return;
                }
                for index in 0..*length {
                    let Ok(index) = u64::try_from(index) else {
                        return;
                    };
                    prefix.push(CheckedStructuralPredicatePathSegment::FixedIndex(index));
                    self.descend(prefix, *element_type, visited);
                    prefix.pop();
                }
            }
            _ => {
                let Some(definition) = owned_data_definition(self.program, carrier) else {
                    return;
                };
                if visited.contains(&definition.symbol) {
                    return;
                }
                visited.push(definition.symbol);
                self.definition_storage(prefix, definition, visited);
                visited.pop();
            }
        }
    }

    /// Emit one definition's own rows at `prefix`, then descend through its
    /// owned relevant fields. The caller has pushed `definition` on
    /// `visited`.
    fn definition_storage(
        &mut self,
        prefix: &mut Vec<CheckedStructuralPredicatePathSegment>,
        definition: &DataDefinition,
        visited: &mut Vec<SymbolHandle>,
    ) {
        self.definition_rows(prefix, definition);
        for member in self.program.data_members(definition) {
            let DataMember::Field(field) = member else {
                continue;
            };
            if !field.symbol.is_valid() || field.relevance.is_erased() {
                continue;
            }
            // A borrowed field's referee is not this storage.
            if matches!(
                self.program
                    .type_reference_table
                    .type_reference(peel_constrained(self.program, field.type_reference)),
                TypeReferenceNode::Reference { .. }
            ) {
                continue;
            }
            prefix.push(CheckedStructuralPredicatePathSegment::Field(
                field.path_identity(),
            ));
            self.descend(prefix, field.type_reference, visited);
            prefix.pop();
        }
    }

    fn definition_rows(
        &mut self,
        prefix: &[CheckedStructuralPredicatePathSegment],
        definition: &DataDefinition,
    ) {
        if definition.zero_gated
            || definition.where_facts.is_empty()
            || crate::data_where_field_intervals(self.program, definition).is_some()
        {
            return;
        }
        let spelling = FactSpelling {
            program: self.program,
            operators: self.operators,
            parameter_position: self.parameter_position,
            prefix,
            definition,
        };
        for fact in self
            .program
            .proof_facts
            .span_or_empty(definition.where_facts)
        {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(predicate) = spelling.boolean(*expression) {
                self.push(predicate);
                continue;
            }
            let Some(consequence) = collection_predicates::data_fact_element_consequences(
                self.program,
                definition,
                *expression,
            ) else {
                continue;
            };
            for field_consequence in consequence.fields {
                let Some(field) =
                    find_data_field(self.program, definition, field_consequence.field, "")
                else {
                    continue;
                };
                let Some(primitive) = field_consequence_primitive(self.program, field) else {
                    continue;
                };
                let mut path = prefix.to_vec();
                path.push(CheckedStructuralPredicatePathSegment::Field(
                    field.path_identity(),
                ));
                match field_consequence.elements {
                    collection_predicates::ConsequenceShape::Array(intervals) => {
                        for (index, interval) in intervals.into_iter().enumerate() {
                            let Ok(index) = u64::try_from(index) else {
                                break;
                            };
                            let mut element = path.clone();
                            element.push(CheckedStructuralPredicatePathSegment::FixedIndex(index));
                            if let Some(predicate) = self.interval_row(element, primitive, interval)
                            {
                                self.push(predicate);
                            }
                        }
                    }
                    collection_predicates::ConsequenceShape::Integer(interval) => {
                        if let Some(predicate) = self.interval_row(path, primitive, interval) {
                            self.push(predicate);
                        }
                    }
                }
            }
        }
    }

    /// `minimum <= leaf && leaf <= maximum`, omitting a side the carrier
    /// already guarantees; `None` when both sides are the carrier's own.
    fn interval_row(
        &self,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        primitive: PrimitiveType,
        (minimum, maximum): (i128, i128),
    ) -> Option<CheckedBooleanExpression> {
        let (carrier_minimum, carrier_maximum) = carrier_bounds(primitive)?;
        let leaf = || CheckedScalarExpression::StructuralParameterField {
            parameter_position: self.parameter_position,
            path: path.clone(),
            primitive_type: primitive,
        };
        let literal = |value: i128| {
            crate::land_integer_value(&BigInt::from_i128(value), primitive)
                .map(|literal| CheckedScalarExpression::IntegerLiteral { literal })
        };
        let lower = (minimum > carrier_minimum).then(|| {
            Some(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::LessOrEqual,
                left: Box::new(literal(minimum)?),
                right: Box::new(leaf()),
            })
        });
        let upper = (maximum < carrier_maximum).then(|| {
            Some(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::LessOrEqual,
                left: Box::new(leaf()),
                right: Box::new(literal(maximum)?),
            })
        });
        match (lower, upper) {
            (Some(lower), Some(upper)) => Some(CheckedBooleanExpression::And {
                left: Box::new(lower?),
                right: Box::new(upper?),
            }),
            (Some(bound), None) | (None, Some(bound)) => bound,
            (None, None) => None,
        }
    }

    fn push(&mut self, predicate: CheckedBooleanExpression) {
        self.rows.push(DeclarationPredicateRow {
            parameter_position: self.parameter_position,
            restored_on_return: self.restored_on_return,
            predicate,
        });
    }
}

/// The integer carrier a collection consequence bounds: an array field's
/// element or a scalar field itself.
fn field_consequence_primitive(program: &TypedTrees, field: &DataField) -> Option<PrimitiveType> {
    let carrier = peel_constrained(program, field.type_reference);
    let leaf = match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
        _ => carrier,
    };
    let primitive = program.primitive_type_reference(leaf)?;
    carrier_bounds(primitive).map(|_| primitive)
}

fn carrier_bounds(primitive: PrimitiveType) -> Option<(i128, i128)> {
    Some(match primitive {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 => (0, i128::from(u64::MAX)),
        _ => return None,
    })
}

fn peel_constrained(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}

/// The nominal data definition a field or parameter carrier owns inline.
/// A `&T`/`&mut T` field is borrowed storage whose referee's facts do not
/// ride on this storage.
fn owned_data_definition(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    match program
        .type_reference_table
        .type_reference(peel_constrained(program, type_reference))
    {
        TypeReferenceNode::Named { symbol, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol),
        _ => None,
    }
}

/// One data definition's relevant field, by retained symbol or, when none
/// was retained, by authored spelling. An erased field has no storage a
/// requirement path could observe.
fn find_data_field<'program>(
    program: &'program TypedTrees,
    definition: &DataDefinition,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        if field.relevance.is_erased() {
            return None;
        }
        if symbol.is_valid() {
            (field.symbol == symbol).then_some(field)
        } else {
            (field.name.as_str() == name).then_some(field)
        }
    })
}

/// Spell one authored fact of `definition` over the leaves at `prefix` below
/// one parameter. Operator-overloaded spellings, arithmetic, case paths,
/// indexed reads and borrowed fields are outside the vocabulary.
struct FactSpelling<'a> {
    program: &'a TypedTrees,
    operators: &'a CheckedOperatorFacts,
    parameter_position: u32,
    prefix: &'a [CheckedStructuralPredicatePathSegment],
    definition: &'a DataDefinition,
}

impl FactSpelling<'_> {
    /// Whether an authored operator application selected anything but its
    /// builtin meaning.
    fn selects_declared_operator(&self, expression: ExpressionHandle) -> bool {
        self.operators.uses.iter().any(|(_, operator)| {
            operator.expression == expression
                && operator.status
                    != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        })
    }

    fn boolean(&self, expression: ExpressionHandle) -> Option<CheckedBooleanExpression> {
        if self.selects_declared_operator(expression) {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => Some(
                CheckedBooleanExpression::Not(Box::new(self.boolean(unary.operand)?)),
            ),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => Some(CheckedBooleanExpression::And {
                    left: Box::new(self.boolean(binary.left)?),
                    right: Box::new(self.boolean(binary.right)?),
                }),
                BinaryOperator::Or => Some(CheckedBooleanExpression::Or {
                    left: Box::new(self.boolean(binary.left)?),
                    right: Box::new(self.boolean(binary.right)?),
                }),
                BinaryOperator::Less => {
                    self.comparison(expression, Order::Less, binary.left, binary.right)
                }
                BinaryOperator::LessOrEqual => {
                    self.comparison(expression, Order::LessOrEqual, binary.left, binary.right)
                }
                BinaryOperator::Greater => {
                    self.comparison(expression, Order::Less, binary.right, binary.left)
                }
                BinaryOperator::GreaterOrEqual => {
                    self.comparison(expression, Order::LessOrEqual, binary.right, binary.left)
                }
                BinaryOperator::Equal | BinaryOperator::NotEqual => {
                    let negated = binary.operator == BinaryOperator::NotEqual;
                    if let Some(comparison) = self.comparison(
                        expression,
                        Order::Equal { negated },
                        binary.left,
                        binary.right,
                    ) {
                        return Some(comparison);
                    }
                    let equal = CheckedBooleanExpression::Equal {
                        left: Box::new(self.boolean(binary.left)?),
                        right: Box::new(self.boolean(binary.right)?),
                    };
                    Some(if negated {
                        CheckedBooleanExpression::Not(Box::new(equal))
                    } else {
                        equal
                    })
                }
                _ => None,
            },
            _ => {
                // A bare Boolean field leaf (`where initialized,`) observes
                // the field at the same path.
                let (path, leaf_type) = self.field_path(expression)?;
                (self.program.primitive_type_reference(leaf_type) == Some(PrimitiveType::Bool))
                    .then_some(CheckedBooleanExpression::StructuralParameterField {
                        parameter_position: self.parameter_position,
                        path,
                    })
            }
        }
    }

    /// One ordered comparison: both operands share an integer carrier, or
    /// both are IEEE values of one format whose authored relation carries
    /// builtin or sealed IEEE meaning in the declaration's scope.
    fn comparison(
        &self,
        authored: ExpressionHandle,
        order: Order,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        let (left_authored, right_authored) = (left, right);
        let (left, left_type) = self.scalar(left)?;
        let (right, right_type) = self.scalar(right)?;
        let operand_type = match (left_type, right_type) {
            (Some(left_type), Some(right_type)) if left_type == right_type => left_type,
            (Some(operand_type), None) | (None, Some(operand_type)) => operand_type,
            _ => return None,
        };
        if matches!(operand_type, PrimitiveType::F32 | PrimitiveType::F64) {
            let ExpressionNode::Binary(binary) = self.program.expression_table.expression(authored)
            else {
                return None;
            };
            let operand_types = [
                self.operand_type_reference(binary.left),
                self.operand_type_reference(binary.right),
            ];
            if !crate::has_builtin_or_sealed_float_comparison_meaning(
                self.program,
                self.definition.symbol,
                authored,
                binary.operator,
                &operand_types,
            ) {
                return None;
            }
            let kind = match order {
                Order::Less => CheckedIeeeFloatComparisonKind::LessThan,
                Order::LessOrEqual => CheckedIeeeFloatComparisonKind::LessOrEqual,
                Order::Equal { negated: false } => CheckedIeeeFloatComparisonKind::Equal,
                Order::Equal { negated: true } => CheckedIeeeFloatComparisonKind::NotEqual,
            };
            return Some(CheckedBooleanExpression::ScalarIeeeFloatComparison {
                kind,
                left: Box::new(self.land_float_operand(left_authored, left, operand_type)?),
                right: Box::new(self.land_float_operand(right_authored, right, operand_type)?),
            });
        }
        carrier_bounds(operand_type)?;
        let (kind, negated) = match order {
            Order::Less => (CheckedIntegerComparisonKind::LessThan, false),
            Order::LessOrEqual => (CheckedIntegerComparisonKind::LessOrEqual, false),
            Order::Equal { negated } => (CheckedIntegerComparisonKind::Equal, negated),
        };
        let comparison = CheckedBooleanExpression::IntegerComparison {
            kind,
            left: Box::new(land_integer_operand(left, operand_type)?),
            right: Box::new(land_integer_operand(right, operand_type)?),
        };
        Some(if negated {
            CheckedBooleanExpression::Not(Box::new(comparison))
        } else {
            comparison
        })
    }

    /// An IEEE operand at the comparison's format: a field leaf of exactly
    /// that format, or a literal rounded directly to it from its authored
    /// spelling. A literal landed at another format refuses.
    fn land_float_operand(
        &self,
        authored: ExpressionHandle,
        operand: CheckedScalarExpression,
        primitive: PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        let format = match primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        match (&operand, self.program.expression_table.expression(authored)) {
            (CheckedScalarExpression::IeeeFloatLiteral { .. }, ExpressionNode::Float(literal)) => {
                if literal.landing().is_some_and(|landing| landing != format) {
                    return None;
                }
                Some(CheckedScalarExpression::IeeeFloatLiteral {
                    value: ieee_literal_bits(literal, format),
                })
            }
            (CheckedScalarExpression::StructuralParameterField { primitive_type, .. }, _) => {
                (*primitive_type == primitive).then_some(operand)
            }
            _ => None,
        }
    }

    /// The declared type of a field-leaf operand; literals have none.
    fn operand_type_reference(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        self.field_path(expression)
            .map(|(_, type_reference)| type_reference)
    }

    /// A field leaf below the parameter, or a literal. The returned carrier
    /// is the operand's own; an unlanded literal contributes `None` so the
    /// comparison's other side lands it.
    fn scalar(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(CheckedScalarExpression, Option<PrimitiveType>)> {
        if self.selects_declared_operator(expression) {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => {
                let primitive = literal.landing().and_then(|landing| {
                    use numerics::literals::LandedIntegerType;
                    match landing.landed_type {
                        LandedIntegerType::I8 => Some(PrimitiveType::I8),
                        LandedIntegerType::I16 => Some(PrimitiveType::I16),
                        LandedIntegerType::I32 => Some(PrimitiveType::I32),
                        LandedIntegerType::I64 => Some(PrimitiveType::I64),
                        LandedIntegerType::U8 => Some(PrimitiveType::U8),
                        LandedIntegerType::U16 => Some(PrimitiveType::U16),
                        LandedIntegerType::U32 => Some(PrimitiveType::U32),
                        LandedIntegerType::U64 => Some(PrimitiveType::U64),
                        LandedIntegerType::Addr => None,
                    }
                });
                Some((
                    CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    },
                    primitive,
                ))
            }
            ExpressionNode::Float(literal) => {
                // An unlanded literal rounds directly to the other operand's
                // format when the comparison lands it (`land_float_operand`).
                let format = literal
                    .landing()
                    .unwrap_or(numerics::literals::FloatFormat::F64);
                let primitive = literal.landing().map(|format| match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                });
                Some((
                    CheckedScalarExpression::IeeeFloatLiteral {
                        value: ieee_literal_bits(literal, format),
                    },
                    primitive,
                ))
            }
            _ => {
                let (path, leaf_type) = self.field_path(expression)?;
                let primitive = self.program.primitive_type_reference(leaf_type)?;
                if primitive == PrimitiveType::Bool || primitive == PrimitiveType::Addr {
                    return None;
                }
                Some((
                    CheckedScalarExpression::StructuralParameterField {
                        parameter_position: self.parameter_position,
                        path,
                        primitive_type: primitive,
                    },
                    Some(primitive),
                ))
            }
        }
    }

    /// Resolve a fact leaf (`value`, `inner.value`) to its path below the
    /// prefix and the leaf's declared type. Only plain record-field chains
    /// have a requirement spelling.
    fn field_path(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(
        Vec<CheckedStructuralPredicatePathSegment>,
        TypeReferenceHandle,
    )> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(name.members);
                let member_symbols = self
                    .program
                    .expression_table
                    .name_path_member_symbols(name.member_symbols);
                let mut path = self.prefix.to_vec();
                let mut owner = self.definition;
                let mut leaf = None;
                for (index, member) in members.iter().enumerate() {
                    let symbol = member_symbols
                        .get(index)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid);
                    let field = find_data_field(self.program, owner, symbol, member.as_str())?;
                    path.push(CheckedStructuralPredicatePathSegment::Field(
                        field.path_identity(),
                    ));
                    if index + 1 == members.len() {
                        leaf = Some(field.type_reference);
                    } else {
                        owner = owned_data_definition(self.program, field.type_reference)?;
                    }
                }
                Some((path, leaf?))
            }
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return None;
                }
                let (mut path, receiver_type) = self.field_path(member.receiver)?;
                let owner = owned_data_definition(self.program, receiver_type)?;
                let field = find_data_field(
                    self.program,
                    owner,
                    member.member_symbol,
                    member.member.as_str(),
                )?;
                path.push(CheckedStructuralPredicatePathSegment::Field(
                    field.path_identity(),
                ));
                Some((path, field.type_reference))
            }
            ExpressionNode::Borrow(borrow) => self.field_path(borrow.target),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Order {
    Less,
    LessOrEqual,
    Equal { negated: bool },
}

/// Land an unlanded literal at the comparison's common integer carrier; an
/// out-of-range literal refuses rather than truncating.
fn land_integer_operand(
    expression: CheckedScalarExpression,
    primitive: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    match expression {
        CheckedScalarExpression::IntegerLiteral { literal } if literal.landing().is_none() => {
            let value = literal
                .value_i64()
                .map(BigInt::from_i64)
                .or_else(|| literal.value_u64().map(BigInt::from_u64))?;
            // Landing checks the value against the carrier; the authored
            // spelling keeps its text.
            let landed = crate::land_integer_value(&value, primitive)?;
            Some(CheckedScalarExpression::IntegerLiteral {
                literal: literal.with_landing(landed.landing()?),
            })
        }
        CheckedScalarExpression::IeeeFloatLiteral { .. } => None,
        other => Some(other),
    }
}

/// The exact interchange bits of a literal's spelling, rounded directly to
/// `format` (never through the other format).
fn ieee_literal_bits(
    literal: &numerics::literals::FloatLiteral,
    format: numerics::literals::FloatFormat,
) -> semantic_vocabulary::IeeeFloatValue {
    match format {
        numerics::literals::FloatFormat::F32 => {
            semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
        }
        numerics::literals::FloatFormat::F64 => {
            semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
        }
    }
}

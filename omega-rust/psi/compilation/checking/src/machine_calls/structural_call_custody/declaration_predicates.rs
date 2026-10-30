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
//! Zero-excluding definitions carry the same obligations once established;
//! zero provisioning remains a separate construction judgment. A conservative
//! declaration flag cannot erase an incoming value's default domain or refuse
//! its replacement. Static subjects read the exact retained argument tuple;
//! zero qualification and Terminal spelling share that readonly join.
//! The authored template is never rewritten for a caller. Descent continues
//! beneath definitions, and through literal-length arrays at each exact index.

use super::super::super::proof_contracts::collection_predicates;
use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerComparisonKind,
    CheckedOperatorFacts, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use language_core::ReferenceAccess;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Recursion belongs to the exact retained application, not merely its
/// generic declaration. Wrapper<Wrapper<u64>> owns two different finite homes.
#[derive(Clone, Copy, PartialEq, Eq)]
struct StorageDefinitionIdentity {
    symbol: SymbolHandle,
    application: Option<TypeReferenceHandle>,
}

impl StorageDefinitionIdentity {
    fn of(definition: &DataDefinition) -> Self {
        Self {
            symbol: definition.symbol,
            application: definition.generic_instance,
        }
    }
}

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

/// The roster of one machine state, in parameter order, then the
/// depth-first declaration order of the storage each parameter reaches.
pub fn state_declaration_predicates(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
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
            None => false,
        };
        let mut emitter = RowEmitter {
            program,
            operators,
            parameter_position,
            restored_on_return,
            case_guards: Vec::new(),
            rows: &mut rows,
        };
        match parameter_data_definition(program, referee) {
            // A lifetime-only application names the declaration itself and
            // owns its storage exactly as its plain name does.
            Some(definition) => {
                let mut visited = vec![StorageDefinitionIdentity::of(definition)];
                emitter.definition_storage(&mut Vec::new(), definition, &mut visited);
            }
            None => emitter.descend(&mut Vec::new(), referee, &mut Vec::new()),
        }
    }
    rows
}

/// The data declaration a parameter's referee names when the ordinary field
/// resolution cannot: the declaration a lifetime-only application
/// instantiates.
fn parameter_data_definition<'program>(
    program: &'program dyn crate::CheckingInput,
    referee: TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    let TypeReferenceNode::Generic {
        base_symbol: owner,
        arguments,
        ..
    } = program.type_reference_table.type_reference(
        program
            .type_reference_table
            .unconstrained_type_reference(referee),
    )
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *owner && definition.generic_instance.is_none())
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
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
) -> Vec<DeclarationPredicateRow> {
    program
        .machine_states(machine)
        .first()
        .map(|entry| state_declaration_predicates(program, operators, entry))
        .unwrap_or_default()
}

/// Whether every own and reachable owned `where` fact has complete roster
/// evidence. A store of an unrelated common field cannot skip this recursive
/// fence merely because its destination path never enters a case payload.
pub fn declaration_facts_carried(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
) -> bool {
    complete_definition_storage(program, operators, definition, &mut Vec::new())
}

fn complete_definition_storage(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
    visited: &mut Vec<StorageDefinitionIdentity>,
) -> bool {
    // Emission cannot revisit an inline recursive owner. Borrowed referents
    // and empty arrays are excluded by the same storage traversal below.
    if visited.contains(&StorageDefinitionIdentity::of(definition))
        || !definition_own_facts_carried(program, operators, definition)
    {
        return false;
    }
    visited.push(StorageDefinitionIdentity::of(definition));
    let complete = owned_storage_fields(program, definition).all(|(_, field)| {
        complete_owned_storage(program, operators, field.type_reference, visited)
    });
    visited.pop();
    complete
}

fn complete_owned_storage(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    reference: TypeReferenceHandle,
    visited: &mut Vec<StorageDefinitionIdentity>,
) -> bool {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(0),
            ..
        } => true,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => complete_owned_storage(program, operators, *element_type, visited),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            // The roster can enumerate only an exact retained literal extent.
            // Never publish a partial element predicate list for another form.
            !storage_carries_rows(program, *element_type, &mut Vec::new())
        }
        TypeReferenceNode::Reference { .. } => true,
        _ => owned_data_definition(program, carrier).is_none_or(|definition| {
            complete_definition_storage(program, operators, definition, visited)
        }),
    }
}

fn definition_own_facts_carried(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
) -> bool {
    if definition.where_facts.is_empty()
        || crate::data_where_field_intervals(program, definition).is_some()
    {
        return true;
    }
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .all(|fact| {
            let Some(expression) = fact.as_expression() else {
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
    program: &'program dyn crate::CheckingInput,
    operators: &'program CheckedOperatorFacts,
    parameter_position: u32,
    restored_on_return: bool,
    case_guards: Vec<CheckedBooleanExpression>,
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
        visited: &mut Vec<StorageDefinitionIdentity>,
    ) {
        let carrier = self
            .program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        match self.program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                // Visiting every element of an array whose elements reach
                // no row-emitting definition emits nothing; skip it whole.
                if !storage_carries_rows(self.program, *element_type, visited) {
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
                if visited.contains(&StorageDefinitionIdentity::of(definition)) {
                    return;
                }
                visited.push(StorageDefinitionIdentity::of(definition));
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
        visited: &mut Vec<StorageDefinitionIdentity>,
    ) {
        self.definition_rows(prefix, definition);
        for (case, field) in owned_storage_fields(self.program, definition) {
            if let Some(case) = case {
                let identity = checked_trees::CheckedMemberIdentity::of_member(
                    case.identity,
                    case.name.clone(),
                );
                self.case_guards
                    .push(CheckedBooleanExpression::StructuralCaseMembership {
                        subject: checked_trees::CheckedStructuralCaseSubject {
                            root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                position: self.parameter_position,
                            },
                            path: prefix.clone(),
                        },
                        case: identity.clone(),
                    });
                prefix.push(CheckedStructuralPredicatePathSegment::Case(identity));
            }
            prefix.push(CheckedStructuralPredicatePathSegment::Field(
                checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
            ));
            self.descend(prefix, field.type_reference, visited);
            prefix.pop();
            if case.is_some() {
                prefix.pop();
                self.case_guards.pop();
            }
        }
    }

    fn definition_rows(
        &mut self,
        prefix: &[CheckedStructuralPredicatePathSegment],
        definition: &DataDefinition,
    ) {
        if !definition_emits_rows(self.program, definition) {
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
            let Some(expression) = fact.as_expression() else {
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
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
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
            observation: Default::default(),
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
        // Outer tag guards must form before any inner tag or payload path.
        // Inactive alternatives contribute true without observing a payload.
        let predicate = self
            .case_guards
            .iter()
            .rev()
            .fold(predicate, |predicate, guard| CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Not(Box::new(guard.clone()))),
                right: Box::new(predicate),
            });
        self.rows.push(DeclarationPredicateRow {
            parameter_position: self.parameter_position,
            restored_on_return: self.restored_on_return,
            predicate,
        });
    }
}

/// The integer carrier a collection consequence bounds: an array field's
/// element or a scalar field itself.
fn field_consequence_primitive(
    program: &dyn crate::CheckingInput,
    field: &DataField,
) -> Option<PrimitiveType> {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(field.type_reference);
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

/// Whether `definition` contributes rows of its own: it has facts that are
/// not interval-only (those are carried by its fields' carriers). Zero
/// provisioning does not change the obligations of an established value.
fn definition_emits_rows(program: &dyn crate::CheckingInput, definition: &DataDefinition) -> bool {
    !(definition.where_facts.is_empty()
        || crate::data_where_field_intervals(program, definition).is_some())
}

/// The relevant fields whose storage `definition` owns inline. A borrowed
/// field's referee is not this storage, and an erased field has none.
fn owned_storage_fields<'program>(
    program: &'program dyn crate::CheckingInput,
    definition: &'program DataDefinition,
) -> impl Iterator<
    Item = (
        Option<&'program typed_trees::data::DataVariant>,
        &'program DataField,
    ),
> {
    program
        .data_members(definition)
        .iter()
        .flat_map(move |member| {
            let (case, fields) = match member {
                DataMember::Field(field) => (None, std::slice::from_ref(field)),
                DataMember::Variant(case) => (Some(case), program.data_payload_fields(case)),
            };
            fields.iter().map(move |field| (case, field))
        })
        .filter(move |(_, field)| {
            field.symbol.is_valid()
                && !field.relevance.is_erased()
                && !matches!(
                    program.type_reference_table.type_reference(
                        program
                            .type_reference_table
                            .unconstrained_type_reference(field.type_reference)
                    ),
                    TypeReferenceNode::Reference { .. }
                )
        })
}

/// Whether the storage of `type_reference` reaches a definition that emits
/// rows, by the same descent `RowEmitter::descend` takes, without visiting
/// array elements one by one. `visited` is the definition stack on this path.
fn storage_carries_rows(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    visited: &mut Vec<StorageDefinitionIdentity>,
) -> bool {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(type_reference);
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(0),
            ..
        } => false,
        TypeReferenceNode::FixedArray { element_type, .. } => {
            // An unresolved extent may still own elements. Detection must
            // retain their requirements even though emission cannot enumerate
            // them yet; recursive eligibility then refuses that partial roster.
            storage_carries_rows(program, *element_type, visited)
        }
        _ => {
            let Some(definition) = owned_data_definition(program, carrier) else {
                return false;
            };
            if visited.contains(&StorageDefinitionIdentity::of(definition)) {
                return false;
            }
            if definition_emits_rows(program, definition) {
                return true;
            }
            visited.push(StorageDefinitionIdentity::of(definition));
            let carries = owned_storage_fields(program, definition)
                .any(|(_, field)| storage_carries_rows(program, field.type_reference, visited));
            visited.pop();
            carries
        }
    }
}

/// The nominal data definition a field or parameter carrier owns inline.
/// A `&T`/`&mut T` field is borrowed storage whose referee's facts do not
/// ride on this storage.
fn owned_data_definition(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(type_reference);
    match program.type_reference_table.type_reference(carrier) {
        // Array traversal revisits these owners at each exact index. Reuse the
        // program's revision-checked first-match index instead of rescanning
        // every declaration; predicate enumeration and path order stay intact.
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            program.closed_data_definition(carrier)
        }
        _ => None,
    }
}

/// One data definition's relevant field, by retained symbol or, when none
/// was retained, by authored spelling. An erased field has no storage a
/// requirement path could observe.
fn find_data_field<'program>(
    program: &'program dyn crate::CheckingInput,
    definition: &DataDefinition,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    program.data_members(definition).iter().find_map(|member| {
        let field = member.as_field()?;
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

/// Spell one complete authored fact over exact parameter storage paths.
/// Tag tests and payload paths retain their selected case identity. An ordinary
/// predicate relation uses the shared independent equivalence judgment; its
/// necessary consequence alone is insufficient. Unsupported arithmetic,
/// indexed reads and borrowed fields remain outside this vocabulary.
struct FactSpelling<'a> {
    program: &'a dyn crate::CheckingInput,
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
        // The shared checked predicate owner proves BOTH directions over the
        // full formal carriers. A necessary consequence cannot replace a row.
        if let Some(consequence) = collection_predicates::exact_predicate_ordering(
            self.program,
            self.definition.symbol,
            expression,
        ) {
            return self.exact_ordering(consequence);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => Some(
                CheckedBooleanExpression::Not(Box::new(self.boolean(unary.operand)?)),
            ),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::CaseMembership | BinaryOperator::Equal
                    if crate::has_exact_data_case_membership_meaning(
                        self.program,
                        self.definition,
                        expression,
                        binary,
                    ) =>
                {
                    let (path, reference) = self.field_path(binary.left)?;
                    let selected = self
                        .program
                        .expression_table
                        .name_path_node(binary.right)?
                        .symbol;
                    let case = self
                        .program
                        .closed_data_constructor_case(reference, selected)?;
                    Some(CheckedBooleanExpression::StructuralCaseMembership {
                        subject: checked_trees::CheckedStructuralCaseSubject {
                            root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                position: self.parameter_position,
                            },
                            path,
                        },
                        case: checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    })
                }
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
                        observation: Default::default(),
                        parameter_position: self.parameter_position,
                        path,
                    })
            }
        }
    }

    fn exact_ordering(
        &self,
        consequence: collection_predicates::PredicateOrderingConsequence,
    ) -> Option<CheckedBooleanExpression> {
        let operand = |operand| match operand {
            collection_predicates::PredicateIntegerOperand::Argument(subject)
                if subject.projection.is_empty() =>
            {
                self.scalar(subject.expression)
            }
            collection_predicates::PredicateIntegerOperand::Integer(value) => {
                let literal = numerics::literals::IntegerLiteral::from_parts(
                    value < 0,
                    numerics::literals::IntegerRadix::Decimal,
                    &value.unsigned_abs().to_string(),
                )
                .ok()?;
                Some((CheckedScalarExpression::IntegerLiteral { literal }, None))
            }
            _ => None,
        };
        let (left, left_type) = operand(consequence.left)?;
        let (right, right_type) = operand(consequence.right)?;
        let primitive = match (left_type, right_type) {
            (Some(left), Some(right)) if left == right => left,
            (Some(primitive), None) | (None, Some(primitive)) => primitive,
            _ => return None,
        };
        carrier_bounds(primitive)?;
        let (left, right, kind, negated) = match consequence.operator {
            BinaryOperator::Equal => (left, right, CheckedIntegerComparisonKind::Equal, false),
            BinaryOperator::NotEqual => (left, right, CheckedIntegerComparisonKind::Equal, true),
            BinaryOperator::Less => (left, right, CheckedIntegerComparisonKind::LessThan, false),
            BinaryOperator::LessOrEqual => (
                left,
                right,
                CheckedIntegerComparisonKind::LessOrEqual,
                false,
            ),
            BinaryOperator::Greater => (right, left, CheckedIntegerComparisonKind::LessThan, false),
            BinaryOperator::GreaterOrEqual => (
                right,
                left,
                CheckedIntegerComparisonKind::LessOrEqual,
                false,
            ),
            _ => return None,
        };
        let comparison = CheckedBooleanExpression::IntegerComparison {
            kind,
            left: Box::new(land_integer_operand(left, primitive)?),
            right: Box::new(land_integer_operand(right, primitive)?),
        };
        Some(if negated {
            CheckedBooleanExpression::Not(Box::new(comparison))
        } else {
            comparison
        })
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
            let Some(binary) = self.program.expression_table.binary_node(authored) else {
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
                    value: typed_trees::expression::ieee_float_value(literal, format),
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
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && self.program.symbols.get(path.symbol).kind
                        == symbols::SymbolKind::TypeParameter =>
            {
                // Static declaration subjects have no storage path. Rejoin
                // their exact applied value with its declared carrier, just
                // as zero qualification does, without rewriting the template.
                let (primitive, value) =
                    crate::proof_contracts::default_domains::applied_const_integer(
                        self.program,
                        self.definition,
                        path.symbol,
                    )?;
                let literal = numerics::literals::IntegerLiteral::from_parts(
                    value < 0,
                    numerics::literals::IntegerRadix::Decimal,
                    &value.unsigned_abs().to_string(),
                )
                .ok()?;
                let landed = crate::land_integer_value(&BigInt::from_i128(value), primitive)?;
                Some((
                    CheckedScalarExpression::IntegerLiteral {
                        literal: literal.with_landing(landed.landing()?),
                    },
                    Some(primitive),
                ))
            }
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
                        value: typed_trees::expression::ieee_float_value(literal, format),
                    },
                    primitive,
                ))
            }
            _ => {
                // The same closed-value judgment used by immutable bounds
                // owns selected operators, arithmetic policy and landing.
                // Carry its exact result and carrier, rather than requiring
                // a predicate's closed operand to be a literal token.
                if let Some(closed) = self
                    .program
                    .closed_integer_value_in(expression, self.definition.symbol)
                {
                    let spelling = closed.value.to_string();
                    let literal = numerics::literals::IntegerLiteral::from_parts(
                        spelling.starts_with('-'),
                        numerics::literals::IntegerRadix::Decimal,
                        spelling.trim_start_matches('-'),
                    )
                    .ok()?;
                    let literal = if let Some(primitive) = closed.primitive {
                        if !primitive.is_fixed_integer() {
                            return None;
                        }
                        let landed = crate::land_integer_value(&closed.value, primitive)?;
                        literal.with_landing(landed.landing()?)
                    } else {
                        literal
                    };
                    return Some((
                        CheckedScalarExpression::IntegerLiteral { literal },
                        closed.primitive,
                    ));
                }
                let (path, leaf_type) = self.field_path(expression)?;
                let primitive = self.program.primitive_type_reference(leaf_type)?;
                if primitive == PrimitiveType::Bool || primitive == PrimitiveType::Addr {
                    return None;
                }
                Some((
                    CheckedScalarExpression::StructuralParameterField {
                        observation: Default::default(),
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
                if self.definition.predicate_subject_symbol.is_valid()
                    && name.symbol == self.definition.predicate_subject_symbol
                    && name.head_symbol == name.symbol
                    && self
                        .program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        == [name.symbol]
                {
                    return Some((self.prefix.to_vec(), self.definition.predicate_subject_type));
                }
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
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
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
                let (mut path, receiver_type) = self.field_path(member.receiver)?;
                let owner = owned_data_definition(self.program, receiver_type)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    self.program,
                    owner,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|case| case.as_str()),
                )?;
                if !field.symbol.is_valid() {
                    return None;
                }
                if let Some(case) = self
                    .program
                    .data_members(owner)
                    .iter()
                    .filter_map(|row| row.as_variant())
                    .find(|case| {
                        self.program
                            .data_payload_fields(case)
                            .iter()
                            .any(|row| row.symbol == field.symbol)
                    })
                {
                    path.push(CheckedStructuralPredicatePathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    ));
                }
                path.push(CheckedStructuralPredicatePathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
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

#[cfg(test)]
mod tests;

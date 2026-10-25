//! Element bounds a cited helper theorem delivers to the caller's flow.
//!
//! A statement call cites a checked theorem: once its `requires` hold under
//! the exact argument substitution, its `ensures` conclusions enter the
//! caller's facts ([citation](../../../../../../../wiki/spec/proofs/contracts.md#citation-and-induction)).
//! This adapter imports the conclusions arithmetic formation can use: a
//! builtin comparison over an element `cells[index]` of a collection parameter.
//! The selector substitutes through the shared ordered-value algebra; copied
//! immutable values survive unknown writes, and other actuals require the
//! complete call frame to preserve their observation. A mutable reference
//! parameter is the post-state place of its argument, so an element comparison
//! substitutes that referent; a mutable scalar formal is still not the
//! caller's pre-call value. Literal indices are not
//! a separate admission path. Static coordinate labels supply interval and
//! invalidation metadata beside the symbol-owned ordered relation.
//! Builtin windows use the shared canonical storage projection: the second
//! element of `board[2..4]` is `board[3]`, not another array dimension or a
//! same-spelled unrelated view. An unresolved coordinate imports no bound.
//!
//! Soundness rests on the ordinary obligations, not on this adapter: the
//! callee's `ensures` is proved at each of its exits and the call's
//! `requires` at this call site, each in the same compilation, so a program
//! that compiles cites only proved facts. The facts live in the caller's
//! value environment and lapse with it: any write overlapping the array
//! place, and any unknown write, drops them.

use super::guard_narrowing::projected_facts::{ProjectedOperand, contribute_projected_comparison};
use super::ordered_values::{self, Operand};
use super::{BinaryOperator, ExpressionHandle, Machine, State, ValueEnvironment, place_path};
use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;

/// `collection[index]` with a literal, non-negative `index` over a place
/// collection, spelled as the value environment keys it.
pub(crate) fn element_place_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    let indexed = program.expression_table.indexed_node(expression)?;
    let index = program
        .expression_table
        .constant_integer_value(indexed.index)?;
    let index = u64::try_from(index).ok()?;
    Some(format!(
        "{}[{index}]",
        place_path(program, indexed.collection)?
    ))
}

/// `collection[index]` whose value cannot change during the activation, keyed
/// as `element_place_path` keys a literal index: the collection is a shared
/// loan the state never reseats (the loan freezes its bytes), and the index
/// is a literal or an immutable scalar parameter. Every read of that spelling
/// is then the same value, so a guard's facts about one read bound the others
/// (`text[index] < 48 || text[index] > 57 -> ..` refuted gives the later
/// `text[index] - 48` its range).
pub(crate) fn stable_element_place_path(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<String> {
    let indexed = program.expression_table.indexed_node(expression)?;
    if let Some(element_index) = program
        .closed_integer_expression_value_in(indexed.index, machine.symbol)
        .and_then(|value| value.to_u64())
        .and_then(|value| usize::try_from(value).ok())
    {
        return crate::value_custody::places::canonical_collection_element_place(
            program,
            machine,
            state,
            indexed.collection,
            element_index,
        )
        .map(|place| place.label(program));
    }
    let state = state?;
    let parameter = |operand: ExpressionHandle| {
        let name = program.expression_table.name_path_node(operand)?;
        if !name.symbol.is_valid() || name.symbol != name.head_symbol {
            return None;
        }
        program.state_parameter_by_symbol(state, name.symbol)
    };
    let collection = parameter(indexed.collection)?;
    let index = parameter(indexed.index)?;
    let frozen = matches!(
        program
            .type_reference_table
            .type_reference(collection.type_reference),
        typed_trees::types::TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Shared,
            ..
        }
    ) && crate::state_reference_parameter_binding_is_stable(
        program,
        machine,
        state,
        collection.symbol,
    );
    let fixed_index = !index.is_mutable
        && program
            .primitive_type_reference(index.type_reference)
            .is_some_and(|primitive| primitive.accepts_integer_literal());
    (frozen && fixed_index).then(|| {
        Some(format!(
            "{}[{}]",
            place_path(program, indexed.collection)?,
            place_path(program, indexed.index)?
        ))
    })?
}

/// Import the element bounds of a checked callee's `ensures` after a
/// statement call returns.
pub(crate) fn seed_cited_element_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    statement: &typed_trees::statement::StatementNode,
    frames: Option<&crate::CallFrameResolver<'_>>,
    environment: &mut ValueEnvironment,
) {
    let Some(call) = statement.as_call() else {
        return;
    };
    if !call.receiver.is_empty() || call.static_requirement_dispatch.is_some() {
        return;
    }
    let (target, static_arguments) = if call.authored_machine_application.is_valid() {
        let application = program
            .expression_table
            .static_machine_application(call.authored_machine_application);
        (
            application.registration_operation,
            program
                .expression_table
                .static_machine_application_arguments(call.authored_machine_application),
        )
    } else {
        (call.target_symbol, call.machine_arguments.as_ref())
    };
    let Some(callee) = checked_callee(program, target) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let arguments = program.statement_table.expression_handles(call.arguments);

    if parameters.len() != arguments.len() {
        return;
    }
    let argument_written =
        frames.and_then(|frames| frames.statement_value_may_write_paths(machine, statement));
    let callee_written = frames.and_then(|frames| frames.may_write_paths(machine, call));
    let written =
        argument_written
            .as_ref()
            .zip(callee_written.as_ref())
            .map(|(arguments, callee)| {
                let mut written = arguments.clone();
                written.extend(callee.iter().cloned());
                written
            });
    let reference_origins = arguments
        .iter()
        .map(|actual| {
            frames.and_then(|frames| {
                frames.structural_reference_origins_before_statement(machine, statement, *actual)
            })
        })
        .collect::<Vec<_>>();
    let substitution = Substitution {
        program,
        machine,
        state,
        parameters,
        arguments,
        written: written.as_deref(),
        static_arguments,
        argument_written: argument_written.as_deref(),
        reference_origins: &reference_origins,
    };
    for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
        program,
        program.machine_contracts(callee),
    ) {
        seed_conjunct(
            program,
            callee,
            entry,
            &substitution,
            expression,
            environment,
        );
    }
}

/// A free machine whose checked body proves its own `ensures`.
fn checked_callee(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<&Machine> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.attached_data.is_none()
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present
            && (machine.symbol == symbol
                || program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == symbol))
    })
}

struct Substitution<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: Option<&'program State>,
    parameters: &'program [typed_trees::signature::StateParameter],
    arguments: &'program [ExpressionHandle],
    written: Option<&'program [String]>,
    static_arguments: &'program [typed_trees::expression::StaticMachineArgument],
    argument_written: Option<&'program [String]>,
    reference_origins: &'program [Option<Vec<(symbols::SymbolHandle, Vec<facts::PlaceSegment>)>>],
}

impl Substitution<'_> {
    /// The actual argument bound to the formal a callee-side name reads.
    fn actual(&self, formal: ExpressionHandle) -> Option<ExpressionHandle> {
        let path = self.program.expression_table.name_path_node(formal)?;
        if !path.symbol.is_valid()
            || self
                .program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let position =
            typed_trees::signature::state_parameter_position(self.parameters, path.symbol)?;
        let parameter = &self.parameters[position];
        // A mutable scalar formal's post-state is not the caller's argument
        // value. A mutable reference formal is that argument's place, so an
        // element fact about it substitutes through.
        let reference = matches!(
            self.program
                .type_reference_table
                .type_reference(parameter.type_reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        );
        if parameter.is_mutable && !reference {
            return None;
        }
        self.arguments.get(position).copied()
    }

    /// Project ordinary and static integer subjects through one simultaneous
    /// snapshot roster. No unbound callee subject escapes into caller facts.
    fn scalar(
        &self,
        callee: &Machine,
        entry: &State,
        expression: ExpressionHandle,
    ) -> Option<ProjectedOperand> {
        let state = self.state?;
        let mut bindings = Vec::new();
        let mut reference_formals = Vec::new();
        for (position, (parameter, actual)) in
            self.parameters.iter().zip(self.arguments).enumerate()
        {
            if crate::reference_shell(self.program, parameter.type_reference).is_some() {
                // The callee's post-state speaks about its evaluated referent,
                // not a descriptor the body has replaced with another loan.
                if !crate::state_reference_parameter_binding_is_stable(
                    self.program,
                    callee,
                    entry,
                    parameter.symbol,
                ) {
                    continue;
                }
                let Some(actual_identity) =
                    ordered_values::operand(self.program, self.machine, state, *actual, None)
                else {
                    continue;
                };
                if !self
                    .argument_written
                    .is_some_and(|written| actual_identity.survives(written))
                {
                    continue;
                }
                let Some(Some(origins)) = self.reference_origins.get(position) else {
                    continue;
                };
                let [(root, segments)] = origins.as_slice() else {
                    continue;
                };
                let Some(value) = captured_storage_operand(self.program, *root, segments) else {
                    continue;
                };
                // Prefix origins identify the evaluated loan. Operand effects
                // must preserve that observation; the callee's intentional
                // writes change its post-state contents, not this identity.
                if !self
                    .argument_written
                    .is_some_and(|written| value.survives(written))
                {
                    continue;
                }
                reference_formals.push(Operand::parameter(parameter));
                bindings.push((Operand::parameter(parameter), value));
                continue;
            }
            if parameter.is_mutable
                || !self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                continue;
            }
            let Some(value) =
                ordered_values::operand(self.program, self.machine, state, *actual, None)
            else {
                continue;
            };
            if !value.is_immutable_snapshot(self.program, state)
                && !self.written.is_some_and(|written| value.survives(written))
            {
                continue;
            }
            bindings.push((Operand::parameter(parameter), value));
        }
        let parameters = self.program.machine_type_parameters(callee);
        if parameters.len() != self.static_arguments.len() {
            return None;
        }
        for (parameter, argument) in parameters.iter().zip(self.static_arguments) {
            if !matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Value { .. }
                    | typed_trees::data::TypeParameterKind::Const { .. }
            ) {
                continue;
            }
            let value = self.static_operand(argument)?;
            if !value.is_immutable_snapshot(self.program, state)
                && !self.written.is_some_and(|written| value.survives(written))
            {
                continue;
            }
            bindings.push((
                Operand::Place {
                    root: parameter.symbol,
                    path: parameter.name.as_str().to_owned(),
                },
                value,
            ));
        }
        let source = ordered_values::operand(self.program, callee, entry, expression, None)?;
        fn reference_path(value: &Operand, references: &[Operand]) -> bool {
            references.contains(value)
                || match value {
                    Operand::Field { receiver, .. } => reference_path(receiver, references),
                    _ => false,
                }
        }
        fn fully_bound(
            value: &Operand,
            bindings: &[(Operand, Operand)],
            references: &[Operand],
        ) -> bool {
            match value {
                Operand::Integer(..) => true,
                Operand::Place { .. } => {
                    bindings.iter().any(|(formal, _)| formal == value)
                        && !references.contains(value)
                }
                Operand::Field {
                    receiver,
                    primitive,
                    ..
                } => {
                    primitive.is_some_and(|primitive| primitive.is_fixed_integer())
                        && reference_path(receiver, references)
                }
                Operand::Binary { operands, .. } => operands
                    .iter()
                    .all(|value| fully_bound(value, bindings, references)),
                _ => false,
            }
        }
        if !fully_bound(&source, &bindings, &reference_formals) {
            return None;
        }
        let values = source.rebound(&bindings);
        let [operand] = values.as_slice() else {
            return None;
        };
        // Every leaf has an exact simultaneous binding: copied scalar values
        // survived the complete frame, reference subjects name captured loans.
        // No unbound callee subject is admitted by matching display spelling.
        Some(ProjectedOperand {
            operand: Some(operand.clone()),
            spelled: operand.static_path(),
            declared_type: crate::declared_place_type_raw(
                self.program,
                callee,
                Some(entry),
                expression,
            ),
        })
    }

    fn static_operand(
        &self,
        argument: &typed_trees::expression::StaticMachineArgument,
    ) -> Option<Operand> {
        if let Some(literal) = &argument.const_literal {
            return Some(Operand::Integer(literal.clone(), literal.landing()));
        }
        if argument.type_reference.is_valid() {
            if let typed_trees::types::TypeReferenceNode::ConstExpression(expression) = self
                .program
                .type_reference_table
                .type_reference(argument.type_reference)
            {
                return ordered_values::operand(
                    self.program,
                    self.machine,
                    self.state?,
                    *expression,
                    None,
                );
            }
        }
        if argument.symbol.is_valid() {
            let state = self.state?;
            let parameter = self
                .program
                .state_parameter_by_symbol(state, argument.symbol);
            let local = self.program.state_local_declaration(state, argument.symbol);
            let static_parameter = self
                .program
                .machine_lexical_type_parameter(self.machine, argument.symbol)
                .is_some_and(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Value { .. }
                            | typed_trees::data::TypeParameterKind::Const { .. }
                    )
                });
            if parameter.is_none() && local.is_none() && !static_parameter {
                return None;
            }
            let reference = crate::value_custody::places::declaration_type_reference(
                self.program,
                argument.symbol,
            )?;
            if !self
                .program
                .primitive_type_reference(reference)
                .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                return None;
            }
            return Some(Operand::Place {
                root: argument.symbol,
                path: self.program.symbols.name(argument.symbol).to_owned(),
            });
        }
        let spelling = typed_trees::typed_trees::static_const_argument_spelling(
            self.program.typed_program(),
            argument,
        )?;
        let value = if let Some(canonical) =
            language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
        {
            match canonical.decode_encoding()? {
                language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    value,
                    ..
                } => value,
                _ => return None,
            }
        } else {
            spelling.parse::<i128>().ok()?
        };
        let literal = numerics::literals::IntegerLiteral::from_parts(
            value < 0,
            numerics::literals::IntegerRadix::Decimal,
            &value.unsigned_abs().to_string(),
        )
        .ok()?;
        Some(Operand::Integer(literal, None))
    }

    /// Project the complete selector under simultaneous scalar snapshots.
    /// A value argument denotes its evaluation-point value, not a mutable
    /// caller field after another argument or the callee has changed it.
    fn element(
        &self,
        callee: &Machine,
        entry: &State,
        formal: ExpressionHandle,
    ) -> Option<ProjectedOperand> {
        let indexed = self.program.expression_table.indexed_node(formal)?;
        let array = self.actual(indexed.collection)?;
        let state = self.state?;
        let declared = crate::declared_place_type_raw(self.program, callee, Some(entry), formal)?;
        let primitive = self.program.primitive_type_reference(declared)?;
        if !primitive.is_fixed_integer() {
            return None;
        }
        let mut bindings = Vec::new();
        for (parameter, actual) in self.parameters.iter().zip(self.arguments) {
            if parameter.is_mutable
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            {
                continue;
            }
            let Some(value) =
                ordered_values::operand(self.program, self.machine, state, *actual, None)
            else {
                continue;
            };
            if !value.is_immutable_snapshot(self.program, state)
                && !self.written.is_some_and(|written| value.survives(written))
            {
                continue;
            }
            bindings.push((Operand::parameter(parameter), value));
        }
        let selector = ordered_values::operand(self.program, callee, entry, indexed.index, None)?;
        let selectors = selector.rebound(&bindings);
        let [selector] = selectors.as_slice() else {
            return None;
        };
        let operand = ordered_values::indexed_operand(
            self.program,
            self.machine,
            state,
            array,
            selector.clone(),
            Some(primitive),
            None,
            0,
        )?;
        // Static coordinate labels remain interval/invalidation metadata, not
        // selector identity or a separate source-shape admission path.
        let spelled = if let Operand::Integer(literal, _) = selector {
            literal
                .value_u64()
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| {
                    crate::value_custody::places::canonical_collection_element_place(
                        self.program,
                        self.machine,
                        self.state,
                        array,
                        index,
                    )
                })
                .map(|place| place.label(self.program))
        } else {
            None
        };
        Some(ProjectedOperand {
            operand: Some(operand),
            spelled,
            declared_type: Some(declared),
        })
    }
}

/// Reuse the exact captured-origin roster, never a live descriptor spelling.
/// Only structural coordinates representable by the shared ordered algebra
/// enter arithmetic facts; unknown, ranged and conditional origins stay opaque.
fn captured_storage_operand(
    program: &dyn crate::CheckingInput,
    root: symbols::SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<Operand> {
    if !root.is_valid() {
        return None;
    }
    let mut operand = Operand::Place {
        root,
        path: program.symbols.name(root).to_owned(),
    };
    for segment in segments {
        operand = match segment {
            facts::PlaceSegment::Field { symbol } if symbol.is_valid() => Operand::Field {
                receiver: Box::new(operand),
                field: *symbol,
                name: program.symbols.name(*symbol).to_owned(),
                primitive: crate::value_custody::places::declaration_type_reference(
                    program, *symbol,
                )
                .and_then(|reference| program.primitive_type_reference(reference)),
            },
            facts::PlaceSegment::FixedIndex { index } => Operand::Indexed {
                collection: Box::new(operand),
                index: Box::new(Operand::Integer(
                    numerics::literals::IntegerLiteral::from_parts(
                        false,
                        numerics::literals::IntegerRadix::Decimal,
                        &index.to_string(),
                    )
                    .ok()?,
                    None,
                )),
                primitive: None,
            },
            _ => return None,
        };
    }
    Some(operand)
}

fn seed_conjunct(
    program: &dyn crate::CheckingInput,
    callee: &Machine,
    entry: &State,
    substitution: &Substitution<'_>,
    conjunct: ExpressionHandle,
    environment: &mut ValueEnvironment,
) {
    let Some(comparison) = program.expression_table.binary_node(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        seed_conjunct(
            program,
            callee,
            entry,
            substitution,
            comparison.left,
            environment,
        );
        seed_conjunct(
            program,
            callee,
            entry,
            substitution,
            comparison.right,
            environment,
        );
        return;
    }
    if !matches!(
        comparison.operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
    ) || !has_builtin_binary_expression_meaning(program, callee, Some(entry), conjunct)
    {
        return;
    }
    let side = |expression| {
        if let typed_trees::expression::ExpressionNode::Integer(literal) =
            program.expression_table.expression(expression)
        {
            Some(ProjectedOperand::literal(literal))
        } else {
            substitution
                .element(callee, entry, expression)
                .or_else(|| substitution.scalar(callee, entry, expression))
        }
    };
    let sides = (side(comparison.left), side(comparison.right));
    let (Some(left), Some(right)) = sides else {
        return;
    };
    contribute_projected_comparison(
        program,
        environment,
        comparison.operator,
        (&left, comparison.left),
        (&right, comparison.right),
    );
}

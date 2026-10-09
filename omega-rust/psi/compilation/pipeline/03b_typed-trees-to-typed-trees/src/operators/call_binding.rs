//! Bind selected token machines and float shorthand to ordinary call edges.
//!
//! The shared selection pass also resolves float shorthand through
//! `float_shorthand_calls`, using the same named-use facts. It must happen
//! before validation so alias calls expose the requirement's signature and
//! effects instead of leaking builtin symbols into execution planning.
//!
//! The executable-supply contract in wiki/spec/language/expressions.md says an
//! ordinary direct `machine + Name(...) { ... }` is supplied by its own checked
//! body, through ordinary call machinery, with no operator interpreter and no
//! satisfier search. Operand-directed selection (`resolve_spelling_for_operands`
//! over the typed `machine_token_bindings` views) settles which declaration a
//! spelled use means; this pass turns that settled meaning into the ordinary
//! call the author could have written by hand -- `left + right` becomes a
//! `Call` on `Wrapped::add`'s entry state with the operands as its arguments,
//! in operand order, evaluated once each.
//! An ordinary bodyless boundary token machine has the same call shape but retains
//! requirement supply: Psi emits its requirement call and Omega installs the
//! selected provider. Its operator view never creates a second executable slot.
//!
//! Private typed preparation binds its initial roots once. Each elaboration
//! round then binds only the new bodies and inferred-result owners it produced,
//! after ordinary specialization settles their dependent temporaries. Unchanged
//! graphs need no fixed-point-wide sweep. Selection reads static operand types
//! and authored origins, not checking conclusions or flow facts. A new call
//! contributes to the same fixed point, including when its operands become known
//! through a generic result. Binding precedes
//! program validation so
//! ownership, effects, termination, contracts, and every executing consumer
//! (interpreter, lowering, Terminal, native) see a plain call edge to the
//! declaration -- the same shape build-time evaluation forms for selected
//! boundary providers (`build-time-evaluation/.../selected_operators.rs`). The
//! rewritten node keeps its authored `Operator` selection occurrence, which
//! finalization settles to the machine symbol
//! (`authored_selections/finalization.rs`), so the checked artifact retains
//! the authored token occurrence together with the exact declaration.
//!
//! A selected implementation parameter supplies the same ordinary requirement
//! call as `Operations::combine(left, right)`. Bind its existing placeholder
//! before body cloning so ordinary evidence dispatch carries the exact closed
//! application and implementation arguments. A newly closed occurrence uses
//! that same dispatch record directly. Neither route executes builtin arithmetic
//! merely because the selected requirement has no top-level token binding.
//!
//! Binary and indexed spellings at expression occurrences bind here. `[]`
//! (and `[..]` under the spec's range normalization -- an omitted start means
//! zero, and an inclusive end `..=end` means `end + 1` -- so `a[..b]` and
//! `a[c..=d]` bind exactly like `a[0..b]` and `a[c..d+1]`) supplies the
//! collection place as the first explicit argument, just as the named static
//! call does. Canonical floating comparisons already
//! have a selected-comparison route, including arm-local match equality. A
//! bodyless requirement keeps its exact occurrence and operands on that route:
//! rewriting it to a bodyless call would erase the comparison computation,
//! while reconstructing a match call would reevaluate its saved subject.
//! Its provider still needs independent selection and realization. Body-bearing
//! machines cannot use that route in place of executing their body. Open-ended
//! range uses (`a[start..]`, `a[..]`) and other unsupported selections reject:
//! leaving a selection without executable supply could silently substitute
//! builtin arithmetic, which the contract forbids.

use checked_trees::{
    CheckedOperatorOccurrence, CheckedOperatorResolutionStatus, CheckedValueOrigin,
};
use diagnostics::Diagnostic;
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableRangeExpression,
};
use typed_trees::typed_trees::StaticRequirementDispatch;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use std::collections::HashSet;
use validation::expression_type_reference_for_origin;

/// Bind resolved token machines and float shorthand to calls, retaining
/// canonical bodyless float comparisons on their selected-comparison route.
pub(crate) fn bind_selected_operator_calls(
    program: &mut TypedTrees,
    roots: &typed_trees::references::DeclarationSelection,
) -> Result<usize, Vec<Diagnostic>> {
    if program.machine_token_bindings().is_empty()
        && program.operators().is_empty()
        && !program.machines().iter().any(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
        && !program.traits().iter().any(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .any(|requirement| requirement.spelling.is_some())
        })
    {
        return Ok(0);
    }
    // Binding reads the operator uses and their candidates only.
    let values = validation::collect_executable_value_origins(program, roots);
    let mut facts = validation::build_operator_uses(program, &values);
    let named = validation::collect_named_requirement_uses(program, &values);
    let shorthand_count = super::float_shorthand_calls::bind(program, &facts, &named)?;
    // A domain-homed binding (`machine + Quantity::Additive::add`) is a
    // `DomainPending` candidate until binding-site selection settles it. That
    // selection reads only static operand qualifications, mints, and
    // signature `requires` -- never flow facts -- so it can run here, before
    // validation, and an unselected domain meaning keeps the builtin surface
    // exactly as a domain-homed `operator` declaration did.
    validation::select_pending_domain_operator_meanings(program, &mut facts);
    let mut diagnostics = Vec::new();
    let mut bindings: Vec<(ExpressionHandle, SelectedOperatorCall, CheckedValueOrigin)> =
        Vec::new();
    let mut occurrences = facts
        .uses
        .iter()
        .map(|(handle, usage)| (usage.expression, usage.origin, usage.spelling, Some(handle)))
        .collect::<Vec<_>>();
    let mut seen = HashSet::with_hasher(symbols::BuildSymbolHasher);
    seen.extend(
        occurrences
            .iter()
            .map(|(expression, origin, _, _)| (*expression, *origin)),
    );
    let mut expressions = Vec::new();
    // Checked operator facts omit primitive-looking binaries without closed
    // candidates. A lexical implementation is not closed yet: retain its
    // source occurrence without fabricating a checked conformance row.
    for (_, value) in values
        .values
        .iter()
        .filter(|(_, value)| value.origin.machine_symbol().is_some())
    {
        expressions.clear();
        validation::expression_traversal::collect_expression_tree(
            program,
            value.expression,
            &mut expressions,
        );
        for expression in expressions.iter().copied() {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                continue;
            };
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                continue;
            };
            if seen.insert((expression, value.origin)) {
                occurrences.push((expression, value.origin, spelling, None));
            }
        }
    }
    for (expression, origin, spelling, use_handle) in occurrences {
        let operator_use = use_handle.map(|handle| facts.uses.get(handle));
        let selected = operator_use.map_or_else(SymbolHandle::invalid, |usage| {
            usage.selected_operator_symbol
        });
        // This classifier rejoins the exact selected declaration, signature,
        // operands and occurrence, not merely the operator token. Preserve its
        // computation and boundary application for independent provider replay;
        // classification grants neither provider authority nor body execution.
        if program.machine_by_symbol(selected).is_some_and(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && !machine.body_is_present
        }) && use_handle
            .and_then(|handle| facts.selected_float_comparison(program, handle))
            .is_some()
        {
            continue;
        }
        // One expression can surface under several value origins (a shared
        // template body, for example); its body binding happens once.
        if bindings.iter().any(|(bound, _, _)| *bound == expression) {
            continue;
        }
        let target = match selected_call_target(program, expression, origin, spelling, operator_use)
        {
            Ok(Some(target)) => target,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let bindable = operator_use
            .is_none_or(|usage| usage.occurrence == CheckedOperatorOccurrence::Expression)
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(_) | ExpressionNode::Indexed(_)
            );
        if !bindable {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token `{}` in a position whose body supply is not implemented; only binary and indexed expression uses bind the declaration's own body",
                target.name,
                spelling.symbol(),
            )));
            continue;
        }
        bindings.push((expression, target, origin));
    }

    let mut bound_count = bindings.len() + shorthand_count;
    // A qualified requirement name is not a runtime receiver. A resolved
    // named call keeps its exact entry and authored call metadata; erase only
    // the namespace prefix after rejoining it with that token declaration.
    // Lexical value receivers must never be reclassified by their spelling.
    for (expression, requirement) in facts
        .named_uses()
        .map(|fact| (fact.expression, fact.selected_operator_symbol))
        .chain(
            named
                .iter()
                .map(|(_, fact)| (fact.expression, fact.requirement_symbol)),
        )
    {
        let Some(binding) = program.machine_token_binding_by_symbol(requirement) else {
            continue;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            continue;
        };
        if !call.receiver.is_valid() {
            continue;
        }
        let Some(receiver) = program.expression_table.name_path_node(call.receiver) else {
            continue;
        };
        if receiver.symbol.is_valid()
            && !matches!(
                program.symbols.get(receiver.symbol).kind,
                symbols::SymbolKind::Data
                    | symbols::SymbolKind::Domain
                    | symbols::SymbolKind::Module
            )
        {
            continue;
        }
        let path = program.operator_path_members(binding.name);
        let Some((member, namespace)) = path.split_last() else {
            continue;
        };
        if member.as_str() != call.target.as_str()
            || namespace != program.expression_table.name_path_members(receiver.members)
        {
            continue;
        }
        let Some(machine) = program.machine_by_symbol(requirement) else {
            continue;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            continue;
        };
        let entry_symbol = entry.symbol;
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) {
            call.receiver = ExpressionHandle::invalid();
            call.target_symbol = entry_symbol;
            bound_count += 1;
        }
    }
    for (expression, target, origin) in bindings {
        // Copy the operand source handles before any endpoint normalization
        // mutates the expression table below.
        let operand_source = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => OperandSource::Pair([binary.left, binary.right]),
            ExpressionNode::Indexed(indexed) => OperandSource::Indexed {
                collection: indexed.collection,
                index: indexed.index,
            },
            _ => unreachable!("binding candidates are binary or indexed expressions"),
        };
        // Copy the signature views the rewrite needs before endpoint
        // normalization mutates the expression table.
        // `[..]` telescopes are exactly (collection, start, end); the range
        // endpoints supply operands one and two after the spec's
        // normalization.
        let start_parameter = target
            .start_parameter
            .is_valid()
            .then_some(target.start_parameter);
        let operands = match operand_source {
            OperandSource::Pair(pair) => pair.to_vec(),
            OperandSource::Indexed { collection, index } => {
                let mut operands = vec![collection];
                let range = match program.expression_table.expression(index) {
                    ExpressionNode::Range(range) => Some(*range),
                    _ => None,
                };
                match range {
                    Some(range) => {
                        match range_operands(program, &target.name, range, start_parameter, origin)
                        {
                            Ok((start, end)) => {
                                operands.push(start);
                                operands.push(end);
                            }
                            Err(diagnostic) => {
                                diagnostics.push(diagnostic);
                                continue;
                            }
                        }
                    }
                    None => operands.push(index),
                }
                operands
            }
        };
        let arguments = program
            .expression_table
            .insert_expression_handles(operands.iter().copied());
        let receiver = if target.binder.is_valid() {
            let binder_name =
                typed_trees::name::Identifier::generated(program.symbols.name(target.binder));
            crate::monomorphization::insert_subject_name(
                &mut program.expression_table,
                binder_name,
                target.binder,
            )
        } else {
            ExpressionHandle::invalid()
        };
        *program.expression_table.expression_mut(expression) =
            ExpressionNode::Call(TableCallExpression {
                authored_machine_application: Default::default(),
                exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                receiver,
                target_symbol: target.entry,
                static_machine_parameter: SymbolHandle::invalid(),
                target: target.name,
                static_requirement_dispatch: target.dispatch,
                machine_arguments: target.arguments,
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                    origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    ..Default::default()
                },
            });
    }
    if diagnostics.is_empty() {
        Ok(bound_count)
    } else {
        Err(diagnostics)
    }
}

/// One ordinary call destination, whether supplied by a token machine or an
/// explicit implementation. The binder is a namespace, never a data argument.
struct SelectedOperatorCall {
    entry: SymbolHandle,
    name: typed_trees::name::Identifier,
    binder: SymbolHandle,
    start_parameter: TypeReferenceHandle,
    dispatch: Option<StaticRequirementDispatch>,
    arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
}

fn selected_call_target(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: language_core::OperatorSpelling,
    operator_use: Option<&checked_trees::CheckedOperatorUseFact>,
) -> Result<Option<SelectedOperatorCall>, Diagnostic> {
    let operand_types = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(
            [binary.left, binary.right]
                .map(|operand| expression_type_reference_for_origin(program, operand, origin))
                .to_vec(),
        ),
        ExpressionNode::Indexed(indexed) => {
            Some(validation::indexed_operand_types(program, indexed, origin))
        }
        _ => None,
    };
    if let Some(operand_types) = operand_types
        && let Some(machine) = origin.machine_symbol()
    {
        let bound = typed_trees::operator::bound_trait_operator_meanings(
            program,
            machine,
            spelling,
            &operand_types,
        );
        if let [meaning] = bound.as_slice() {
            let definition = program
                .trait_definition_by_symbol(meaning.bound.trait_symbol)
                .ok_or_else(|| Diagnostic::error("selected implementation bound lost its trait"))?;
            let mut requirements = Vec::new();
            crate::monomorphization::collect_evidence_requirement_closure(
                program,
                definition,
                &mut Vec::new(),
                &mut requirements,
            );
            let entry = requirements
                .iter()
                .position(|requirement| requirement.symbol == meaning.requirement.symbol)
                .and_then(|position| {
                    program
                        .symbols
                        .child_handles(meaning.bound.binder)
                        .into_iter()
                        .flatten()
                        .nth(position)
                })
                .ok_or_else(|| {
                    Diagnostic::error(
                        "selected implementation lost its requirement call placeholder",
                    )
                })?;
            return Ok(Some(SelectedOperatorCall {
                entry,
                name: meaning.requirement.name.clone(),
                binder: meaning.bound.binder,
                start_parameter: program
                    .state_signature_parameters(meaning.requirement)
                    .get(1)
                    .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                        parameter.type_reference
                    }),
                dispatch: None,
                arguments: Box::default(),
            }));
        }
        if !bound.is_empty() {
            return Err(Diagnostic::error(
                "operator has multiple explicitly selected implementation meanings",
            ));
        }
        let closed = typed_trees::operator::selected_trait_operator_meanings(
            program,
            machine,
            spelling,
            &operand_types,
        );
        if let [meaning] = closed.as_slice() {
            let entry = program
                .state_by_symbol(meaning.row.realization_state)
                .ok_or_else(|| {
                    Diagnostic::error("selected implementation lost its realization entry")
                })?;
            return Ok(Some(SelectedOperatorCall {
                entry: entry.symbol,
                name: entry.name.clone(),
                binder: SymbolHandle::invalid(),
                start_parameter: program
                    .state_parameters(entry)
                    .get(1)
                    .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                        parameter.type_reference
                    }),
                dispatch: Some(StaticRequirementDispatch {
                    application_report_fingerprint: meaning.application.report_fingerprint,
                    application_commitment: meaning.application.commitment,
                    declaring_trait: meaning.row.declaring_trait,
                    requirement: meaning.row.requirement,
                    realization_machine: meaning.row.realization_machine,
                    realization_state: meaning.row.realization_state,
                }),
                arguments: meaning.application.arguments.clone(),
            }));
        }
        if !closed.is_empty() {
            return Err(Diagnostic::error(
                "operator has multiple explicitly selected implementation meanings",
            ));
        }
    }
    let Some(operator_use) = operator_use else {
        return Ok(None);
    };
    let selected = operator_use.selected_operator_symbol;
    if operator_use.status != CheckedOperatorResolutionStatus::Resolved
        || program.machine_token_binding_by_symbol(selected).is_none()
    {
        return Ok(None);
    }
    let machine = program
        .machine_by_symbol(selected)
        .ok_or_else(|| Diagnostic::error("selected token machine lost its declaration"))?;
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or_else(|| Diagnostic::error("selected token machine declares no entry state"))?;
    Ok(Some(SelectedOperatorCall {
        entry: entry.symbol,
        name: entry.name.clone(),
        binder: SymbolHandle::invalid(),
        start_parameter: program
            .state_parameters(entry)
            .get(1)
            .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                parameter.type_reference
            }),
        dispatch: None,
        arguments: Box::default(),
    }))
}

/// Operand source positions of a token-bound expression, copied out before
/// range endpoint normalization mutates the expression table.
enum OperandSource {
    Pair([ExpressionHandle; 2]),
    Indexed {
        collection: ExpressionHandle,
        index: ExpressionHandle,
    },
}

/// The operand pair an authored range supplies to a `[..]` binding's start and
/// end parameters, after the normalization the spec assigns to `[..]`: an
/// omitted start means zero, and an inclusive end `start..=end` means
/// `start..(end + 1)`.
///
/// Both normalized forms are gated on integer operands. Literal zero needs a
/// declared integer start parameter to land on. The synthesized `end + 1` must
/// stay builtin arithmetic, and a bare integer primitive can never match a
/// direct token binding -- the semantic-home law keeps compiler primitives out
/// of every declared operator family -- so an integer end operand guarantees
/// the inserted `+` means the builtin one. An omitted end (`a[start..]`,
/// `a[..]`) would need the collection's length, which a `[..]` telescope has
/// no operand formation for, so those uses still reject.
fn range_operands(
    program: &mut TypedTrees,
    machine_name: &str,
    range: TableRangeExpression,
    start_parameter: Option<TypeReferenceHandle>,
    origin: CheckedValueOrigin,
) -> Result<(ExpressionHandle, ExpressionHandle), Diagnostic> {
    if !range.end.is_valid() {
        return Err(Diagnostic::error(format!(
            "`{machine_name}` was selected for its fixed operator token `[..]` but the range use omits its end; the omitted endpoint would be the collection's length, which a declared telescope cannot form -- closed `start..end`, `..end`, `start..=end`, and `..=end` uses bind the declaration's own body",
        )));
    }
    let start = if range.start.is_valid() {
        range.start
    } else {
        let start_accepts_zero = start_parameter
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .is_some_and(PrimitiveType::accepts_integer_literal);
        if !start_accepts_zero {
            return Err(Diagnostic::error(format!(
                "`{machine_name}` was selected for its fixed operator token `[..]` but the range use omits its start; the omitted endpoint is zero, which only an integer start parameter forms -- write it explicitly as `0..`",
            )));
        }
        program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)))
    };
    if !range.end_inclusive {
        return Ok((start, range.end));
    }
    let end_is_integer = expression_type_reference_for_origin(program, range.end, origin)
        .and_then(|type_reference| validation::unwrapped_type_reference(program, type_reference))
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some_and(PrimitiveType::accepts_integer_literal);
    if !end_is_integer {
        return Err(Diagnostic::error(format!(
            "`{machine_name}` was selected for its fixed operator token `[..]` but the range use is inclusive (`..=`); the normalized `end + 1` form requires an integer end operand -- write the exclusive bound explicitly",
        )));
    }
    let one = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let end = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: range.end,
            operator: BinaryOperator::Add,
            right: one,
        }));
    Ok((start, end))
}

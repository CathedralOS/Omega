//! Rebind checked calculus applications to the caller's observed values.
//!
//! The mathematical producer has already checked and reduced the application.
//! This consumer reads that term, never a source definition body. Its callback
//! owns observation and substitution; kernel Int alone supplies no permission
//! to reread storage or to replace a selected source operation's meaning.

use super::StructuralTerm;
use crate::mathematical_applications::{
    PreparedApplication, PreparedConstantMeaning, PreparedMathematicalView, PreparedSourceBinding,
    PreparedSourceBindingKind,
};
use numerics::literals::FloatLiteral;
use proof_admission::{Term, TermHandle};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

pub(in crate::proof_contracts::contract_entailment) fn application_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    mut observe: impl FnMut(ExpressionHandle) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    let view = program.prepared_mathematics()?;
    let application = view.application(expression)?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .map(|capture| {
            let value = observe(capture.expression)?;
            let value = if program
                .primitive_type_reference(capture.source_type)
                .is_some_and(|primitive| {
                    primitive.is_fixed_integer()
                        || primitive == typed_trees::types::PrimitiveType::Addr
                }) {
                StructuralTerm::IntegerEmbedding {
                    reference: capture.source_type,
                    value: Box::new(value),
                }
            } else if crate::reference_shell(program, capture.source_type).is_none()
                && crate::value_custody::places::data_definition_for_type(
                    program,
                    capture.source_type,
                )
                .is_some()
            {
                super::owned_projection_observation(program, capture.source_type, value)
            } else {
                value
            };
            Some(Some(value))
        })
        .collect::<Option<Vec<_>>>()?;
    read_term(program, view, application, application.term, &captures, 0)
}

fn read_term(
    program: &dyn crate::CheckingInput,
    view: PreparedMathematicalView<'_>,
    application: &PreparedApplication,
    term: TermHandle,
    captures: &[Option<StructuralTerm>],
    depth: usize,
) -> Option<StructuralTerm> {
    if depth >= 128 {
        return None;
    }
    let read = |term| read_term(program, view, application, term, captures, depth + 1);
    let source_captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .len();
    let inserted_binders = u32::try_from(captures.len().checked_sub(source_captures)?).ok()?;
    if let Some((carrier, [left, right])) = view.identity_value(application, term, inserted_binders)
    {
        return Some(StructuralTerm::Identity {
            carrier: carrier.type_identity,
            left: Box::new(read(left)?),
            right: Box::new(read(right)?),
        });
    }
    match view.arena.get(term) {
        Term::TwoZero | Term::TwoOne => Some(boolean(matches!(view.arena.get(term), Term::TwoOne))),
        Term::Variable(index) => captures
            .get(captures.len().checked_sub(index as usize + 1)?)?
            .clone(),
        Term::Squash { ty } => read(ty),
        Term::Id { ty, left, right } if matches!(view.arena.get(ty), Term::Two) => {
            let reference =
                crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    symbols::BuiltinTypeAtom::Bool,
                )?;
            let meaning = (
                typed_trees::types::PrimitiveType::Bool,
                program.arithmetic_domain_for_type_reference(reference),
            );
            Some(StructuralTerm::ScalarBinary {
                references: [reference; 2],
                operator: BinaryOperator::Equal,
                meaning: [meaning; 2],
                left: Box::new(read(left)?),
                right: Box::new(read(right)?),
            })
        }
        Term::Id { ty, left, right } => {
            let Term::Constant { declaration, .. } = view.arena.get(ty) else {
                return None;
            };
            if !matches!(
                view.constant(declaration),
                Some(
                    PreparedConstantMeaning::IntegerCarrier
                        | PreparedConstantMeaning::AddressCarrier
                )
            ) {
                return None;
            }
            Some(binary(BinaryOperator::Equal, read(left)?, read(right)?))
        }
        Term::Sigma { domain, codomain } => {
            let mut extended = captures.to_vec();
            extended.push(None);
            if matches!(view.arena.get(domain), Term::Two) {
                let Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } = view.arena.get(codomain)
                else {
                    return None;
                };
                if !matches!(view.arena.get(scrutinee), Term::Variable(0)) {
                    return None;
                }
                // The checked logical sum chooses a Boolean witness. Its
                // branches keep the same outer captures; neither branch is an
                // ambient assumption about a caller's runtime Boolean.
                return Some(binary(
                    BinaryOperator::Or,
                    read_term(
                        program,
                        view,
                        application,
                        zero_branch,
                        &extended,
                        depth + 1,
                    )?,
                    read_term(program, view, application, one_branch, &extended, depth + 1)?,
                ));
            }
            Some(binary(
                BinaryOperator::And,
                read(domain)?,
                read_term(program, view, application, codomain, &extended, depth + 1)?,
            ))
        }
        _ => {
            let (declaration, arguments) =
                crate::mathematical_applications::application_spine(view, term)?;
            match (view.constant(declaration)?, arguments.as_slice()) {
                (PreparedConstantMeaning::Integer(value), []) => {
                    Some(StructuralTerm::Integer(value.clone()))
                }
                (PreparedConstantMeaning::FloatLiteral { format, bits }, []) => {
                    // Retain the producer's already landed format and bits.
                    // Rebuilding an anonymous f64 literal loses the f32 lane
                    // and cannot authenticate an IEEE bound observation.
                    Some(StructuralTerm::Float(FloatLiteral::from_bits(
                        *format, *bits,
                    )?))
                }
                (
                    PreparedConstantMeaning::FloatRelation {
                        operator,
                        reference,
                        ..
                    },
                    [left, right],
                ) => {
                    // The producer retained selected IEEE authority. Keep its
                    // scalar lane distinct from mathematical integer order,
                    // so negation never acquires integer complement laws.
                    let primitive = program.primitive_type_reference(*reference)?;
                    let domain = program.arithmetic_domain_for_type_reference(*reference);
                    Some(StructuralTerm::ScalarBinary {
                        references: [*reference; 2],
                        operator: *operator,
                        meaning: [(primitive, domain); 2],
                        left: Box::new(read(*left)?),
                        right: Box::new(read(*right)?),
                    })
                }
                (PreparedConstantMeaning::AddressBound(symbol), [])
                    if program.symbols.builtin_type_member(*symbol)
                        == Some(symbols::BuiltinTypeMember::AddressBound) =>
                {
                    Some(StructuralTerm::BoundValue(*symbol))
                }
                (
                    PreparedConstantMeaning::IntegerOperation(operator)
                    | PreparedConstantMeaning::IntegerRelation(operator),
                    [left, right],
                ) => Some(binary(*operator, read(*left)?, read(*right)?)),
                // Captures already retain the source carrier. The checked
                // embedding preserves that value; it does not land it again.
                (PreparedConstantMeaning::IntegerEmbedding { .. }, [argument]) => read(*argument),
                (PreparedConstantMeaning::Negation, [proposition]) => {
                    let operand = read(*proposition)?;
                    // MathematicalBinary comparisons here come only from
                    // authenticated integer relations/identity, never IEEE or
                    // a selected custom comparison. Retain their exact
                    // complement instead of hiding it in Boolean equality.
                    if let StructuralTerm::MathematicalBinary {
                        operator,
                        left,
                        right,
                    } = &operand
                        && let Some(operator) = operator.complement()
                    {
                        return Some(StructuralTerm::MathematicalBinary {
                            operator,
                            left: left.clone(),
                            right: right.clone(),
                        });
                    }
                    Some(binary(
                        BinaryOperator::Equal,
                        operand,
                        StructuralTerm::boolean(false),
                    ))
                }
                (
                    PreparedConstantMeaning::SourceOperation {
                        expression,
                        owner,
                        source_type,
                        bindings,
                    },
                    arguments,
                ) => {
                    match source_operation_term(
                        program,
                        *expression,
                        *owner,
                        *source_type,
                        bindings,
                        arguments,
                        read,
                    ) {
                        Ok(value) => Some(value),
                        Err(reason) => {
                            if super::super::struct_trace() {
                                eprintln!(
                                    "source value {:?} under {:?} cannot be read: {reason}",
                                    expression, owner
                                );
                            }
                            None
                        }
                    }
                }
                // Opaque and source-selected operations have no builtin
                // arithmetic law merely because their kernel result is Int.
                _ => None,
            }
        }
    }
}

/// Rejoin only the producer-recorded occurrence, under its own lexical scope.
/// The checked spine supplies the actual dependency values; observing the
/// occurrence must never recover the producer's unsubstituted formal instead.
fn source_operation_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owner: symbols::SymbolHandle,
    source_type: typed_trees::types::TypeReferenceHandle,
    bindings: &[PreparedSourceBinding],
    arguments: &[TermHandle],
    read: impl Fn(TermHandle) -> Option<StructuralTerm>,
) -> Result<StructuralTerm, String> {
    if bindings.len() != arguments.len() {
        return Err(format!(
            "dependency arity {} differs from checked argument arity {}",
            bindings.len(),
            arguments.len()
        ));
    }
    if !program.expression_table.expression_is_valid(expression) || !owner.is_valid() {
        return Err("the retained source occurrence or lexical owner is unavailable".to_owned());
    }
    let mut values = Vec::with_capacity(bindings.len());
    for (position, (binding, argument)) in bindings.iter().zip(arguments).enumerate() {
        if binding.kind == PreparedSourceBindingKind::Type {
            return Err(format!(
                "type dependency {position} has no ordinary source type substitution"
            ));
        }
        let value = read(*argument)
            .ok_or_else(|| format!("value dependency {position} has no readable actual value"))?;
        values.push(value);
    }
    let scope =
        crate::machine_calls::calls::machine_state_by_symbol(program, owner).or_else(|| {
            let machine = program.machine_by_symbol(owner)?;
            Some((machine, program.machine_states(machine).first()?))
        });
    let mut parameters = if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == owner)
    {
        crate::mathematical_applications::ordinary_parameters(program, definition)
    } else if let Some((_, state)) = scope {
        program.state_parameters(state).to_vec()
    } else {
        return Err("the retained lexical owner has no declaration telescope".to_owned());
    };
    // Copied contract roots can use fresh formals. Their exact source types
    // come from the producer's own dependency census, not from the Id goal.
    for binding in bindings {
        if binding.symbol.is_valid() && binding.source_type.is_valid() {
            if let Some(parameter) = parameters
                .iter_mut()
                .find(|parameter| parameter.symbol == binding.symbol)
            {
                parameter.type_reference = binding.source_type;
            } else {
                parameters.push(typed_trees::signature::StateParameter {
                    symbol: binding.symbol,
                    name: program.symbols.name(binding.symbol).into(),
                    type_reference: binding.source_type,
                    ..Default::default()
                });
            }
        }
    }
    let substitute = |candidate| {
        bindings.iter().zip(&values).find_map(|(binding, value)| {
            let same_symbol = binding.symbol.is_valid() && matches!(
                program.expression_table.expression(candidate),
                typed_trees::expression::ExpressionNode::Name(path) if path.symbol == binding.symbol
            );
            (binding.expressions.contains(&candidate) || same_symbol).then(|| value.clone())
        })
    };
    let value = super::structural_term(
        program,
        scope.map(|(machine, state)| (machine, state, source_type)),
        Some((owner, &parameters)),
        expression,
        Some(&substitute),
    ).ok_or_else(|| "the retained occurrence has no complete selected structural value after dependency substitution".to_owned())?;
    Ok(if source_type.is_valid() {
        super::owned_projection_observation(program, source_type, value)
    } else {
        value
    })
}

fn binary(operator: BinaryOperator, left: StructuralTerm, right: StructuralTerm) -> StructuralTerm {
    StructuralTerm::MathematicalBinary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn boolean(value: bool) -> StructuralTerm {
    StructuralTerm::boolean(value)
}

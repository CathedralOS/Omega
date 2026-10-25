//! Runtime data indices describe captured values, not mutable storage names.
//!
//! A type at a local formation captures the scalar roster at that statement;
//! a parameter captures arrival. Every store, result and call compares those
//! independent snapshots. CurrentScalarValues owns assignment/effect transfer
//! and checked equality, so changing the source variable never retags an old
//! object. These are erased checking obligations, not runtime proof storage.

use super::current_scalar_values::CurrentScalarValues;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::{CallFrameResolver, ScopedArithmeticValue};

#[derive(Clone)]
struct Application<'program> {
    reference: TypeReferenceHandle,
    formation: usize,
    substitutions: Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
    arguments: Vec<(SymbolHandle, Option<ExpressionHandle>)>,
    call: Option<validation::CallSite<'program>>,
    position: Option<usize>,
}

struct Scope<'a> {
    program: &'a dyn validation::CheckingInput,
    machine: &'a Machine,
    state: &'a State,
    statement: usize,
    target: TransitionTargetHandle,
    frames: Option<&'a CallFrameResolver<'a>>,
}

impl Scope<'_> {
    fn static_index(
        &self,
        argument: TypeReferenceHandle,
    ) -> Option<typed_trees::type_identity::NormalizedTypeIdentity> {
        let TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(argument)
        else {
            return None;
        };
        // Never use symbolic identity as equality of mutable runtime subjects.
        // This path preserves already-admitted non-integer static values too.
        if symbol.is_valid() && self.program.const_declaration_by_symbol(*symbol).is_none() {
            return None;
        }
        typed_trees::type_identity::applied_index_identity(
            self.program,
            typed_trees::type_identity::IndexIdentityArgument::TypeReference(argument),
            self.machine.symbol,
            &[],
        )
    }
    fn values(&self, at: usize) -> Option<CurrentScalarValues<'_>> {
        CurrentScalarValues::at(
            self.program,
            self.machine,
            self.state,
            at,
            if at == self.statement {
                self.target
            } else {
                TransitionTargetHandle::invalid()
            },
            self.frames,
        )
    }

    fn index(
        &self,
        argument: TypeReferenceHandle,
        application: &Application<'_>,
    ) -> Option<ScopedArithmeticValue> {
        let supplied = matches!(
            self.program.type_reference_table.type_reference(argument),
            TypeReferenceNode::ConstExpression(_)
        ) || matches!(self.program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. }
                if application.substitutions.iter().any(|(formal, _)| formal == symbol));
        if supplied && let Some(site) = application.call {
            let prefix_unchanged = if let validation::CallSite::Expression { expression, .. } = site
            {
                let statement = self
                    .program
                    .statement_table
                    .statement_at(self.state, application.formation)?;
                let roots = self
                    .program
                    .tables
                    .expression_roots(statement)
                    .into_iter()
                    .map(|(_, expression)| expression)
                    .collect::<Vec<_>>();
                validation::expression_traversal::expressions_before_occurrence(
                    self.program,
                    &roots,
                    expression,
                )
                .is_some_and(|prefix| {
                    prefix.iter().all(|expression| {
                        self.frames.is_some_and(|frames| {
                            frames
                                .expression_write_frame(self.machine, *expression)
                                .complete_paths()
                                .is_some_and(|paths| paths.is_empty())
                        })
                    })
                })
            } else {
                true
            };
            let missing_scalar = matches!(self.program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. }
                if application.substitutions.iter().any(|(formal, value)| formal == symbol && value.is_none()));
            if !prefix_unchanged
                || missing_scalar
                || application
                    .arguments
                    .iter()
                    .any(|(_, value)| value.is_none())
            {
                let target = site.target_symbol(self.program);
                let parameters = validation::call_target_parameters(self.program, target)?;
                return validation::captured_call_index_value(
                    self.program,
                    self.state.symbol,
                    application.formation,
                    site,
                    parameters,
                    target,
                    argument,
                );
            }
        }
        if let TypeReferenceNode::ConstExpression(expression) =
            *self.program.type_reference_table.type_reference(argument)
        {
            return self
                .values(application.formation)?
                .application_value_with_captured(
                    expression,
                    &application.arguments,
                    &application.substitutions,
                )
                .map(ScopedArithmeticValue::Term);
        }
        let TypeReferenceNode::Named { symbol, name } =
            self.program.type_reference_table.type_reference(argument)
        else {
            return None;
        };
        if symbol.is_valid() {
            if let Some(declaration) = self.program.const_declaration_by_symbol(*symbol) {
                let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
                return match CanonicalConstValue::new("", encoding.to_owned(), "")
                    .decode_encoding()?
                {
                    DecodedCanonicalConstValue::Integer { value, .. } => Some(
                        ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(value)),
                    ),
                    _ => None,
                };
            }
            if let Some((_, value)) = application
                .substitutions
                .iter()
                .find(|(parameter, _)| parameter == symbol)
            {
                return value.clone();
            }
            return self.values(application.formation)?.current_symbol(*symbol);
        }
        if let Ok(integer) = name.as_str().parse::<i128>() {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i128(integer),
            ));
        }
        match CanonicalConstValue::from_atom(name.as_str())?.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => Some(
                ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(value)),
            ),
            _ => None,
        }
    }

    fn argument_substitutions(
        &self,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
    ) -> (
        Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
        Vec<(SymbolHandle, Option<ExpressionHandle>)>,
    ) {
        let inert = arguments.iter().all(|argument| {
            self.frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, *argument)
                    .complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        });
        let values = self.values(self.statement);
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                // A missing captured operand is explicit. It cannot fall back to a
                // same-symbol current parameter when the target is recursive.
                let stable = inert
                    || matches!(
                        self.program.expression_table.expression(*argument),
                        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                    );
                let value = stable
                    .then(|| values.as_ref()?.current(*argument))
                    .flatten()
                    .map(ScopedArithmeticValue::Term);
                (
                    (parameter.symbol, value),
                    (parameter.symbol, stable.then_some(*argument)),
                )
            })
            .unzip()
    }

    fn actual(&self, expression: ExpressionHandle) -> Option<Application<'_>> {
        if !expression.is_valid() {
            return None;
        }
        let reference = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            expression,
        )?;
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let formation = self
                    .program
                    .state_local_declaration(self.state, path.head_symbol)
                    .map(|(position, _)| position)
                    .unwrap_or(0);
                Some(Application {
                    reference,
                    formation,
                    substitutions: Vec::new(),
                    arguments: Vec::new(),
                    call: None,
                    position: None,
                })
            }
            ExpressionNode::Call(call) => {
                let parameters =
                    validation::call_target_parameters(self.program, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                let (mut substitutions, captured_arguments) =
                    self.argument_substitutions(parameters, arguments);
                let inert = arguments.iter().all(|argument| {
                    self.frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(self.machine, *argument)
                            .complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    })
                });
                if let Some(signature) = self
                    .program
                    .dynamic_slot_by_symbol(call.target_symbol)
                    .and_then(|slot| self.program.dynamic_slot_requirement(slot))
                    .or_else(|| {
                        self.program
                            .trait_machine_signature(call.target_symbol)
                            .map(|(_, signature)| signature)
                    })
                {
                    let locals = self.program.state_signature_type_parameters(signature);
                    if locals.len() == call.machine_arguments.len() {
                        let values = self.values(self.statement);
                        for (formal, actual) in locals.iter().zip(call.machine_arguments.iter()) {
                            if matches!(
                                formal.kind,
                                TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
                            ) {
                                substitutions.push((
                                    formal.symbol,
                                    inert
                                        .then(|| values.as_ref()?.static_argument_value(actual))
                                        .flatten(),
                                ));
                            }
                        }
                    }
                }
                let site = validation::CallSite::Expression { expression, call };
                if let Some(callee) = self
                    .program
                    .machine_by_own_or_state_symbol(call.target_symbol)
                    && let Some(values) = self.values(self.statement)
                    && let Some(bindings) = values.machine_value_bindings(callee, site)
                {
                    // Generic value coordinates share the ordinary call's
                    // substitution. Effectful operands require the completed
                    // operand projection rather than a current-home read.
                    let inert = arguments.iter().all(|argument| {
                        self.frames.is_some_and(|frames| {
                            frames
                                .expression_write_frame(self.machine, *argument)
                                .complete_paths()
                                .is_some_and(|paths| paths.is_empty())
                        })
                    });
                    substitutions.extend(
                        bindings
                            .into_iter()
                            .map(|(symbol, value)| (symbol, inert.then_some(value))),
                    );
                }

                Some(Application {
                    // Interpret the declaration under captured formal bindings.
                    // A caller-applied index could otherwise bypass the prefix
                    // guard by ceasing to name any of those formal binders.
                    reference: validation::call_target_return_type(
                        self.program,
                        call.target_symbol,
                    )?,
                    formation: self.statement,
                    substitutions,
                    arguments: captured_arguments,
                    call: Some(site),
                    position: None,
                })
            }
            ExpressionNode::Member(member) => {
                let mut receiver = self.actual(member.receiver)?;
                receiver.reference = reference;
                Some(receiver)
            }
            ExpressionNode::Borrow(borrow) => {
                let mut receiver = self.actual(borrow.target)?;
                receiver.reference = reference;
                Some(receiver)
            }
            ExpressionNode::Indexed(indexed) => {
                let mut collection = self.actual(indexed.collection)?;
                collection.reference = reference;
                Some(collection)
            }
            _ => None,
        }
    }

    fn compare(&self, actual: &Application<'_>, required: &Application<'_>, depth: usize) -> bool {
        if depth > 128 {
            return false;
        }
        let table = &self.program.type_reference_table;
        let actual_reference = actual.reference;
        let required_reference = required.reference;
        let exact = |left, right| {
            let left = typed_trees::type_identity::applied_type_identity(
                self.program,
                left,
                self.machine.symbol,
                &[],
            );
            let right = typed_trees::type_identity::applied_type_identity(
                self.program,
                right,
                self.machine.symbol,
                &[],
            );
            left.is_some() && left == right
        };
        let recurse = |left, right| {
            let mut actual = actual.clone();
            actual.reference = left;
            let mut required = required.clone();
            required.reference = right;
            self.compare(&actual, &required, depth + 1)
        };
        match (
            table.type_reference(actual_reference),
            table.type_reference(required_reference),
        ) {
            (
                TypeReferenceNode::Reference {
                    referee: left,
                    access: left_access,
                    ..
                },
                TypeReferenceNode::Reference {
                    referee: right,
                    access: right_access,
                    ..
                },
            ) => left_access == right_access && recurse(*left, *right),
            (
                TypeReferenceNode::Slice { element_type: left },
                TypeReferenceNode::Slice {
                    element_type: right,
                },
            ) => recurse(*left, *right),
            (
                TypeReferenceNode::FixedArray {
                    element_type: left,
                    length: left_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right,
                    length: right_length,
                },
            ) => left_length == right_length && recurse(*left, *right),
            (
                TypeReferenceNode::Generic {
                    base_symbol: left_base,
                    arguments: left,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_base,
                    arguments: right,
                    ..
                },
            ) if left_base == right_base => {
                let Some(data) = self.program.data_definition_by_symbol(*left_base) else {
                    return exact(actual_reference, required_reference);
                };
                let parameters = self.program.data_type_parameters(data);
                let left = table.type_reference_handles(*left);
                let right = table.type_reference_handles(*right);
                if parameters.len() != left.len() || parameters.len() != right.len() {
                    return false;
                }
                parameters
                    .iter()
                    .zip(left.iter().zip(right))
                    .all(|(parameter, (left, right))| {
                        if matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                            if let (Some(left), Some(right)) =
                                (self.static_index(*left), self.static_index(*right))
                            {
                                return left == right;
                            }
                            // Parameter types retain their arrival subjects. A
                            // forwarded reference can prove the exact scalar
                            // relation directly; a missing descriptor-capture
                            // proof must not discard that independent evidence.
                            if let (Some(left), Some(right)) =
                                (self.index(*left, actual), self.index(*right, required))
                                && (left == right || self.values(self.statement).is_some_and(|values| {
                                    values.proves_values_relation(self.program, self.machine,
                                        BinaryOperator::Equal, &left, &right)
                                }))
                            {
                                return true;
                            }
                            let closed_requirement = self.static_index(*right).is_some()
                                || matches!(self.program.type_reference_table.type_reference(*right),
                                    TypeReferenceNode::ConstExpression(expression)
                                        if self.program.closed_integer_expression_value_in(*expression, self.machine.symbol).is_some());
                            if !closed_requirement
                                && let (Some(site), Some(position)) =
                                    (required.call, required.position)
                                && let Some((_, descriptor, _)) = self
                                    .program
                                    .data_application_value_fields(actual.reference)
                                    .into_iter()
                                    .find(|(binder, _, _)| binder.symbol == parameter.symbol)
                            {
                                let target = site.target_symbol(self.program);
                                let Some(parameters) =
                                    validation::call_target_parameters(self.program, target)
                                else {
                                    return false;
                                };
                                return validation::captured_call_argument_index_matches(
                                    self.program,
                                    self.state.symbol,
                                    self.statement,
                                    site,
                                    parameters,
                                    position,
                                    actual.reference,
                                    descriptor.symbol,
                                    *right,
                                );
                            }
                            false
                        } else if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                            let left = typed_trees::type_identity::applied_index_identity(
                                self.program,
                                typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                                    *left,
                                ),
                                self.machine.symbol,
                                &[],
                            );
                            let right = typed_trees::type_identity::applied_index_identity(
                                self.program,
                                typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                                    *right,
                                ),
                                self.machine.symbol,
                                &[],
                            );
                            left.is_some() && left == right
                        } else if self.has_indices(*left, 0) || self.has_indices(*right, 0) {
                            recurse(*left, *right)
                        } else {
                            exact(*left, *right)
                        }
                    })
            }
            _ => exact(actual_reference, required_reference),
        }
    }

    fn has_indices(&self, reference: TypeReferenceHandle, depth: usize) -> bool {
        if !reference.is_valid() || depth > 128 {
            return false;
        }
        let table = &self.program.type_reference_table;
        match table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type: inner, ..
            }
            | TypeReferenceNode::Reference { referee: inner, .. }
            | TypeReferenceNode::FixedArray {
                element_type: inner,
                ..
            }
            | TypeReferenceNode::Slice {
                element_type: inner,
            } => self.has_indices(*inner, depth + 1),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                self.program
                    .data_definition_by_symbol(*base_symbol)
                    .is_some_and(|data| {
                        self.program
                            .data_type_parameters(data)
                            .iter()
                            .any(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Value { .. })
                            })
                    })
                    || table
                        .type_reference_handles(*arguments)
                        .iter()
                        .any(|argument| self.has_indices(*argument, depth + 1))
            }
            TypeReferenceNode::Named { symbol, .. } => self
                .program
                .data_definition_by_symbol(*symbol)
                .and_then(|data| data.generic_instance)
                .is_some_and(|origin| self.has_indices(origin, depth + 1)),
            _ => false,
        }
    }

    /// The source telescope owns descriptor association; use the retained field
    /// handle rather than treating an identically spelled ordinary field as an index.
    fn descriptor_arguments(
        &self,
        application: &Application<'_>,
    ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
        self.program
            .data_application_value_fields(application.reference)
            .into_iter()
            .map(|(_, field, argument)| (field.symbol, argument))
            .collect()
    }

    fn check_descriptor(
        &self,
        value: ExpressionHandle,
        argument: TypeReferenceHandle,
        application: &Application<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let static_equal = self.static_index(argument).is_some_and(|required| {
            typed_trees::type_identity::applied_index_identity(
                self.program,
                typed_trees::type_identity::IndexIdentityArgument::Expression(value),
                self.machine.symbol,
                &[],
            )
            .is_some_and(|actual| actual == required)
        });
        let equal = static_equal
            || self.values(self.statement).is_some_and(|values| {
                let Some(required) = self.index(argument, application) else {
                    return false;
                };
                let Some(actual) = values.current(value).map(ScopedArithmeticValue::Term) else {
                    return false;
                };
                actual == required
                    || values.proves_values_relation(
                        self.program,
                        self.machine,
                        BinaryOperator::Equal,
                        &actual,
                        &required,
                    )
            });
        if !equal {
            diagnostics.push(
                Diagnostic::error(
                    "cannot prove runtime index descriptor equals its captured index",
                )
                .with_source_span(self.program.expression_table.source_span(value)),
            );
        }
    }

    fn descriptor_target(
        &self,
        target: ExpressionHandle,
    ) -> Option<(Application<'_>, TypeReferenceHandle)> {
        let selected = match self.program.expression_table.expression(target) {
            ExpressionNode::Member(member) => self
                .actual(member.receiver)
                .map(|receiver| (receiver, member.member_symbol)),
            ExpressionNode::Name(path) => {
                let symbols = self
                    .program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                let Some((&last, prefix)) = symbols.split_last() else {
                    return None;
                };
                if prefix.is_empty() {
                    return None;
                }
                let Some(reference) =
                    validation::declaration_type_reference(self.program, path.head_symbol)
                else {
                    return None;
                };
                let formation = self
                    .program
                    .state_local_declaration(self.state, path.head_symbol)
                    .map(|(position, _)| position)
                    .unwrap_or(0);
                let mut receiver = Application {
                    reference,
                    formation,
                    substitutions: Vec::new(),
                    arguments: Vec::new(),
                    call: None,
                    position: None,
                };
                for symbol in prefix.iter().skip(1) {
                    let reference = self
                        .program
                        .type_reference_table
                        .peeled_type_reference(receiver.reference);
                    let data = self
                        .program
                        .data_application_definition(reference)
                        .or_else(|| {
                            self.program
                                .type_reference_table
                                .type_head_symbol(reference)
                                .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
                        });
                    if data.is_some_and(|data| self.program.data_members(data).iter().any(|member| {
                        matches!(member, typed_trees::data::DataMember::Variant(case) if case.symbol == *symbol)
                    })) { continue; }
                    let Some(field) = data.and_then(|data| {
                        self.program
                            .data_member_fields(self.program.data_members(data))
                            .find(|field| field.symbol == *symbol)
                    }) else {
                        return None;
                    };
                    receiver.reference = field.type_reference;
                }
                Some((receiver, last))
            }
            _ => None,
        };
        let (receiver, member) = selected?;
        self.descriptor_arguments(&receiver)
            .into_iter()
            .find(|(field, _)| *field == member)
            .map(|(_, argument)| (receiver, argument))
    }

    fn check_descriptor_store(
        &self,
        target: ExpressionHandle,
        value: ExpressionHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let Some((receiver, argument)) = self.descriptor_target(target) {
            self.check_descriptor(value, argument, &receiver, diagnostics);
        }
    }

    /// Whole erased evidence has no runtime storage subject. The source
    /// producer checked this exact closed term against its complete W application,
    /// including every constructor payload and recursive child's next index.
    /// Consume that result only at an erased binding boundary. Recursive
    /// runtime field checking never turns an isolated child into evidence of
    /// the enclosing descriptor relationship.
    fn check_binding(
        &self,
        expression: ExpressionHandle,
        required: Application<'_>,
        erased: bool,
        expected_proof_identity: Option<typed_trees::type_identity::NormalizedTypeIdentity>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let formal_evidence = erased
            && validation::mathematical_applications::is_source_evidence_carrier(
                self.program,
                required.reference,
            )
            && expected_proof_identity.is_some_and(|expected| {
                self.erased_evidence_reference(expression, 0)
                    .is_some_and(|actual| {
                        let payload_owner =
                            match self.program.expression_table.expression(expression) {
                                ExpressionNode::Member(_) => {
                                    validation::declared_case_projection_field(
                                        self.program,
                                        self.machine,
                                        Some(self.state),
                                        expression,
                                    )
                                    .map(|field| self.program.symbols.get(field.symbol).parent)
                                    .unwrap_or_default()
                                }
                                _ => SymbolHandle::invalid(),
                            };
                        if !self.stable_proof_indices(actual, payload_owner, 0) {
                            return false;
                        }
                        self.actual_erased_identity(expression, actual)
                            .is_some_and(|actual| actual.is_valid() && actual == expected)
                    })
            });
        if formal_evidence {
            return;
        }
        let constructor_evidence = erased
            && self.program.prepared_mathematics().is_some_and(|view| {
                view.source_constructor(expression)
                    .is_some_and(|constructor| {
                        constructor.machine == self.machine.symbol
                            && constructor.state == self.state.symbol
                            && constructor.type_identity.is_valid()
                            && constructor.type_identity
                                == self
                                    .program
                                    .normalized_type_identity(constructor.source_type)
                            && expected_proof_identity.is_some_and(|expected| {
                                // Both sides name the caller's value telescope.
                                // The retained ordinary identity above binds the
                                // source row; it has no lexical application scope.
                                typed_trees::type_identity::applied_type_identity(
                                    self.program,
                                    constructor.source_type,
                                    self.machine.symbol,
                                    &[],
                                ) == Some(expected)
                            })
                    })
            });
        if constructor_evidence {
            return;
        }
        let checked_evidence = erased
            && self.independent_of_substitutions(required.reference, &required.substitutions, 0)
            && self.program.prepared_mathematics().is_some_and(|view| {
                view.source_evidence(expression).is_some_and(|evidence| {
                    evidence.term.is_valid()
                        && evidence.ty.is_valid()
                        && view
                            .applications
                            .source_carriers
                            .iter()
                            .any(|(_, carrier)| {
                                carrier.domains.is_empty()
                                    && carrier.type_identity == evidence.type_identity
                                    && carrier.ty == evidence.ty
                            })
                        && evidence.type_identity.is_valid()
                        && evidence.type_identity
                            == self.program.normalized_type_identity(evidence.source_type)
                        && evidence.type_identity
                            == self.program.normalized_type_identity(required.reference)
                })
            });
        if checked_evidence {
            return;
        }
        self.check(expression, required, diagnostics);
    }

    /// An open mathematical value is supplied by an exact erased formal,
    /// including an exact erased payload projection. Its checked source
    /// carrier, rather than a fabricated closed term, owns the type.
    fn erased_evidence_reference(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        if depth > 128 || !expression.is_valid() {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let reference = validation::expression_result_type_reference(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                )?;
                if !validation::mathematical_applications::is_source_evidence_carrier(
                    self.program,
                    reference,
                ) {
                    return None;
                }
                if path.symbol != path.head_symbol
                    || self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let formal = self
                    .program
                    .state_parameter_by_symbol(self.state, path.symbol)?;
                (formal.relevance.is_erased()
                    && !formal.is_mutable
                    && formal.type_reference == reference)
                    .then_some(reference)
            }
            ExpressionNode::Member(member) => {
                let receiver = self.erased_evidence_reference(member.receiver, depth + 1)?;
                let field = validation::declared_case_projection_field(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                )?;
                if !field.relevance.is_erased() {
                    return None;
                }
                if validation::mathematical_applications::is_source_evidence_carrier(
                    self.program,
                    field.type_reference,
                ) {
                    return Some(field.type_reference);
                }
                // A prepared Type application substitutes its payload types.
                // Rejoin this exact selection to the independently formed
                // declaration carrier, not an unregistered applied reference.
                // actual_erased_identity subsequently applies the receiver's
                // complete Type/value tuple and exact payload projections.
                let retained = member.member_symbol;
                if !retained.is_valid()
                    || self.program.symbols.get(retained).kind != symbols::SymbolKind::Field
                {
                    return None;
                }
                let case_symbol = self.program.symbols.get(retained).parent;
                if !case_symbol.is_valid()
                    || self.program.symbols.get(case_symbol).kind != symbols::SymbolKind::Variant
                {
                    return None;
                }
                let owner_symbol = self.program.symbols.get(case_symbol).parent;
                let owner = self
                    .program
                    .unique_data_definition_by_symbol(owner_symbol)?;
                let case = self.program.data_members(owner).iter().find_map(|member| {
                    let case = member.as_variant()?;
                    (case.symbol == case_symbol).then_some(case)
                })?;
                let declared = self.program.data_payload_field_by_symbol(case, retained)?;
                let applied =
                    self.program
                        .data_constructor_field(receiver, owner.symbol, retained)?;
                (declared.relevance.is_erased()
                    && applied.symbol == field.symbol
                    && applied.identity == field.identity
                    && applied.type_reference == field.type_reference
                    && validation::mathematical_applications::is_source_evidence_carrier(
                        self.program,
                        declared.type_reference,
                    ))
                .then_some(declared.type_reference)
            }
            _ => None,
        }
    }

    fn stable_index_symbol(&self, symbol: SymbolHandle, payload_owner: SymbolHandle) -> bool {
        if !symbol.is_valid() {
            return true;
        }
        if self.program.const_declaration_by_symbol(symbol).is_some() {
            return true;
        }
        if self
            .program
            .machine_type_parameters(self.machine)
            .iter()
            .any(|parameter| {
                parameter.symbol == symbol
                    && match parameter.kind {
                        TypeParameterKind::Value { type_reference }
                        | TypeParameterKind::Const { type_reference } => self
                            .program
                            .primitive_type_reference(type_reference)
                            .is_some_and(|primitive| primitive.is_fixed_integer()),
                        _ => false,
                    }
            })
        {
            return true;
        }
        if let Some(parameter) = self.program.state_parameter_by_symbol(self.state, symbol) {
            return !parameter.is_mutable
                && self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer());
        }
        payload_owner.is_valid()
            && self.program.symbols.get(symbol).parent == payload_owner
            && self.program.symbols.get(symbol).kind == symbols::SymbolKind::Field
    }

    fn stable_proof_indices(
        &self,
        reference: TypeReferenceHandle,
        payload_owner: SymbolHandle,
        depth: usize,
    ) -> bool {
        if depth > 128 || !reference.is_valid() {
            return false;
        }
        let table = &self.program.type_reference_table;
        match table.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let Some(data) = self.program.data_definition_by_symbol(*base_symbol) else {
                    return false;
                };
                let parameters = self.program.data_type_parameters(data);
                let arguments = table.type_reference_handles(*arguments);
                parameters.len() == arguments.len()
                    && parameters
                        .iter()
                        .zip(arguments)
                        .all(|(parameter, argument)| {
                            if matches!(
                                parameter.kind,
                                TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
                            ) {
                                match table.type_reference(*argument) {
                                    TypeReferenceNode::Named { symbol, .. } => {
                                        self.stable_index_symbol(*symbol, payload_owner)
                                    }
                                    TypeReferenceNode::ConstExpression(expression) => {
                                        self.stable_proof_argument(*expression, depth + 1)
                                    }
                                    _ => false,
                                }
                            } else {
                                !self.has_indices(*argument, 0)
                                    || self.stable_proof_indices(
                                        *argument,
                                        payload_owner,
                                        depth + 1,
                                    )
                            }
                        })
            }
            TypeReferenceNode::Named { symbol, .. } => self
                .program
                .data_definition_by_symbol(*symbol)
                .and_then(|data| data.generic_instance)
                .is_some_and(|origin| self.stable_proof_indices(origin, payload_owner, depth + 1)),
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.stable_proof_indices(*base_type, payload_owner, depth + 1)
            }
            _ => false,
        }
    }

    fn stable_proof_argument(&self, expression: ExpressionHandle, depth: usize) -> bool {
        if depth > 128 || !expression.is_valid() {
            return false;
        }
        if self.program.prepared_mathematics().is_some_and(|view| {
            view.source_constructor(expression)
                .is_some_and(|constructor| {
                    constructor.machine == self.machine.symbol
                        && constructor.state == self.state.symbol
                        && constructor.schema.is_valid()
                        && constructor.ty.is_valid()
                        && constructor.type_identity.is_valid()
                        && constructor.type_identity
                            == self
                                .program
                                .normalized_type_identity(constructor.source_type)
                })
        }) {
            return true;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.symbol == path.head_symbol =>
            {
                self.stable_index_symbol(path.symbol, SymbolHandle::invalid())
                    || self
                        .erased_evidence_reference(expression, depth + 1)
                        .is_some()
            }
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
            ExpressionNode::Member(member) => {
                self.erased_evidence_reference(member.receiver, depth + 1)
                    .is_some()
                    && validation::declared_case_projection_field(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )
                    .is_some()
            }
            _ => false,
        }
    }

    fn stable_static_proof_argument(
        &self,
        argument: &typed_trees::expression::StaticMachineArgument,
    ) -> bool {
        if argument.symbol.is_valid() {
            return self.stable_index_symbol(argument.symbol, SymbolHandle::invalid());
        }
        if argument.const_literal.is_some() {
            return true;
        }
        if argument.type_reference.is_valid() {
            if let TypeReferenceNode::ConstExpression(expression) = self
                .program
                .type_reference_table
                .type_reference(argument.type_reference)
            {
                return self.stable_proof_argument(*expression, 0);
            }
        }
        typed_trees::typed_trees::static_const_argument_spelling(
            self.program.typed_program(),
            argument,
        )
        .is_some_and(|spelling| {
            spelling.parse::<i128>().is_ok() || CanonicalConstValue::from_atom(&spelling).is_some()
        })
    }

    fn actual_erased_identity(
        &self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<typed_trees::type_identity::NormalizedTypeIdentity> {
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
        let ExpressionNode::Member(member) = self.program.expression_table.expression(expression)
        else {
            return typed_trees::type_identity::applied_type_identity(
                self.program,
                reference,
                self.machine.symbol,
                &[],
            );
        };
        let field = validation::declared_case_projection_field(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        )?;
        let case_symbol = self.program.symbols.get(field.symbol).parent;
        let data_symbol = self.program.symbols.get(case_symbol).parent;
        let data = self.program.data_definition_by_symbol(data_symbol)?;
        let case = self
            .program
            .data_members(data)
            .iter()
            .filter_map(|member| member.as_variant())
            .find(|case| case.symbol == case_symbol)?;
        let fields = self
            .program
            .data_payload_fields(case)
            .iter()
            .map(|field| {
                (
                    field.symbol,
                    IndexIdentityArgument::FieldProjection {
                        receiver: member.receiver,
                        field: field.symbol,
                    },
                )
            })
            .collect::<Vec<_>>();
        let receiver_type = self.erased_evidence_reference(member.receiver, 0)?;
        let table = &self.program.type_reference_table;
        let receiver_type = table.peeled_type_reference(receiver_type);
        let receiver_type = match table.type_reference(receiver_type) {
            TypeReferenceNode::Named { symbol, .. } => self
                .program
                .data_definition_by_symbol(*symbol)
                .and_then(|data| data.generic_instance)
                .unwrap_or(receiver_type),
            _ => receiver_type,
        };
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = table.type_reference(receiver_type)
        else {
            return None;
        };
        if *base_symbol != data.symbol {
            return None;
        }
        let arguments = table.type_reference_handles(*arguments);
        let parameters = self.program.data_type_parameters(data);
        if parameters.len() != arguments.len() {
            return None;
        }
        let indices = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    if matches!(parameter.kind, TypeParameterKind::Type) {
                        IndexIdentityArgument::TypeArgument(*argument)
                    } else {
                        IndexIdentityArgument::TypeReference(*argument)
                    },
                )
            })
            .collect::<Vec<_>>();
        typed_trees::type_identity::applied_type_identity_in_scope(
            self.program,
            reference,
            data.symbol,
            self.machine.symbol,
            &[
                AppliedTypeIdentityScope {
                    declaration: data.symbol,
                    arguments: &indices,
                    lifetimes: &[],
                },
                AppliedTypeIdentityScope {
                    declaration: case.symbol,
                    arguments: &fields,
                    lifetimes: &[],
                },
            ],
        )
        .filter(|identity| identity.is_valid())
    }

    fn expected_erased_identity(
        &self,
        mut reference: TypeReferenceHandle,
        target: SymbolHandle,
        site: &validation::CallSite<'_>,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
        position: usize,
    ) -> Option<typed_trees::type_identity::NormalizedTypeIdentity> {
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
        if parameters.len() != arguments.len()
            || !arguments
                .iter()
                .all(|argument| self.stable_proof_argument(*argument, 0))
        {
            return None;
        }
        let callee = self
            .program
            .machine_by_symbol(target)
            .or_else(|| self.program.machine_holding_state(target))?;
        let (application, explicit) = match site {
            validation::CallSite::Statement(call) => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            validation::CallSite::Expression { call, .. } => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            validation::CallSite::TransitionNamed { .. } => (Default::default(), &[][..]),
        };
        let mut owner = callee.symbol;
        let mut source_parameters_override = None;
        let mut static_parameters = self.program.machine_type_parameters(callee);
        let mut static_arguments = explicit;
        if application.is_valid() {
            let row = self
                .program
                .expression_table
                .static_machine_application(application);
            let source = self
                .program
                .machine_by_symbol(row.registration_operation)
                .or_else(|| {
                    self.program
                        .machine_holding_state(row.registration_operation)
                })?;
            let entry = self.program.machine_states(source).first()?;
            let source_parameters = self.program.state_parameters(entry);
            if source_parameters.len() != parameters.len()
                || !source_parameters.get(position)?.relevance.is_erased()
            {
                return None;
            }
            // The authored proof signature keeps the original telescope even
            // when executable specialization closes another entry's indices.
            reference = source_parameters[position].type_reference;
            source_parameters_override = Some(source_parameters);
            owner = source.symbol;
            static_parameters = self.program.machine_type_parameters(source);
            static_arguments = self
                .program
                .expression_table
                .static_machine_application_arguments(application);
        }
        let parameters = source_parameters_override.unwrap_or(parameters);
        if !validation::mathematical_applications::is_source_evidence_carrier(
            self.program,
            reference,
        ) {
            return None;
        }
        if static_parameters.len() != static_arguments.len() && !static_arguments.is_empty() {
            return None;
        }
        let mut substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(formal, actual)| (formal.symbol, IndexIdentityArgument::Expression(*actual)))
            .collect::<Vec<_>>();
        for (formal, actual) in static_parameters.iter().zip(static_arguments) {
            let value_role = matches!(
                formal.kind,
                TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
            );
            if value_role && !self.stable_static_proof_argument(actual) {
                return None;
            }
            let argument = if value_role && actual.type_reference.is_valid() {
                IndexIdentityArgument::TypeReference(actual.type_reference)
            } else if value_role && actual.symbol.is_valid() {
                IndexIdentityArgument::Subject(actual.symbol)
            } else if matches!(formal.kind, TypeParameterKind::Type)
                && actual.type_reference.is_valid()
            {
                IndexIdentityArgument::TypeArgument(actual.type_reference)
            } else {
                IndexIdentityArgument::StaticArgument(actual)
            };
            substitutions.push((formal.symbol, argument));
        }
        typed_trees::type_identity::applied_type_identity_in_scope(
            self.program,
            reference,
            owner,
            self.machine.symbol,
            &[AppliedTypeIdentityScope {
                declaration: owner,
                arguments: &substitutions,
                lifetimes: &[],
            }],
        )
        .filter(|identity| identity.is_valid())
    }

    /// An equal unspecialized spelling does not prove a dependent call's
    /// actual index. If a receiving index mentions a substituted state formal,
    /// retain the ordinary subject comparison. Closed and generic-static
    /// indices do not depend on those runtime substitutions.
    fn independent_of_substitutions(
        &self,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, Option<ScopedArithmeticValue>)],
        depth: usize,
    ) -> bool {
        if substitutions.is_empty() {
            return true;
        }
        if !reference.is_valid() || depth > 128 {
            return false;
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { arguments, .. } => self
                .program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .all(|argument| {
                    self.independent_of_substitutions(*argument, substitutions, depth + 1)
                }),
            TypeReferenceNode::Named { symbol, .. } => {
                !substitutions
                    .iter()
                    .any(|(parameter, _)| parameter == symbol)
                    && self
                        .program
                        .data_definition_by_symbol(*symbol)
                        .and_then(|data| data.generic_instance)
                        .is_none_or(|origin| {
                            self.independent_of_substitutions(origin, substitutions, depth + 1)
                        })
            }
            // Complex dependent receiving domains keep the existing checked
            // subject comparison rather than borrowing the root's annotation.
            _ => false,
        }
    }

    fn check(
        &self,
        expression: ExpressionHandle,
        required: Application<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.has_indices(required.reference, 0) {
            return;
        }
        if !expression.is_valid() {
            diagnostics.push(Diagnostic::error(
                "runtime indexed default requires an explicit captured construction",
            ));
            return;
        }
        // Contextual formation checks the same receiving obligation at every
        // produced leaf. A branch, aggregate, or cast cannot hide an old
        // indexed value behind its destination's annotation.
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                self.check(atomic.value, required, diagnostics);
                return;
            }
            ExpressionNode::Match(dispatch) => {
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    self.check(arm.value, required.clone(), diagnostics);
                }
                return;
            }
            ExpressionNode::Cast(cast) => {
                self.check(
                    cast.value,
                    Application {
                        reference: cast.target_type,
                        formation: self.statement,
                        substitutions: Vec::new(),
                        arguments: Vec::new(),
                        call: None,
                        position: None,
                    },
                    diagnostics,
                );
                self.check(cast.value, required, diagnostics);
                return;
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let reference = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(required.reference);
                if let TypeReferenceNode::FixedArray { element_type, .. } =
                    self.program.type_reference_table.type_reference(reference)
                {
                    for element in self.program.expression_table.expression_handles(*elements) {
                        self.check(
                            *element,
                            Application {
                                reference: *element_type,
                                ..required.clone()
                            },
                            diagnostics,
                        );
                    }
                    return;
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                let reference = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(required.reference);
                let data = self
                    .program
                    .data_application_definition(reference)
                    .or_else(|| {
                        self.program
                            .type_reference_table
                            .type_head_symbol(reference)
                            .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
                    });
                if let Some(data) = data {
                    let fields = self.program.expression_table.struct_fields(literal.fields);
                    // One authored roster for both obligations below: the
                    // construction's common fields and the payload fields of
                    // the case it selects.
                    let declared = self.program.data_construction_fields(
                        self.program.data_members(data),
                        literal.case_symbol,
                    );
                    for declared in &declared {
                        if self.has_indices(declared.type_reference, 0)
                            && !fields
                                .iter()
                                .any(|field| field.field_symbol == declared.symbol)
                        {
                            diagnostics.push(Diagnostic::error(
                                "runtime indexed field requires an explicit captured construction",
                            ));
                        }
                    }
                    for (symbol, argument) in self.descriptor_arguments(&required) {
                        if let Some(field) =
                            fields.iter().find(|field| field.field_symbol == symbol)
                        {
                            self.check_descriptor(field.value, argument, &required, diagnostics);
                        } else {
                            diagnostics.push(Diagnostic::error(
                                "runtime indexed construction has no captured descriptor",
                            ));
                        }
                    }
                    for field in fields {
                        let field_type = declared
                            .iter()
                            .find(|declared| declared.symbol == field.field_symbol)
                            .map(|declared| declared.type_reference);
                        if let Some(reference) = field_type {
                            self.check(
                                field.value,
                                Application {
                                    reference,
                                    ..required.clone()
                                },
                                diagnostics,
                            );
                        } else {
                            diagnostics.push(Diagnostic::error("runtime indexed construction has no exact substituted field telescope"));
                        }
                    }
                    return;
                }
            }
            _ => {}
        }
        let Some(actual) = self.actual(expression) else {
            diagnostics.push(
                Diagnostic::error("runtime indexed value has no retained subject provenance")
                    .with_source_span(self.program.expression_table.source_span(expression)),
            );
            return;
        };
        if !self.compare(&actual, &required, 0) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` cannot prove runtime index subject equality at statement {}; an indexed value retains its captured index",
                self.machine.name, self.state.name, self.statement,
            )).with_source_span(self.program.expression_table.source_span(expression)));
        }
    }
}

pub(super) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    frames: Option<&CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let program_has_runtime_indices = program.data_definitions().iter().any(|data| {
        program
            .data_type_parameters(data)
            .iter()
            .any(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
    });
    if !program_has_runtime_indices {
        return;
    }
    for (_, flow) in facts.flow.control.states.iter() {
        let Some(machine) = program.machine_by_symbol(flow.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(flow.state_symbol) else {
            continue;
        };
        for (statement, node) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let scope = Scope {
                program,
                machine,
                state,
                statement,
                target: TransitionTargetHandle::invalid(),
                frames,
            };
            // A scalar loan would discard the descriptor/index relationship.
            // Whole indexed loans retain that relationship in their referee type.
            let mut roots = Vec::new();
            validation::expression_traversal::collect_statement_expression_trees(
                program, node, &mut roots,
            );
            let mut visited = Vec::new();
            for root in roots {
                validation::expression_traversal::visit_post_order(
                    program,
                    root,
                    &mut visited,
                    |expression, node| {
                        if let ExpressionNode::Borrow(borrow) = node {
                            if borrow.access != language_core::ReferenceAccess::Shared
                                && scope.descriptor_target(borrow.target).is_some()
                            {
                                diagnostics.push(Diagnostic::error("mutable scalar loan cannot preserve a runtime index descriptor")
                                .with_source_span(program.expression_table.source_span(expression)));
                            }
                        }
                    },
                );
            }
            match node {
                StatementNode::LocalData(local) => scope.check_binding(
                    local.initial_value,
                    Application {
                        reference: local.type_reference,
                        formation: statement,
                        substitutions: Vec::new(),
                        arguments: Vec::new(),
                        call: None,
                        position: None,
                    },
                    local.relevance.is_erased(),
                    typed_trees::type_identity::applied_type_identity(
                        program,
                        local.type_reference,
                        machine.symbol,
                        &[],
                    ),
                    diagnostics,
                ),
                StatementNode::Expression(expression) if state.return_type.is_valid() => scope
                    .check(
                        *expression,
                        Application {
                            reference: state.return_type,
                            formation: 0,
                            substitutions: Vec::new(),
                            arguments: Vec::new(),
                            call: None,
                            position: None,
                        },
                        diagnostics,
                    ),
                StatementNode::Assignment(assignment) => {
                    scope.check_descriptor_store(assignment.target, assignment.value, diagnostics);
                    if let Some(required) = scope.actual(assignment.target) {
                        scope.check(assignment.value, required, diagnostics);
                    }
                }
                StatementNode::Transition(transition) => {
                    for target in [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| target.is_valid())
                    {
                        if let TransitionTargetNode::Value(value) =
                            program.statement_table.transition_target(target)
                        {
                            let scope = Scope { target, ..scope };
                            scope.check(
                                *value,
                                Application {
                                    reference: state.return_type,
                                    formation: 0,
                                    substitutions: Vec::new(),
                                    arguments: Vec::new(),
                                    call: None,
                                    position: None,
                                },
                                diagnostics,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        for call in facts.flow.control.calls.span_or_empty(flow.calls) {
            let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
            else {
                continue;
            };
            let Some(site) = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let arguments = validation::call_site_argument_expressions(program, &site);
            let scope = Scope {
                program,
                machine,
                state,
                statement: call.statement_index,
                target: TransitionTargetHandle::invalid(),
                frames,
            };
            let (substitutions, captured_arguments) =
                scope.argument_substitutions(parameters, &arguments);
            for (position, (parameter, argument)) in
                parameters.iter().zip(arguments.iter()).enumerate()
            {
                scope.check_binding(
                    *argument,
                    Application {
                        reference: parameter.type_reference,
                        formation: call.statement_index,
                        substitutions: substitutions.clone(),
                        arguments: captured_arguments.clone(),
                        call: Some(site),
                        position: Some(position),
                    },
                    parameter.relevance.is_erased(),
                    (parameter.relevance.is_erased()
                        && validation::mathematical_applications::is_source_evidence_carrier(
                            program,
                            parameter.type_reference,
                        ))
                    .then(|| {
                        scope.expected_erased_identity(
                            parameter.type_reference,
                            call.target_symbol,
                            &site,
                            parameters,
                            &arguments,
                            position,
                        )
                    })
                    .flatten(),
                    diagnostics,
                );
            }
        }
    }
}

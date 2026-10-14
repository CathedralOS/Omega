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
struct Application {
    reference: TypeReferenceHandle,
    formation: usize,
    substitutions: Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
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
        application: &Application,
    ) -> Option<ScopedArithmeticValue> {
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
    ) -> Vec<(SymbolHandle, Option<ScopedArithmeticValue>)> {
        let inert = arguments.iter().all(|argument| {
            validation::expression_traversal::inert_value_tree_with_calls(
                self.program,
                *argument,
                |call| {
                    self.frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(self.machine, call)
                            .complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    })
                },
            )
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
                (parameter.symbol, value)
            })
            .collect()
    }

    fn actual(&self, expression: ExpressionHandle) -> Option<Application> {
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
                })
            }
            ExpressionNode::Call(call) => {
                let parameters =
                    validation::call_target_parameters(self.program, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                let substitutions = self.argument_substitutions(parameters, arguments);
                Some(Application {
                    reference: validation::call_target_return_type(
                        self.program,
                        call.target_symbol,
                    )?,
                    formation: self.statement,
                    substitutions,
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

    fn compare(&self, actual: &Application, required: &Application, depth: usize) -> bool {
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
                            let Some(left) = self.index(*left, actual) else {
                                return false;
                            };
                            let Some(right) = self.index(*right, required) else {
                                return false;
                            };
                            left == right
                                || self.values(self.statement).is_some_and(|values| {
                                    values.proves_values_relation(
                                        self.program,
                                        self.machine,
                                        BinaryOperator::Equal,
                                        &left,
                                        &right,
                                    )
                                })
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
        application: &Application,
    ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
        let reference = self
            .program
            .type_reference_table
            .peeled_type_reference(application.reference);
        let concrete = match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                self.program.data_definition_by_symbol(*symbol)
            }
            _ => None,
        };
        let reference = concrete
            .and_then(|data| data.generic_instance)
            .unwrap_or(reference);
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = self.program.type_reference_table.type_reference(reference)
        else {
            return Vec::new();
        };
        let Some(data) = self.program.data_definition_by_symbol(*base_symbol) else {
            return Vec::new();
        };
        let arguments = self
            .program
            .type_reference_table
            .type_reference_handles(*arguments);
        let parameters = self.program.data_type_parameters(data);
        self.program
            .data_value_parameter_fields(data)
            .enumerate()
            .filter_map(|(descriptor_ordinal, (parameter, field))| {
                let ordinal = parameters
                    .iter()
                    .position(|candidate| candidate.symbol == parameter.symbol)?;
                let field = if let Some(concrete) = concrete {
                    self.program
                        .data_members(concrete)
                        .get(descriptor_ordinal)?
                        .as_field()?
                } else {
                    field
                };
                Some((field.symbol, *arguments.get(ordinal)?))
            })
            .collect()
    }

    fn check_descriptor(
        &self,
        value: ExpressionHandle,
        argument: TypeReferenceHandle,
        application: &Application,
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
    ) -> Option<(Application, TypeReferenceHandle)> {
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
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.symbol == *symbol =>
                                {
                                    Some(field)
                                }
                                typed_trees::data::DataMember::Variant(case) => self
                                    .program
                                    .data_payload_fields(case)
                                    .iter()
                                    .find(|field| field.symbol == *symbol),
                                _ => None,
                            })
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

    fn check(
        &self,
        expression: ExpressionHandle,
        required: Application,
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
                    for member in self.program.data_members(data) {
                        let declared = match member {
                            typed_trees::data::DataMember::Field(field) => {
                                std::slice::from_ref(field)
                            }
                            typed_trees::data::DataMember::Variant(case)
                                if Some(case.symbol) == literal.case_symbol =>
                            {
                                self.program.data_payload_fields(case)
                            }
                            _ => &[],
                        };
                        for declared in declared {
                            if self.has_indices(declared.type_reference, 0)
                                && !fields
                                    .iter()
                                    .any(|field| field.field_symbol == declared.symbol)
                            {
                                diagnostics.push(Diagnostic::error("runtime indexed field requires an explicit captured construction"));
                            }
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
                    for field in self.program.expression_table.struct_fields(literal.fields) {
                        let mut field_type = None;
                        for member in self.program.data_members(data) {
                            match member {
                                typed_trees::data::DataMember::Field(declared)
                                    if declared.symbol == field.field_symbol =>
                                {
                                    field_type = Some(declared.type_reference)
                                }
                                typed_trees::data::DataMember::Variant(case)
                                    if Some(case.symbol) == literal.case_symbol =>
                                {
                                    field_type = self
                                        .program
                                        .data_payload_fields(case)
                                        .iter()
                                        .find(|declared| declared.symbol == field.field_symbol)
                                        .map(|declared| declared.type_reference)
                                        .or(field_type);
                                }
                                _ => {}
                            }
                        }
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
                StatementNode::LocalData(local) => scope.check(
                    local.initial_value,
                    Application {
                        reference: local.type_reference,
                        formation: statement,
                        substitutions: Vec::new(),
                    },
                    diagnostics,
                ),
                StatementNode::Expression(expression) if state.return_type.is_valid() => scope
                    .check(
                        *expression,
                        Application {
                            reference: state.return_type,
                            formation: 0,
                            substitutions: Vec::new(),
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
            let substitutions = scope.argument_substitutions(parameters, &arguments);
            for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
                scope.check(
                    *argument,
                    Application {
                        reference: parameter.type_reference,
                        formation: call.statement_index,
                        substitutions: substitutions.clone(),
                    },
                    diagnostics,
                );
            }
        }
    }
}

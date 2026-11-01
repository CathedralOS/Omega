//! Application value indices describe captured values, not mutable storage names.
//!
//! Const bindings retain immutable invocation coordinates; runtime Value bindings
//! additionally owe descriptor capture and freshness. A type at a local formation
//! captures the scalar roster at that statement;
//! a parameter captures arrival. Every store, result and call compares those
//! independent snapshots. CurrentScalarValues owns assignment/effect transfer
//! and checked equality, so changing the source variable never retags an old
//! object. These are erased checking obligations, not runtime proof storage.
//!
//! Region operands of an application compare by their identity in the current
//! machine. A call that authors no lifetime rows does not name such regions in
//! its signature types: inference instantiates the callee's binders to the
//! regions of the arguments, which `region_operand` reads back from them.

use super::current_scalar_values::CurrentScalarValues;
use crate::flow::state_statements;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{DataMember, TypeParameter, TypeParameterKind};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};
use typed_trees::static_applications::StaticMachineApplication;
use typed_trees::static_applications::StaticMachineApplicationHandle;
use typed_trees::type_identity::NormalizedTypeIdentity;
use typed_trees::type_identity::StaticOperandIdentityRequest;
use typed_trees::type_identity::applied_index_identity;
use typed_trees::type_identity::applied_type_identity;
use typed_trees::type_identity::applied_type_identity_in_scope;
use typed_trees::typed_trees::static_const_argument_spelling;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::expression_traversal::collect_statement_expression_trees;
use validation::expression_traversal::expressions_before_occurrence;
use validation::expression_traversal::visit_post_order;
use validation::mathematical_applications::is_source_evidence_carrier;
use validation::{
    CallFrameResolver, CallSite, CheckingInput, ScopedArithmeticValue,
    call_site_argument_expressions, call_target_parameters, call_target_return_type,
    captured_call_argument_index_matches, captured_call_index_value, declaration_type_reference,
    declared_case_projection_field, expression_result_type_reference, find_call_site,
};

#[derive(Clone)]
struct Application<'program> {
    reference: TypeReferenceHandle,
    formation: usize,
    substitutions: Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
    arguments: Vec<(SymbolHandle, Option<ExpressionHandle>)>,
    call: Option<CallSite<'program>>,
    position: Option<usize>,
}

impl Application<'_> {
    /// A bare applied reference: an exact type at its formation statement,
    /// carrying no substitutions, arguments, call site or parameter position
    /// until a caller installs them.
    fn bare(reference: TypeReferenceHandle, formation: usize) -> Self {
        Self {
            reference,
            formation,
            substitutions: Vec::new(),
            arguments: Vec::new(),
            call: None,
            position: None,
        }
    }
}

/// A region operand as this comparison sees it.
enum Region {
    /// Names a region of the current machine.
    Known(typed_trees::type_identity::NormalizedTypeIdentity),
    /// Instantiated by call inference to no single region the arguments name,
    /// so no obligation compares it here.
    Inferred,
}

/// One edge followed from a type reference to a node that binds region operands.
#[derive(Clone, Copy)]
enum RegionStep {
    Pointee,
    Element,
    Argument(usize),
}

struct Scope<'a> {
    program: &'a dyn CheckingInput,
    machine: &'a Machine,
    state: &'a State,
    statement: usize,
    target: TransitionTargetHandle,
    frames: Option<&'a CallFrameResolver<'a>>,
}

impl Scope<'_> {
    fn expression_is_const(
        &self,
        expr: ExpressionHandle,
        static_parameters: &[TypeParameter],
    ) -> bool {
        let is_const_symbol = |symbol: SymbolHandle| {
            !symbol.is_valid()
                || self.program.const_declaration_by_symbol(symbol).is_some()
                || self
                    .program
                    .machine_type_parameters(self.machine)
                    .iter()
                    .chain(static_parameters)
                    .any(|p| {
                        p.symbol == symbol && matches!(p.kind, TypeParameterKind::Const { .. })
                    })
        };
        match self.program.expression_table.expression(expr) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => true,
            ExpressionNode::Name(name) => is_const_symbol(name.symbol),
            ExpressionNode::Unary(u) => self.expression_is_const(u.operand, static_parameters),
            ExpressionNode::Binary(b) => {
                self.expression_is_const(b.left, static_parameters)
                    && self.expression_is_const(b.right, static_parameters)
            }
            ExpressionNode::Cast(c) => self.expression_is_const(c.value, static_parameters),
            ExpressionNode::Call(call) => call.machine_arguments.iter().all(|arg| {
                (!arg.symbol.is_valid() || is_const_symbol(arg.symbol))
                    && (arg.const_literal.is_some()
                        || !arg.type_reference.is_valid()
                        || self
                            .program
                            .type_reference_table
                            .anonymous_numeric_literal_value(arg.type_reference)
                            .is_some())
            }),
            _ => false,
        }
    }

    fn static_index(
        &self,
        argument: TypeReferenceHandle,
        application: &Application<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        let source = self.application_source(application);
        let static_parameters = source.map_or(&[][..], |(_, source)| {
            self.program
                .declaration_type_parameters(source.binding_declaration)
        });
        let is_const_symbol = |symbol: SymbolHandle| {
            !symbol.is_valid()
                || self.program.const_declaration_by_symbol(symbol).is_some()
                || self
                    .program
                    .machine_type_parameters(self.machine)
                    .iter()
                    .chain(static_parameters)
                    .any(|p| {
                        p.symbol == symbol && matches!(p.kind, TypeParameterKind::Const { .. })
                    })
        };
        let table = &self.program.type_reference_table;
        let eligible = match table.type_reference(argument) {
            TypeReferenceNode::Named { symbol, .. } => is_const_symbol(*symbol),
            TypeReferenceNode::ConstExpression(expression) => {
                self.expression_is_const(*expression, static_parameters)
            }
            _ => false,
        };
        if !eligible {
            return None;
        }
        let argument = typed_trees::type_identity::IndexIdentityArgument::TypeReference(argument);
        let (Some((handle, source)), Some(site)) = (source, application.call) else {
            return applied_index_identity(self.program, argument, self.machine.symbol, &[]);
        };
        if !source.binding_declaration.is_valid() || !source.lexical_owner.is_valid() {
            return None;
        }
        use crate::facts::index_compatibility::{
            BoundIndexArgument, call_index_substitutions, index_identity_bindings,
        };
        // Even exact immutable indices belong to the incoming call frame.
        // In a recursive permutation the callee and caller reuse declaration
        // symbols; comparing those symbols before substitution would erase
        // the independently supplied actuals. Runtime subjects stay with the
        // captured-value comparison rather than this static fast path.
        let mut substitutions = call_index_substitutions(self.program, self.machine.symbol, &site);
        substitutions.retain(|(_, argument)| {
            matches!(
                argument,
                BoundIndexArgument::Reference(_) | BoundIndexArgument::Algebra(_)
            )
        });
        if static_parameters.iter().any(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Const { .. })
                && !substitutions
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
        }) {
            return None;
        }
        let arguments = index_identity_bindings(&substitutions);
        let lifetimes = self.application_lifetimes(handle);
        let scopes = [typed_trees::type_identity::AppliedTypeIdentityScope {
            declaration: source.binding_declaration,
            arguments: &arguments,
            lifetimes: &lifetimes,
        }];
        applied_index_identity(self.program, argument, source.lexical_owner, &scopes)
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
            let prefix_unchanged = if let CallSite::Expression { expression, .. } = site {
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
                expressions_before_occurrence(self.program, &roots, expression).is_some_and(
                    |prefix| {
                        prefix.iter().all(|expression| {
                            self.frames.is_some_and(|frames| {
                                frames
                                    .expression_write_frame(self.machine, *expression)
                                    .complete_paths()
                                    .is_some_and(|paths| paths.is_empty())
                            })
                        })
                    },
                )
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
                let parameters = call_target_parameters(self.program, target)?;
                return captured_call_index_value(
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
            if let Some(values) = self.values(application.formation)
                && let Some(term) = values.application_value_with_captured(
                    expression,
                    &application.arguments,
                    &application.substitutions,
                )
            {
                return Some(ScopedArithmeticValue::Term(term));
            }
            if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
                && let Some(callee) = self
                    .program
                    .machine_by_own_or_state_symbol(call.target_symbol)
                && let Some(entry) = self.program.machine_states(callee).first()
            {
                let statements = state_statements(self.program, entry.statement_nodes);
                let return_expr = statements.last().and_then(|s| match s {
                    StatementNode::Expression(e) => Some(*e),
                    _ => None,
                });
                if let Some(return_expr) = return_expr {
                    let callee_params = self.program.machine_type_parameters(callee);
                    let mut callee_substitutions = Vec::new();
                    for (param, actual_arg) in
                        callee_params.iter().zip(call.machine_arguments.iter())
                    {
                        let value = if actual_arg.symbol.is_valid() {
                            application
                                .substitutions
                                .iter()
                                .find(|(s, _)| *s == actual_arg.symbol)
                                .and_then(|(_, v)| v.clone())
                        } else if let Some(lit) = &actual_arg.const_literal {
                            lit.value_bignum().map(ScopedArithmeticValue::Integer)
                        } else if actual_arg.type_reference.is_valid() {
                            self.program
                                .type_reference_table
                                .anonymous_numeric_literal_value(actual_arg.type_reference)
                                .map(|v| {
                                    ScopedArithmeticValue::Integer(BigInt::from_i128(v as i128))
                                })
                        } else {
                            None
                        };
                        callee_substitutions.push((param.symbol, value));
                    }
                    if let Some(values) = self.values(application.formation)
                        && let Some(term) = values.application_value_with_captured(
                            return_expr,
                            &[],
                            &callee_substitutions,
                        )
                    {
                        return Some(ScopedArithmeticValue::Term(term));
                    }
                    if let ExpressionNode::Name(path) =
                        self.program.expression_table.expression(return_expr)
                        && let Some((_, Some(val))) =
                            callee_substitutions.iter().find(|(s, _)| *s == path.symbol)
                    {
                        return Some(val.clone());
                    }
                }
            }
            return None;
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
                    DecodedCanonicalConstValue::Integer { value, .. } => {
                        Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                    }
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
            return Some(ScopedArithmeticValue::Integer(BigInt::from_i128(integer)));
        }
        match CanonicalConstValue::from_atom(name.as_str())?.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => {
                Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
            }
            _ => None,
        }
    }

    /// A call whose arguments all write no completed path is inert: every
    /// substitution stays at its current home instead of a projected one.
    fn arguments_inert(&self, arguments: &[ExpressionHandle]) -> bool {
        arguments.iter().all(|argument| {
            self.frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, *argument)
                    .complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        })
    }

    fn argument_substitutions(
        &self,
        parameters: &[StateParameter],
        arguments: &[ExpressionHandle],
    ) -> (
        Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
        Vec<(SymbolHandle, Option<ExpressionHandle>)>,
    ) {
        let inert = self.arguments_inert(arguments);
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

    fn call_substitutions(
        &self,
        site: CallSite<'_>,
        parameters: &[StateParameter],
        arguments: &[ExpressionHandle],
    ) -> (
        Vec<(SymbolHandle, Option<ScopedArithmeticValue>)>,
        Vec<(SymbolHandle, Option<ExpressionHandle>)>,
    ) {
        let (mut substitutions, captured_arguments) =
            self.argument_substitutions(parameters, arguments);
        let inert = self.arguments_inert(arguments);
        let target = site.target_symbol(self.program);
        let static_arguments = match site {
            CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            CallSite::Statement(call) => call.machine_arguments.as_ref(),
            CallSite::TransitionNamed { .. } => &[][..],
        };
        if let Some(signature) = self
            .program
            .dynamic_slot_by_symbol(target)
            .and_then(|slot| self.program.dynamic_slot_requirement(slot))
            .or_else(|| {
                self.program
                    .trait_machine_signature(target)
                    .map(|(_, signature)| signature)
            })
        {
            let locals = self.program.state_signature_type_parameters(signature);
            if locals.len() == static_arguments.len() {
                let values = self.values(self.statement);
                for (formal, actual) in locals.iter().zip(static_arguments.iter()) {
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
        if let Some(callee) = self.program.machine_by_own_or_state_symbol(target)
            && let Some(values) = self.values(self.statement)
            && let Some(bindings) = values.machine_value_bindings(callee, site)
        {
            // Generic value coordinates share the ordinary call's
            // substitution. Effectful operands require the completed
            // operand projection rather than a current-home read.
            substitutions.extend(
                bindings
                    .into_iter()
                    .map(|(symbol, value)| (symbol, inert.then_some(value))),
            );
        }

        (substitutions, captured_arguments)
    }

    fn actual(&self, expression: ExpressionHandle) -> Option<Application<'_>> {
        if !expression.is_valid() {
            return None;
        }
        let reference =
            expression_result_type_reference(self.program, self.machine, self.state, expression)?;
        if self
            .program
            .expression_const_declaration(expression)
            .is_some()
        {
            return Some(Application::bare(reference, 0));
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast)
                if cast.form.is_recast()
                    && !self
                        .program
                        .type_application_value_indices(reference)
                        .has_runtime_values() =>
            {
                // The prepared recast produces this exact loan application.
                // Its backing place is not a value of the target shape; the
                // recast judgment separately owes representation and authority.
                // Runtime value indices still require source capture below.
                Some(Application::bare(reference, self.statement))
            }
            ExpressionNode::Name(path) => {
                let formation = self
                    .program
                    .state_local_declaration(self.state, path.head_symbol)
                    .map(|(position, _)| position)
                    .unwrap_or(0);
                Some(Application::bare(reference, formation))
            }
            ExpressionNode::Call(call) => {
                let parameters = call_target_parameters(self.program, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                let site = CallSite::Expression { expression, call };
                let (substitutions, captured_arguments) =
                    self.call_substitutions(site, parameters, arguments);

                Some(Application {
                    // Interpret the declaration under captured formal bindings.
                    // A caller-applied index could otherwise bypass the prefix
                    // guard by ceasing to name any of those formal binders.
                    reference: call_target_return_type(self.program, call.target_symbol)?,
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

    fn application_source(
        &self,
        application: &Application<'_>,
    ) -> Option<(StaticMachineApplicationHandle, &StaticMachineApplication)> {
        application.call.and_then(|site| {
            let handle = match site {
                CallSite::Statement(call) => call.authored_machine_application,
                CallSite::Expression { call, .. } => call.authored_machine_application,
                CallSite::TransitionNamed { .. } => Default::default(),
            };
            handle.is_valid().then(|| {
                (
                    handle,
                    self.program
                        .expression_table
                        .static_machine_application(handle),
                )
            })
        })
    }

    fn application_lifetimes(
        &self,
        handle: StaticMachineApplicationHandle,
    ) -> Vec<(String, String)> {
        self.program
            .expression_table
            .static_machine_application_lifetimes(handle)
            .iter()
            .map(|binding| (binding.parameter.to_string(), binding.argument.to_string()))
            .collect()
    }

    fn application_lifetime_identity(
        &self,
        application: &Application<'_>,
        reference: TypeReferenceHandle,
        ordinal: usize,
    ) -> Option<NormalizedTypeIdentity> {
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, applied_type_lifetime_identity_in_scope,
        };
        let source = self.application_source(application);
        let Some((handle, source)) = source else {
            return applied_type_lifetime_identity_in_scope(
                self.program,
                reference,
                ordinal,
                self.machine.symbol,
                self.machine.symbol,
                &[],
            );
        };
        if !source.binding_declaration.is_valid() || !source.lexical_owner.is_valid() {
            return None;
        }
        let lifetimes = self.application_lifetimes(handle);
        let scopes = [AppliedTypeIdentityScope {
            declaration: source.binding_declaration,
            arguments: &[],
            lifetimes: &lifetimes,
        }];
        applied_type_lifetime_identity_in_scope(
            self.program,
            reference,
            ordinal,
            source.binding_declaration,
            source.lexical_owner,
            &scopes,
        )
    }

    /// The region operand `ordinal` of `reference` in this machine's terms.
    ///
    /// A call that authors no lifetime rows leaves its callee's region binders
    /// to inference: its formal types and its result type name regions of the
    /// callee's scope, so resolving them here would match them against nothing
    /// (the application may carry no binding declaration at all). Inference
    /// instantiates a binder to the regions of the arguments it types, so the
    /// operand is read from those arguments instead. A binder that no
    /// argument types, or that arguments type with different regions, is left
    /// to the borrow checker and carries no obligation to this comparison.
    /// Authored rows, declared types and locals keep their exact identity.
    fn region_operand(
        &self,
        application: &Application<'_>,
        reference: TypeReferenceHandle,
        ordinal: usize,
        depth: usize,
    ) -> Option<Region> {
        let rowless_call = application.call.is_some()
            && self
                .application_source(application)
                .is_none_or(|(handle, _)| self.application_lifetimes(handle).is_empty());
        if !rowless_call {
            return self
                .application_lifetime_identity(application, reference, ordinal)
                .map(Region::Known);
        }
        let site = application.call?;
        let TypeReferenceNode::Generic {
            lifetime_arguments, ..
        } = self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let name = lifetime_arguments.get(ordinal)?;
        if depth > 16 {
            return Some(Region::Inferred);
        }
        let parameters =
            validation::call_target_parameters(self.program, site.target_symbol(self.program))?;
        let arguments = validation::call_site_argument_expressions(self.program, &site);
        let mut supplied: Option<typed_trees::type_identity::NormalizedTypeIdentity> = None;
        for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
            let mut occurrences = Vec::new();
            self.region_binder_paths(
                parameter.type_reference,
                name.as_str(),
                &mut Vec::new(),
                &mut occurrences,
            );
            if occurrences.is_empty() {
                continue;
            }
            let Some(actual) = self.actual(*argument) else {
                continue;
            };
            for (path, binder_ordinal) in occurrences {
                let Some(node) = self.region_node_at(actual.reference, &path) else {
                    continue;
                };
                match self.region_operand(&actual, node, binder_ordinal, depth + 1) {
                    Some(Region::Known(identity)) => match supplied {
                        Some(earlier) if earlier != identity => return Some(Region::Inferred),
                        _ => supplied = Some(identity),
                    },
                    Some(Region::Inferred) | None => {}
                }
            }
        }
        Some(supplied.map_or(Region::Inferred, Region::Known))
    }

    /// Every generic node of `reference` that binds the region operand `name`,
    /// as the path of edges reaching it and the operand's ordinal there.
    fn region_binder_paths(
        &self,
        reference: TypeReferenceHandle,
        name: &str,
        path: &mut Vec<RegionStep>,
        occurrences: &mut Vec<(Vec<RegionStep>, usize)>,
    ) {
        if path.len() > 16 {
            return;
        }
        let table = &self.program.type_reference_table;
        match table.type_reference(self.region_carrier(reference)) {
            TypeReferenceNode::Generic {
                lifetime_arguments,
                arguments,
                ..
            } => {
                for (ordinal, argument) in lifetime_arguments.iter().enumerate() {
                    if argument.as_str() == name {
                        occurrences.push((path.clone(), ordinal));
                    }
                }
                for (position, argument) in
                    table.type_reference_handles(*arguments).iter().enumerate()
                {
                    path.push(RegionStep::Argument(position));
                    self.region_binder_paths(*argument, name, path, occurrences);
                    path.pop();
                }
            }
            TypeReferenceNode::Reference { referee, .. } => {
                path.push(RegionStep::Pointee);
                self.region_binder_paths(*referee, name, path, occurrences);
                path.pop();
            }
            TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. } => {
                path.push(RegionStep::Element);
                self.region_binder_paths(*element_type, name, path, occurrences);
                path.pop();
            }
            _ => {}
        }
    }

    /// The node reached by following `path` from `reference`, if the types
    /// have the same shape along it.
    fn region_node_at(
        &self,
        reference: TypeReferenceHandle,
        path: &[RegionStep],
    ) -> Option<TypeReferenceHandle> {
        let table = &self.program.type_reference_table;
        let mut current = self.region_carrier(reference);
        for step in path {
            current = self.region_carrier(match (step, table.type_reference(current)) {
                (RegionStep::Pointee, TypeReferenceNode::Reference { referee, .. }) => *referee,
                (
                    RegionStep::Element,
                    TypeReferenceNode::Slice { element_type }
                    | TypeReferenceNode::FixedArray { element_type, .. },
                ) => *element_type,
                (RegionStep::Argument(position), TypeReferenceNode::Generic { arguments, .. }) => {
                    *table.type_reference_handles(*arguments).get(*position)?
                }
                _ => return None,
            });
        }
        Some(current)
    }

    /// `reference` without domain constraints or the specialization wrapper
    /// of a generic data instance, matching what `compare` looks through.
    fn region_carrier(&self, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
        let table = &self.program.type_reference_table;
        for _ in 0..16 {
            reference = table.unconstrained_type_reference(reference);
            let TypeReferenceNode::Named { symbol, .. } = table.type_reference(reference) else {
                break;
            };
            let Some(origin) = self
                .program
                .data_definition_by_symbol(*symbol)
                .and_then(|data| data.generic_instance)
                .filter(|origin| *origin != reference)
            else {
                break;
            };
            reference = origin;
        }
        reference
    }

    fn compare(
        &self,
        actual: &Application<'_>,
        required: &Application<'_>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        if depth > 128 {
            return false;
        }
        let table = &self.program.type_reference_table;
        let contextual = |reference| {
            table.unconstrained_type_reference(
                self.program
                    .statically_refined_type_reference(expression, reference),
            )
        };
        let actual_reference = contextual(actual.reference);
        let required_reference = contextual(required.reference);
        let instance_origin = |reference| {
            let symbol = match table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if arguments.count() == 0 => *base_symbol,
                _ => return None,
            };
            self.program
                .data_definition_by_symbol(symbol)?
                .generic_instance
                .filter(|origin| *origin != reference)
        };
        let exact = |left, right| {
            let left = applied_type_identity(self.program, left, self.machine.symbol, &[]);
            let right = applied_type_identity(self.program, right, self.machine.symbol, &[]);
            left.is_some() && left == right
        };
        let recurse = |left, right| {
            let mut actual = actual.clone();
            actual.reference = left;
            let mut required = required.clone();
            required.reference = right;
            self.compare(&actual, &required, expression, depth + 1)
        };
        if let Some(origin) = instance_origin(actual_reference) {
            return recurse(origin, required_reference);
        }
        if let Some(origin) = instance_origin(required_reference) {
            return recurse(actual_reference, origin);
        }
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
                    lifetime_arguments: left_lifetimes,
                    arguments: left,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_base,
                    lifetime_arguments: right_lifetimes,
                    arguments: right,
                    ..
                },
            ) if left_base == right_base => {
                // Contextual value equality never erases independent region
                // operands, including phantom regions on selected code.
                if left_lifetimes.len() != right_lifetimes.len()
                    || !(0..left_lifetimes.len()).all(|ordinal| {
                        match (
                            self.region_operand(actual, actual_reference, ordinal, 0),
                            self.region_operand(required, required_reference, ordinal, 0),
                        ) {
                            (Some(Region::Known(left)), Some(Region::Known(right))) => {
                                left == right
                            }
                            (Some(_), Some(_)) => true,
                            _ => false,
                        }
                    })
                {
                    return false;
                }
                let parameters = self.program.declaration_type_parameters(*left_base);
                let left = table.type_reference_handles(*left);
                let right = table.type_reference_handles(*right);
                if parameters.len() != left.len() || parameters.len() != right.len() {
                    return false;
                }
                parameters
                    .iter()
                    .zip(left.iter().zip(right))
                    .all(|(parameter, (left, right))| {
                        if matches!(
                            parameter.kind,
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                        ) {
                            if let (Some(left), Some(right)) =
                                (self.static_index(*left, actual), self.static_index(*right, required))
                            {
                                if left == right {
                                    return true;
                                }
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
                            // Only runtime Value binders have descriptor fields.
                            // Const bindings still use the checked scalar relation
                            // above, but never acquire descriptor authority.
                            if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                                return false;
                            }
                            let closed_requirement = self.static_index(*right, required).is_some()
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
                                    call_target_parameters(self.program, target)
                                else {
                                    return false;
                                };
                                return captured_call_argument_index_matches(
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
                        } else if matches!(
                            parameter.kind,
                            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. }
                        ) {
                            let identity = |reference| {
                                let operand = self.program.static_operand_from_type_reference(reference).ok()?;
                                self.program.static_operand_identity(
                                    StaticOperandIdentityRequest {
                                        operand: typed_trees::type_identity::IndexIdentityArgument::StaticArgument(&operand),
                                        lexical_owner: self.machine.symbol,
                                        substitutions: &[],
                                    },
                                )
                            };
                            let left = identity(*left);
                            let right = identity(*right);
                            left.is_some() && left == right
                        } else if !self.program.type_application_value_indices(*left).is_empty() || !self.program.type_application_value_indices(*right).is_empty() {
                            recurse(*left, *right)
                        } else {
                            exact(*left, *right)
                        }
                    })
            }
            _ => exact(actual_reference, required_reference),
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
        let static_equal = self
            .static_index(argument, application)
            .is_some_and(|required| {
                applied_index_identity(
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
                let Some(reference) = declaration_type_reference(self.program, path.head_symbol)
                else {
                    return None;
                };
                let formation = self
                    .program
                    .state_local_declaration(self.state, path.head_symbol)
                    .map(|(position, _)| position)
                    .unwrap_or(0);
                let mut receiver = Application::bare(reference, formation);
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
                    if data.is_some_and(|data| {
                        self.program.data_members(data).iter().any(|member| {
                        matches!(member, DataMember::Variant(case) if case.symbol == *symbol)
                    })
                    }) {
                        continue;
                    }
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
        expected_proof_identity: Option<NormalizedTypeIdentity>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let formal_evidence = erased
            && is_source_evidence_carrier(self.program, required.reference)
            && expected_proof_identity.is_some_and(|expected| {
                self.erased_evidence_reference(expression, 0)
                    .is_some_and(|actual| {
                        let payload_owner =
                            match self.program.expression_table.expression(expression) {
                                ExpressionNode::Member(_) => declared_case_projection_field(
                                    self.program,
                                    self.machine,
                                    Some(self.state),
                                    expression,
                                )
                                .map(|field| self.program.symbols.get(field.symbol).parent)
                                .unwrap_or_default(),
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
                                applied_type_identity(
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
                let reference = expression_result_type_reference(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                )?;
                if !is_source_evidence_carrier(self.program, reference) {
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
                let field = declared_case_projection_field(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                )?;
                if !field.relevance.is_erased() {
                    return None;
                }
                if is_source_evidence_carrier(self.program, field.type_reference) {
                    return Some(field.type_reference);
                }
                // A prepared Type application substitutes its payload types.
                // Rejoin this exact selection to the independently formed
                // declaration carrier, not an unregistered applied reference.
                // actual_erased_identity subsequently applies the receiver's
                // complete Type/value tuple and exact payload projections.
                let retained = member.member_symbol;
                if !retained.is_valid()
                    || self.program.symbols.get(retained).kind != SymbolKind::Field
                {
                    return None;
                }
                let case_symbol = self.program.symbols.get(retained).parent;
                if !case_symbol.is_valid()
                    || self.program.symbols.get(case_symbol).kind != SymbolKind::Variant
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
                    && is_source_evidence_carrier(self.program, declared.type_reference))
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
            && self.program.symbols.get(symbol).kind == SymbolKind::Field
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
                                self.program
                                    .type_application_value_indices(*argument)
                                    .is_empty()
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
                    && declared_case_projection_field(
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

    fn stable_static_proof_argument(&self, argument: &StaticMachineArgument) -> bool {
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
        static_const_argument_spelling(self.program.typed_program(), argument).is_some_and(
            |spelling| {
                spelling.parse::<i128>().is_ok()
                    || CanonicalConstValue::from_atom(&spelling).is_some()
            },
        )
    }

    fn actual_erased_identity(
        &self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<NormalizedTypeIdentity> {
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
        let ExpressionNode::Member(member) = self.program.expression_table.expression(expression)
        else {
            return applied_type_identity(self.program, reference, self.machine.symbol, &[]);
        };
        let field = declared_case_projection_field(
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
        applied_type_identity_in_scope(
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
        site: &CallSite<'_>,
        parameters: &[StateParameter],
        arguments: &[ExpressionHandle],
        position: usize,
    ) -> Option<NormalizedTypeIdentity> {
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
            CallSite::Statement(call) => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            CallSite::Expression { call, .. } => (
                call.authored_machine_application,
                call.machine_arguments.as_ref(),
            ),
            CallSite::TransitionNamed { .. } => (Default::default(), &[][..]),
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
        if !is_source_evidence_carrier(self.program, reference) {
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
        applied_type_identity_in_scope(
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
        if self
            .program
            .type_application_value_indices(required.reference)
            .is_empty()
        {
            return;
        }
        if !expression.is_valid() {
            if self
                .program
                .type_application_value_indices(required.reference)
                .has_runtime_values()
            {
                diagnostics.push(Diagnostic::error(
                    "runtime indexed default requires an explicit captured construction",
                ));
            }
            return;
        }
        // Contextual formation checks the same receiving obligation at every
        // produced leaf. A branch, aggregate, or cast cannot hide an old
        // indexed value behind its destination's annotation.
        if self
            .program
            .expression_const_declaration(expression)
            .is_some()
        {
            self.check_completed_value(expression, &required, diagnostics);
            return;
        }
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
            ExpressionNode::Cast(cast)
                if cast.form.is_recast()
                    && !self
                        .program
                        .type_application_value_indices(cast.target_type)
                        .has_runtime_values() =>
            {
                self.check_completed_value(expression, &required, diagnostics);
                return;
            }
            ExpressionNode::Cast(cast) => {
                self.check(
                    cast.value,
                    Application::bare(cast.target_type, self.statement),
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
                        if self
                            .program
                            .type_application_value_indices(declared.type_reference)
                            .has_runtime_values()
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
        self.check_completed_value(expression, &required, diagnostics);
    }

    fn check_completed_value(
        &self,
        expression: ExpressionHandle,
        required: &Application<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // An exact empty static world has no runtime argument arrival. Source
        // formation remains checked; a dead type-dispatch edge cannot require
        // equality between two distinct applied data carriers.
        if self.program.static_type_context_is_unreachable(expression) {
            return;
        }
        let Some(actual) = self.actual(expression) else {
            diagnostics.push(
                Diagnostic::error("runtime indexed value has no retained subject provenance")
                    .with_source_span(self.program.expression_table.source_span(expression)),
            );
            return;
        };
        if !self.compare(&actual, required, expression, 0) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` cannot prove runtime index subject equality at statement {}; an indexed value retains its captured index",
                self.machine.name, self.state.name, self.statement,
            )).with_source_span(self.program.expression_table.source_span(expression)));
        }
    }
}

pub(super) fn check(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    frames: Option<&CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (has_const_indices, has_runtime_indices) = program.data_type_parameters.iter().fold(
        (false, false),
        |(has_const, has_runtime), (_, parameter)| match parameter.kind {
            TypeParameterKind::Const { .. } => (true, has_runtime),
            TypeParameterKind::Value { .. } => (has_const, true),
            _ => (has_const, has_runtime),
        },
    );
    if !has_const_indices && !has_runtime_indices {
        return;
    }
    for (_, flow) in facts.flow.control.states.iter() {
        let Some(machine) = program.machine_by_symbol(flow.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(flow.state_symbol) else {
            continue;
        };
        for (statement, node) in crate::flow::state_statements(program, state.statement_nodes)
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
            if has_runtime_indices {
                let mut roots = Vec::new();
                collect_statement_expression_trees(program, node, &mut roots);
                let mut visited = Vec::new();
                for root in roots {
                    visit_post_order(program, root, &mut visited, |expression, node| {
                        if let ExpressionNode::Borrow(borrow) = node {
                            if borrow.access != language_core::ReferenceAccess::Shared
                                && scope.descriptor_target(borrow.target).is_some()
                            {
                                diagnostics.push(Diagnostic::error("mutable scalar loan cannot preserve a runtime index descriptor")
                                .with_source_span(program.expression_table.source_span(expression)));
                            }
                        }
                    });
                }
            }
            match node {
                StatementNode::LocalData(local) => scope.check_binding(
                    local.initial_value,
                    Application::bare(local.type_reference, statement),
                    local.relevance.is_erased(),
                    applied_type_identity(program, local.type_reference, machine.symbol, &[]),
                    diagnostics,
                ),
                StatementNode::Expression(expression) if state.return_type.is_valid() => scope
                    .check(
                        *expression,
                        Application::bare(state.return_type, 0),
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
                                Application::bare(state.return_type, 0),
                                diagnostics,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        for call in facts.flow.control.calls.span_or_empty(flow.calls) {
            let Some(parameters) = call_target_parameters(program, call.target_symbol) else {
                continue;
            };
            let Some(site) = find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let arguments = call_site_argument_expressions(program, &site);
            let scope = Scope {
                program,
                machine,
                state,
                statement: call.statement_index,
                target: TransitionTargetHandle::invalid(),
                frames,
            };
            let (substitutions, captured_arguments) =
                scope.call_substitutions(site, parameters, &arguments);
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
                        && is_source_evidence_carrier(program, parameter.type_reference))
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

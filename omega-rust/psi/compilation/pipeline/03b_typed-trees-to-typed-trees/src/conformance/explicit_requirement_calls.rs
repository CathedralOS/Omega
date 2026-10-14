//! A requirement call that names its implementation,
//! `Ranked::compare<Card, PowerOrder>(left, right)`, selects that
//! implementation's row for the requirement. The arguments before the
//! implementation are the trait's application and must be the implementation's
//! own. A concrete implementation's call becomes a direct static call to the
//! row's realization carrying the closed application as its dispatch, the same
//! rewrite specialization applies to a call through an evidence binder once
//! the binder is bound. A call naming an evidence binder,
//! `Ranked::compare<T, Order>(left, right)` inside a generic machine, becomes
//! that binder's own requirement call, `Order::compare(left, right)`, which
//! specialization binds.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::typed_trees::StaticRequirementDispatch;
use typed_trees::types::TypeReferenceHandle;
use validation::conformance_applications::close_conformance_application;

mod application_arguments;
use application_arguments::{ApplicationArguments, ApplicationScope};

struct ExplicitRequirementRewrite {
    call: ExpressionHandle,
    target: SymbolHandle,
    name: typed_trees::name::Identifier,
    application_arguments: Box<[StaticMachineArgument]>,
    /// Present for a concrete implementation; a binder's requirement call is
    /// dispatched by specialization.
    dispatch: Option<StaticRequirementDispatch>,
    /// A binder's requirement call keeps the binder as its receiver, the
    /// spelling `Order::compare(left, right)` checks and specializes.
    binder_receiver: Option<(typed_trees::name::Identifier, SymbolHandle)>,
}

/// Retarget every requirement call whose static arguments name one
/// implementation, concrete or an evidence binder.
pub(crate) fn route_explicit_requirement_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let requirements = program
        .traits()
        .iter()
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .map(move |signature| (signature.symbol, definition.symbol))
        })
        .collect::<Vec<_>>();
    if requirements.is_empty() {
        return Ok(());
    }
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            let (requirement, declaring_trait) = requirements
                .iter()
                .find(|(requirement, _)| *requirement == call.target_symbol)?;
            call.machine_arguments
                .iter()
                .any(|argument| names_implementation(program, argument))
                .then(|| {
                    (
                        handle,
                        *requirement,
                        *declaring_trait,
                        call.machine_arguments.clone(),
                    )
                })
        })
        .collect::<Vec<_>>();
    for (handle, requirement, declaring_trait, arguments) in calls {
        match explicit_requirement_rewrite(
            program,
            handle,
            requirement,
            declaring_trait,
            &arguments,
        ) {
            Ok(rewrite) => rewrites.push(rewrite),
            Err(message) => diagnostics.push(
                Diagnostic::error(message)
                    .with_source_span(program.expression_table.source_span(handle)),
            ),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for rewrite in rewrites {
        let receiver =
            rewrite
                .binder_receiver
                .map_or_else(ExpressionHandle::invalid, |(name, binder)| {
                    crate::monomorphization::insert_subject_name(
                        &mut program.expression_table,
                        name,
                        binder,
                    )
                });
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(rewrite.call)
        else {
            continue;
        };
        call.target_symbol = rewrite.target;
        call.target = rewrite.name;
        call.receiver = receiver;
        call.machine_arguments = rewrite.application_arguments;
        call.static_requirement_dispatch = rewrite.dispatch;
    }
    Ok(())
}

fn names_implementation(
    program: &dyn ::validation::CheckingInput,
    argument: &StaticMachineArgument,
) -> bool {
    matches!(
        program.symbols.get(argument.symbol).kind,
        SymbolKind::Conformance | SymbolKind::ConformanceParameter
    )
}

/// The implementation is the last static argument; every argument before it
/// is the trait's application, in the trait's declaration order.
fn explicit_requirement_rewrite(
    program: &mut TypedTrees,
    call: ExpressionHandle,
    requirement: SymbolHandle,
    declaring_trait: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Result<ExplicitRequirementRewrite, String> {
    let requirement_name = program.symbols.display_path(requirement, "::");
    let Some((implementation, trait_arguments)) = arguments.split_last() else {
        return Err(format!("`{requirement_name}` names no implementation"));
    };
    let definition = program
        .trait_definition_by_symbol(declaring_trait)
        .ok_or_else(|| format!("`{requirement_name}` lost its declaring trait"))?;
    let parameters = program.trait_type_parameters(definition);
    if !names_implementation(program, implementation)
        || trait_arguments.len() != parameters.len()
        || parameters
            .iter()
            .zip(trait_arguments)
            .any(|(parameter, argument)| {
                (parameter.kind == typed_trees::data::TypeParameterKind::Implementation)
                    != names_implementation(program, argument)
            })
    {
        return Err(format!(
            "`{requirement_name}` names its implementation as its one last static argument, after the trait's arguments"
        ));
    }
    let trait_references = trait_arguments
        .iter()
        .map(|argument| {
            program
                .static_operand_type_reference(argument)
                .map_err(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let implementation_references = implementation
        .application
        .as_ref()
        .map_or(&[][..], |application| &application.arguments)
        .iter()
        .map(|argument| {
            program
                .static_operand_type_reference(argument)
                .map_err(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lexical_owner = program
        .expression_table
        .call_node(call)
        .filter(|call| call.authored_machine_application.is_valid())
        .map(|call| {
            program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .lexical_owner
        })
        .unwrap_or_default();
    let mut request = ExplicitApplication {
        program,
        call,
        requirement,
        requirement_name,
        declaring_trait,
        trait_arguments,
        implementation,
        trait_references,
        implementation_references,
        lexical_owner,
    };
    if request.program.symbols.get(implementation.symbol).kind == SymbolKind::ConformanceParameter {
        request.through_binder()
    } else {
        request.through_implementation()
    }
}

struct ExplicitApplication<'a> {
    program: &'a mut TypedTrees,
    call: ExpressionHandle,
    requirement: SymbolHandle,
    requirement_name: String,
    declaring_trait: SymbolHandle,
    trait_arguments: &'a [StaticMachineArgument],
    implementation: &'a StaticMachineArgument,
    trait_references: Vec<TypeReferenceHandle>,
    implementation_references: Vec<TypeReferenceHandle>,
    lexical_owner: SymbolHandle,
}

impl ExplicitApplication<'_> {
    fn through_implementation(&mut self) -> Result<ExplicitRequirementRewrite, String> {
        let program = &*self.program;
        let application = close_conformance_application(program, self.implementation)
            .map_err(|diagnostic| diagnostic.message)?;
        let Some(conformance) = program
            .conformance_by_symbol(application.declaration)
            .cloned()
        else {
            return Err(format!(
                "`{}` does not name an implementation",
                self.implementation.display_name()
            ));
        };
        let implemented = self.application_display(
            conformance.trait_name.as_str(),
            program
                .type_reference_table
                .type_reference_handles(conformance.arguments),
        );
        let scope = ApplicationScope::new(
            conformance.symbol,
            program.conformance_type_parameters(&conformance),
            &self.implementation_references,
            conformance
                .lifetime_parameters
                .iter()
                .map(|parameter| parameter.as_str().to_owned())
                .zip(application.lifetime_arguments.iter().cloned())
                .collect(),
        )
        .ok_or_else(|| self.wrong_trait(&implemented))?;
        let lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                conformance
                    .lifetime_parameters
                    .get(*ordinal as usize)
                    .map(|name| name.as_str().to_owned())
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| self.wrong_trait(&implemented))?;
        let expected = ApplicationArguments {
            declaration: application.trait_definition,
            arguments: program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .to_vec(),
            lifetimes,
            scopes: vec![scope],
        };
        self.check_trait_arguments(&implemented, expected)?;
        let program = &mut *self.program;
        let Some(row) = conformance
            .rows
            .iter()
            .find(|row| row.requirement == self.requirement)
        else {
            return Err(format!(
                "implementation `{}` has no row for `{}`",
                self.implementation.display_name(),
                self.requirement_name
            ));
        };
        let source_application = program
            .expression_table
            .call_node(self.call)
            .map(|call| call.authored_machine_application)
            .filter(|handle| handle.is_valid())
            .ok_or_else(|| {
                "direct conformance selection lost its authored application".to_owned()
            })?;
        let ordinal = program
            .expression_table
            .static_machine_application_arguments(source_application)
            .len()
            .checked_sub(1)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "direct conformance selection lost its explicit operand".to_owned())?;
        let target = row.realization_state;
        let row = row.clone();
        program
            .expression_table
            .static_machine_application_mut(source_application)
            .conformance_selection = Some(Box::new(
            typed_trees::static_applications::StaticConformanceSelection {
                operand_ordinal: ordinal,
                application: Some(Box::new(application.clone())),
            },
        ));
        Ok(ExplicitRequirementRewrite {
            call: self.call,
            target,
            name: typed_trees::name::Identifier::generated(
                row.realization_name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or(row.realization_name.as_str()),
            ),
            application_arguments: Box::new([]),
            binder_receiver: None,
            dispatch: Some(StaticRequirementDispatch {
                selection_application: source_application,
                application_report_fingerprint: application.report_fingerprint,
                application_commitment: application.commitment,
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                contract_signature: SymbolHandle::invalid(),
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
            }),
        })
    }

    /// A binder's placeholder children are its trait's requirement closure in
    /// order, the same order specialization rewrites when it binds them.
    fn through_binder(&mut self) -> Result<ExplicitRequirementRewrite, String> {
        if self.implementation.application.is_some() {
            return Err("an implementation evidence binder is already selected and takes no application arguments".to_owned());
        }
        let program = &*self.program;
        let binder = self.implementation.symbol;
        let Some(bound) = program
            .machines()
            .iter()
            .flat_map(|machine| machine.conformance_bounds.iter())
            .find(|bound| bound.binder == binder)
            .cloned()
        else {
            return Err(format!(
                "evidence binder `{}` has no declared trait",
                self.implementation.display_name()
            ));
        };
        let bounded = self.application_display(bound.trait_name.as_str(), &bound.arguments);
        let Some(trait_definition) = program.trait_definition_by_symbol(bound.trait_symbol) else {
            return Err(format!("evidence binder bound `{bounded}` names no trait"));
        };
        let mut closure = Vec::new();
        crate::monomorphization::collect_evidence_requirement_closure(
            program,
            trait_definition,
            &mut Vec::new(),
            &mut closure,
        );
        let Some(position) = closure
            .iter()
            .position(|signature| signature.symbol == self.requirement)
        else {
            return Err(self.wrong_trait(&bounded));
        };
        let Some(placeholder) = program
            .symbols
            .child_handles(binder)
            .into_iter()
            .flatten()
            .nth(position)
        else {
            return Err(format!(
                "evidence binder `{}` has no requirement call for `{}`",
                self.implementation.display_name(),
                self.requirement_name
            ));
        };
        let target_name = typed_trees::name::Identifier::generated(closure[position].name.as_str());
        self.check_trait_arguments(
            &bounded,
            ApplicationArguments {
                declaration: bound.trait_symbol,
                arguments: bound.arguments.clone(),
                lifetimes: Vec::new(),
                scopes: Vec::new(),
            },
        )?;
        Ok(ExplicitRequirementRewrite {
            call: self.call,
            target: placeholder,
            name: target_name,
            application_arguments: Box::default(),
            dispatch: None,
            binder_receiver: Some((bound.binder_name.clone(), binder)),
        })
    }

    fn application_display(
        &self,
        trait_name: &str,
        arguments: &[typed_trees::types::TypeReferenceHandle],
    ) -> String {
        format!(
            "{trait_name}<{}>",
            arguments
                .iter()
                .map(|argument| self.program.display_type_reference(*argument))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// How the named implementation relates to its trait application: a
    /// concrete implementation implements it, a binder is bound as it.
    fn relation(&self) -> &'static str {
        if self.program.symbols.get(self.implementation.symbol).kind
            == SymbolKind::ConformanceParameter
        {
            "is bound as"
        } else {
            "implements"
        }
    }

    fn wrong_trait(&self, implemented: &str) -> String {
        format!(
            "`{}` {} `{implemented}`, which neither is nor inherits `{}`, the trait that declares `{}`",
            self.implementation.display_name(),
            self.relation(),
            self.program
                .symbols
                .display_path(self.declaring_trait, "::"),
            self.requirement_name,
        )
    }

    fn check_trait_arguments(
        &mut self,
        implemented: &str,
        expected: ApplicationArguments,
    ) -> Result<(), String> {
        let matched = application_arguments::matches_declaring_application(
            self.program,
            self.lexical_owner,
            self.declaring_trait,
            &self.trait_references,
            expected,
        );
        match matched {
            Some(true) => return Ok(()),
            None => return Err(self.wrong_trait(implemented)),
            Some(false) => {}
        }
        Err(format!(
            "`{}` {} `{implemented}`, not the `{}<{}>` this call of `{}` names",
            self.implementation.display_name(),
            self.relation(),
            self.program
                .symbols
                .display_path(self.declaring_trait, "::"),
            self.trait_arguments
                .iter()
                .map(StaticMachineArgument::display_name)
                .collect::<Vec<_>>()
                .join(", "),
            self.requirement_name,
        ))
    }
}

/// Rebuild copied occurrence selections from their rewritten explicit operand.
/// Cloning transports source operands, never old derived application authority.
pub(crate) fn refresh_occurrence_selections(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let live = program.live_conformance_occurrence_applications();
    let pending = live
        .iter()
        .copied()
        .filter_map(|handle| {
            let source = program.expression_table.static_machine_application(handle);
            source.conformance_selection.as_ref().map(|selection| {
                (
                    handle,
                    selection.operand_ordinal,
                    selection.application.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    let mut changed = Vec::new();
    for (handle, ordinal, previous) in pending {
        let operand = program
            .expression_table
            .static_machine_application_arguments(handle)
            .get(ordinal as usize)
            .cloned()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "direct conformance occurrence lost its selected operand",
                )]
            })?;
        let mut application =
            close_conformance_application(program, &operand).map_err(|d| vec![d])?;
        let mut old = previous.as_deref().cloned();
        if let Some(old) = &mut old {
            old.callable_application = Default::default();
            old.trait_scope = Default::default();
        }
        if old.as_ref() == Some(&application) {
            let previous = previous.as_ref().unwrap();
            let source_owner = program
                .expression_table
                .static_machine_application(handle)
                .lexical_owner;
            if previous.trait_scope.scope(program).is_some()
                && previous.trait_scope.supplying_owner() != source_owner
            {
                return Err(vec![Diagnostic::error(
                    "refreshed conformance scope has another supplying owner",
                )]);
            }
            if previous.callable_application.is_valid()
                && program
                    .expression_table
                    .static_machine_application(previous.callable_application)
                    .lexical_owner
                    != source_owner
            {
                return Err(vec![Diagnostic::error(
                    "refreshed capture view has another supplying owner",
                )]);
            }
            application.callable_application = previous.callable_application;
            application.trait_scope = previous.trait_scope;
        } else {
            changed.push(handle);
        }
        program
            .expression_table
            .static_machine_application_mut(handle)
            .conformance_selection
            .as_mut()
            .unwrap()
            .application = Some(Box::new(application));
    }
    let mut expressions = Vec::new();
    let mut statements = Vec::new();
    for (handle, node) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = node
            && live.contains(&call.authored_machine_application)
            && call
                .static_requirement_dispatch
                .as_ref()
                .is_some_and(|d| d.selection_application.is_valid())
        {
            expressions.push((handle, call.authored_machine_application));
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
                if let Some(call) = program.statement_table.statement(handle).as_call()
                    && live.contains(&call.authored_machine_application)
                    && call
                        .static_requirement_dispatch
                        .as_ref()
                        .is_some_and(|d| d.selection_application.is_valid())
                {
                    statements.push((handle, call.authored_machine_application));
                }
            }
        }
    }
    let update = |program: &TypedTrees,
                  handle|
     -> Result<
        (
            u64,
            typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
        ),
        Vec<Diagnostic>,
    > {
        let app = program
            .expression_table
            .static_machine_application(handle)
            .conformance_selection
            .as_ref()
            .and_then(|s| s.application.as_deref())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "direct dispatch lost its occurrence-owned application",
                )]
            })?;
        Ok((app.report_fingerprint, app.commitment))
    };
    for (expression, handle) in expressions {
        let (fingerprint, commitment) = update(program, handle)?;
        let call = match program.expression_table.expression_mut(expression) {
            ExpressionNode::Call(call) => call,
            _ => unreachable!(),
        };
        let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
        if changed.contains(&handle) || dispatch.selection_application != handle {
            dispatch.contract_signature = SymbolHandle::invalid();
        }
        dispatch.selection_application = handle;
        dispatch.application_report_fingerprint = fingerprint;
        dispatch.application_commitment = commitment;
    }
    for (statement, handle) in statements {
        let (fingerprint, commitment) = update(program, handle)?;
        let call = match program.statement_table.statement_mut(statement) {
            typed_trees::statement::StatementNode::Call(call) => call,
            _ => unreachable!(),
        };
        let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
        if changed.contains(&handle) || dispatch.selection_application != handle {
            dispatch.contract_signature = SymbolHandle::invalid();
        }
        dispatch.selection_application = handle;
        dispatch.application_report_fingerprint = fingerprint;
        dispatch.application_commitment = commitment;
    }
    Ok(())
}

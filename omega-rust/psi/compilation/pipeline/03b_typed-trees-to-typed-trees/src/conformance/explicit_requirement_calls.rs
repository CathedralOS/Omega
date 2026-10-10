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
use validation::conformance_applications::{
    close_conformance_application, substituted_type_identity, type_argument_identity,
};

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
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some((requirement, declaring_trait)) = requirements
            .iter()
            .find(|(requirement, _)| *requirement == call.target_symbol)
        else {
            continue;
        };
        if !call
            .machine_arguments
            .iter()
            .any(|argument| names_implementation(program, argument))
        {
            continue;
        }
        match explicit_requirement_rewrite(
            program,
            handle,
            *requirement,
            *declaring_trait,
            &call.machine_arguments,
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
    program: &dyn ::validation::CheckingInput,
    call: ExpressionHandle,
    requirement: SymbolHandle,
    declaring_trait: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Result<ExplicitRequirementRewrite, String> {
    let requirement_name = program.symbols.display_path(requirement, "::");
    let Some((implementation, trait_arguments)) = arguments.split_last() else {
        return Err(format!("`{requirement_name}` names no implementation"));
    };
    if !names_implementation(program, implementation)
        || trait_arguments
            .iter()
            .any(|argument| names_implementation(program, argument))
    {
        return Err(format!(
            "`{requirement_name}` names its implementation as its one last static argument, after the trait's arguments"
        ));
    }
    let request = ExplicitApplication {
        program,
        call,
        requirement,
        requirement_name,
        declaring_trait,
        trait_arguments,
        implementation,
    };
    if program.symbols.get(implementation.symbol).kind == SymbolKind::ConformanceParameter {
        request.through_binder()
    } else {
        request.through_implementation()
    }
}

struct ExplicitApplication<'a> {
    program: &'a dyn ::validation::CheckingInput,
    call: ExpressionHandle,
    requirement: SymbolHandle,
    requirement_name: String,
    declaring_trait: SymbolHandle,
    trait_arguments: &'a [StaticMachineArgument],
    implementation: &'a StaticMachineArgument,
}

impl ExplicitApplication<'_> {
    fn through_implementation(&self) -> Result<ExplicitRequirementRewrite, String> {
        let program = self.program;
        let application = close_conformance_application(program, self.implementation)
            .map_err(|diagnostic| diagnostic.message)?;
        let Some(conformance) = program
            .conformance_by_symbol(application.declaration)
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
        let Some(expected) = self.declaring_application(
            application.trait_definition,
            application.trait_arguments.clone(),
        ) else {
            return Err(self.wrong_trait(&implemented));
        };
        self.check_trait_arguments(&implemented, &expected)?;
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
        Ok(ExplicitRequirementRewrite {
            call: self.call,
            target: row.realization_state,
            name: typed_trees::name::Identifier::generated(
                row.realization_name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or(row.realization_name.as_str()),
            ),
            application_arguments: application.arguments.clone(),
            binder_receiver: None,
            dispatch: Some(StaticRequirementDispatch {
                application_report_fingerprint: application.report_fingerprint,
                application_commitment: application.commitment,
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
            }),
        })
    }

    /// A binder's placeholder children are its trait's requirement closure in
    /// order, the same order specialization rewrites when it binds them.
    fn through_binder(&self) -> Result<ExplicitRequirementRewrite, String> {
        let program = self.program;
        let binder = self.implementation.symbol;
        let Some(bound) = program
            .machines()
            .iter()
            .flat_map(|machine| machine.conformance_bounds.iter())
            .find(|bound| bound.binder == binder)
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
        let declared = bound
            .arguments
            .iter()
            .map(|argument| program.normalized_type_identity(*argument).into_string())
            .collect::<Vec<_>>();
        let Some(expected) = self.declaring_application(bound.trait_symbol, declared) else {
            return Err(self.wrong_trait(&bounded));
        };
        self.check_trait_arguments(&bounded, &expected)?;
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
        Ok(ExplicitRequirementRewrite {
            call: self.call,
            target: placeholder,
            name: typed_trees::name::Identifier::generated(closure[position].name.as_str()),
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

    /// The application of the requirement's declaring trait that an
    /// implementation of `implemented` with `arguments` (type identities) also
    /// supplies: the trait itself, or a parent reached through its inheritance
    /// edges with each child's parameters substituted into the parent's
    /// arguments. `None` when the declaring trait is not among them.
    fn declaring_application(
        &self,
        implemented: SymbolHandle,
        arguments: Vec<String>,
    ) -> Option<Vec<String>> {
        let program = self.program;
        let mut pending = vec![(implemented, arguments)];
        let mut visited = Vec::new();
        while let Some((trait_symbol, arguments)) = pending.pop() {
            if trait_symbol == self.declaring_trait {
                return Some(arguments);
            }
            if visited.contains(&trait_symbol) {
                continue;
            }
            visited.push(trait_symbol);
            let definition = program.trait_definition_by_symbol(trait_symbol)?;
            let substitutions = program
                .trait_type_parameters(definition)
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(arguments)
                .collect::<Vec<_>>();
            for parent in program.trait_requirements(definition) {
                let parent_arguments = program
                    .type_reference_table
                    .type_reference_handles(parent.arguments)
                    .iter()
                    .map(|argument| {
                        substituted_type_identity(
                            program,
                            *argument,
                            &substitutions,
                            &[],
                        )
                    })
                    .collect();
                pending.push((parent.symbol, parent_arguments));
            }
        }
        None
    }

    fn check_trait_arguments(&self, implemented: &str, expected: &[String]) -> Result<(), String> {
        let supplied = self
            .trait_arguments
            .iter()
            .map(|argument| type_argument_identity(self.program, argument))
            .collect::<Vec<_>>();
        if supplied == expected {
            return Ok(());
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

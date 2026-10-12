//! Source choices for pending constant computation, independent of realization.
//!
//! An executed Build choice retains its original Product operands. Those inputs
//! describe a source application; they do not supply a ProviderPlan, completion
//! role, native body or execution permission. The ordinary constant evaluator
//! checks the selected source body and its closure before publishing a value.
//! A choice whose application has no closed source-body correspondence remains
//! a recipe rather than borrowing a target's realized adapter.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::typed_trees::product_applications::{ProductDeclarationKind, ProductDeclarationOperand};

/// The original operands of one executed source selection. Default realization
/// rows have no such input and cannot nominate a constant's source meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProviderSelection {
    pub operands: [ProductDeclarationOperand; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceProviderBody {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
}

/// Issued once by original source choice elaboration. Consumers retain this
/// relation; they do not re-run map selection after a private call rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProviderBinding {
    selection: SourceProviderSelection,
    owner: std::sync::Arc<typed_trees::FrozenTypedTrees>,
    body: SourceProviderBody,
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
    requirement: SymbolHandle,
}

impl SourceProviderBinding {
    pub fn original_program(&self) -> &typed_trees::FrozenTypedTrees {
        &self.owner
    }

    pub(crate) fn selection(&self) -> &SourceProviderSelection {
        &self.selection
    }

    pub(crate) fn belongs_to(
        &self,
        program: &TypedTrees,
        row: &crate::SelectedBuildTimeProviderBody,
    ) -> bool {
        self.body.machine == row.provider_machine
            && self.body.state == row.provider_state
            && self.expression == row.expression
            && self.origin == row.origin
            && self.requirement == row.requirement
            && program.identity.get() == self.owner.identity.get()
            && [row.provider_machine, row.provider_state].into_iter().all(|symbol| {
                program.retains_source_declaration_symbol(&self.owner, symbol)
            })
    }
}

pub(crate) enum SourceProviderResolution {
    Unselected,
    Body(SourceProviderBody),
    Retained(&'static str),
}

impl SourceProviderSelection {
    /// Declaration correspondence is checked in each issuing namespace. A
    /// shared source symbol alone never relocates either operand's type rows.
    fn belongs_to(&self, program: &TypedTrees) -> bool {
        let [subject, provider] = &self.operands;
        subject.source_application.is_valid()
            && subject.source_application == provider.source_application
            && subject.argument_index == 0
            && provider.argument_index == 1
            && matches!(
                subject.kind,
                ProductDeclarationKind::ProviderSubject | ProductDeclarationKind::BoundaryRequirement
            )
            && provider.kind == ProductDeclarationKind::ProviderType
            && self.operands.iter().all(|operand| {
                program.identity.get() == operand.owner.identity.get()
                    && program.retains_source_declaration_symbol(&operand.owner, operand.declaration)
            })
    }

    /// Rejoin the original selected declaration before inspecting its body.
    /// This is a supplying application view, not conformance admission. The
    /// private ordinary checking operation still judges the complete map and
    /// executable closure under the invocation's declaration authority.
    pub(crate) fn resolve(
        &self,
        program: &TypedTrees,
        requirement: SymbolHandle,
    ) -> Result<SourceProviderResolution, String> {
        if !self.belongs_to(program) {
            return Err("source constant selection lost its original Product operand owners".into());
        }
        let boundary = program.machine(requirement).is_some_and(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        }) || program.public_trait_requirement_signature(requirement).is_some_and(|member| member.owner.is_boundary)
            || typed_trees::operator::declaration_by_symbol(program, requirement).is_some_and(|operator| operator.is_boundary);
        if !boundary {
            return Ok(SourceProviderResolution::Unselected);
        }
        let original = typed_trees::product_requirements::original_prepared_requirement_symbol(
            program,
            requirement,
        )
        .map_err(|_| "source constant use lost its original requirement application".to_owned())?;
        let [subject, provider] = &self.operands;
        let matches = if subject.declaration == original {
            true
        } else if let Some((declaring, _)) = program.trait_machine_signature(original) {
            typed_trees::static_applications::boundary_interface_members(
                &subject.owner,
                subject.declaration,
            )
            .is_some_and(|members| members.contains(&(declaring.symbol, original)))
        } else {
            false
        };
        if !matches {
            return Ok(SourceProviderResolution::Unselected);
        }
        if subject.application.is_valid() {
            let application = subject
                .owner
                .expression_table
                .static_machine_application(subject.application);
            let Some(owner) = application.type_identity_owner(&subject.owner) else {
                return Ok(SourceProviderResolution::Retained("source constant subject lost its original lexical application"));
            };
            let actual = typed_trees::static_applications::trait_type_application_identity(
                &subject.owner,
                subject.receiver_type,
                owner,
            );
            let expected = typed_trees::static_applications::requirement_provider_application_identity(
                program,
                requirement,
                subject.declaration,
            );
            let same = actual.ok()
                .zip(expected.ok())
                .is_some_and(|(actual, expected)| actual == expected);
            if !same {
                return Ok(SourceProviderResolution::Unselected);
            }
        }
        // The selected map's closed source instance must be retained by its
        // producer. This operation never reconstructs actuals from expected
        // trait arguments or from a native adapter's runtime signature.
        if provider.application.is_valid() {
            return Ok(SourceProviderResolution::Retained("source constant provider application needs its original closed map instance"));
        }
        let mut bodies = Vec::new();
        if let Some(conformance) = program.conformance_by_symbol(provider.declaration) {
            if !program.conformance_type_parameters(conformance).is_empty()
                || !conformance.lifetime_parameters.is_empty()
            {
                return Ok(SourceProviderResolution::Retained("source constant map has an unclosed original telescope"));
            }
            let scope = typed_trees::trait_definition::AppliedTraitScope::from_conformance(
                program,
                conformance,
            );
            let complete = program.trait_machine_signature(original).is_some_and(|(owner, _)| {
                typed_trees::static_applications::original_requirement_is_complete(
                    program,
                    conformance.trait_symbol,
                    original,
                ) && scope.as_ref().is_some_and(|scope| scope.declaring_scope(program, owner.symbol).is_some())
            });
            let matches = scope.as_ref().is_some_and(|scope| {
                program.requirement_application_matches_scope(requirement, scope)
            });
            if !matches && !complete {
                return Ok(SourceProviderResolution::Retained("source constant map lost its complete public application correspondence"));
            }
            for row in &conformance.rows {
                if row.requirement != original {
                    continue;
                }
                if requirement == original {
                    bodies.push(SourceProviderBody { machine: row.realization_machine, state: row.realization_state });
                } else {
                    bodies.extend(program.instantiated_requirement_realizations.iter().filter_map(|(_, instance)| {
                        (instance.named_conformance.is_valid()
                            && program.conformances.get(instance.named_conformance).symbol == conformance.symbol
                            && instance.requirement_template == original
                            && instance.requirement_instance == requirement
                            && instance.provider_template == row.realization_machine)
                            .then_some(SourceProviderBody { machine: instance.provider_instance, state: instance.realization_state })
                    }));
                }
            }
        } else if program.data_definition_by_symbol(provider.declaration).is_some() {
            for machine in program.machines().iter().filter(|machine| machine.attached_data_symbol == provider.declaration) {
                for edge in program.machine_trait_conformances(machine) {
                    let Some(selected) = typed_trees::machine::resolve_satisfied_declaration(program, machine, edge) else { continue; };
                    if selected.symbol() != original || edge.external_binding.is_some() {
                        continue;
                    }
                    if let typed_trees::machine::SatisfiedDeclaration::Trait { .. } = selected {
                        let Some(scope) = typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(program, machine, edge) else {
                            return Ok(SourceProviderResolution::Retained("source constant provider lost its original trait scope"));
                        };
                        let complete = typed_trees::static_applications::original_requirement_is_complete(program, scope.declaration.symbol, original);
                        if !complete && !program.requirement_application_matches_scope(requirement, &scope) {
                            continue;
                        }
                    }
                    if requirement != original {
                        bodies.extend(program.instantiated_requirement_realizations.iter().filter_map(|(_, instance)| {
                            (instance.requirement_template == original
                                && instance.requirement_instance == requirement
                                && instance.provider_template == machine.symbol
                                && instance.conformance.is_valid()
                                && program.machine_trait_conformances.get(instance.conformance) == edge)
                                .then_some(SourceProviderBody { machine: instance.provider_instance, state: instance.realization_state })
                        }));
                    } else if let Some(entry) = program.machine_states(machine).first() {
                        bodies.push(SourceProviderBody { machine: machine.symbol, state: entry.symbol });
                    }
                }
            }
        }
        match bodies.as_slice() {
            [] => Ok(SourceProviderResolution::Retained("source constant selection has no exact closed source body")),
            [body] => {
                let Some(machine) = program.machine(body.machine) else {
                    return Ok(SourceProviderResolution::Retained("source constant choice lost its declared executable body"));
                };
                if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || machine.target.is_some()
                    || machine.target_family_interface
                {
                    return Ok(SourceProviderResolution::Retained("source constant choice requires a target observation or realization"));
                }
                if !program.machine_type_parameters(machine).is_empty()
                    || program.machine_states(machine).first().is_none_or(|entry| entry.symbol != body.state)
                {
                    return Ok(SourceProviderResolution::Retained("source constant choice needs its closed executable application"));
                }
                Ok(SourceProviderResolution::Body(*body))
            },
            _ => Err("source constant selection has competing original body correspondences".into()),
        }
    }
}

/// Construction scratch from the existing pending-position owners. Ordinary
/// runtime references and merely offered provider rows are not demand roots.
#[derive(Default)]
pub(crate) struct SourceConstantDemands {
    pub machines: Vec<SymbolHandle>,
    pub expressions: Vec<typed_trees::expression::ExpressionHandle>,
}

impl SourceConstantDemands {
    pub fn machine(&mut self, machine: SymbolHandle) {
        if machine.is_valid() && !self.machines.contains(&machine) {
            self.machines.push(machine);
        }
    }

    pub fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) {
        if expression.is_valid() && !self.expressions.contains(&expression) {
            self.expressions.push(expression);
        }
    }
}

/// A completed source fold retains the source application and supplying body,
/// not a native plan. The original graph owns occurrence handles after folding.
/// This association does not retain the consumed checked-closure Arc or its
/// invocation judgment, and is not a completed cross-target consumption roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConstantSupply {
    /// Source authority owns its original graph in the opaque binding receipt.
    pub body: crate::SelectedBuildTimeProviderBody,
}

/// The same one-shot continuation remains available when a source dependency
/// cannot yet be completed portably. A retained recipe is never a Const value.
pub enum SourceConstantOutcome {
    Completed {
        supplies: Vec<SourceConstantSupply>,
        /// Successful checked executions and concrete admission judgments.
        /// Earlier folds remain uncovered; admissions are not executions.
        checked_inputs: Vec<crate::CheckedConstantInput>,
    },
    Retained {
        continuation: crate::PreCheckEvaluation,
        reason: &'static str,
    },
}

pub(crate) enum SourceProviderBodies {
    Complete(Vec<crate::SelectedBuildTimeProviderBody>),
    Retained(&'static str),
}

pub(crate) fn source_provider_bodies(
    program: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
    demands: &SourceConstantDemands,
    selections: &[SourceProviderSelection],
) -> Result<SourceProviderBodies, Vec<diagnostics::Diagnostic>> {
    let facts = validation::derive_pre_flow_operator_selections(program);
    let admission = crate::BuildTimeAdmissionPlan::infer(program, None);
    let mut machines = admission.reachable_machine_symbols(&demands.machines);
    let mut expressions = Vec::new();
    let mut pending = demands.expressions.clone();
    while let Some(expression) = pending.pop() {
        if expressions.contains(&expression) {
            continue;
        }
        expressions.push(expression);
        pending.extend(crate::machine_execution::admission::expression_children(program, expression));
    }
    let uses = facts.resolved_expression_selections().collect::<Vec<_>>();
    let mut examined = Vec::new();
    let mut bodies = Vec::new();
    let mut owner = None;
    let trace_choices = std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some();
    if trace_choices {
        eprintln!("source-constant-choice-roster selections={} demanded-machines={} demanded-expressions={}", selections.len(), machines.len(), expressions.len());
    }
    // Source selection can extend a demanded closure by its genuine body. The
    // finite fact roster is visited once per original occurrence; no frontend
    // pass or evaluation is repeated to discover another provider.
    loop {
        let mut progress = false;
        for (expression, origin, requirement, _) in &uses {
            let key = (*expression, *origin);
            if examined.contains(&key)
                || !(expressions.contains(expression)
                    || origin.machine_symbol().is_some_and(|machine| machines.contains(&machine)))
            {
                continue;
            }
            examined.push(key);
            let mut selected = None;
            if trace_choices {
                eprintln!("source-constant-choice-use expression={expression:?} source={:?} origin={origin:?} requirement={requirement:?} name={}", program.expression_table.source_span(*expression), program.symbols.display_path(*requirement, "::"));
            }
            for selection in selections {
                if trace_choices {
                    let [subject, provider] = &selection.operands;
                    eprintln!("source-constant-choice-input subject={:?} subject-name={} kind={:?} application={:?} receiver={:?} provider={:?} provider-name={}", subject.declaration, subject.owner.symbols.display_path(subject.declaration, "::"), subject.kind, subject.application, subject.receiver_type, provider.declaration, provider.owner.symbols.display_path(provider.declaration, "::"));
                }
                match selection.resolve(program, *requirement).map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])? {
                    SourceProviderResolution::Unselected => {
                        if trace_choices {
                            eprintln!("source-constant-choice-resolution unselected");
                        }
                    },
                    SourceProviderResolution::Retained(reason) => {
                        if trace_choices {
                            eprintln!("source-constant-choice-resolution retained reason={reason}");
                        }
                        return Ok(SourceProviderBodies::Retained(reason));
                    },
                    SourceProviderResolution::Body(body) => {
                        if trace_choices {
                            eprintln!("source-constant-choice-resolution body={} state={:?}", program.symbols.display_path(body.machine, "::"), body.state);
                        }
                        if selected.is_some() {
                            return Err(vec![diagnostics::Diagnostic::error("source constant use has competing executed provider choices")]);
                        }
                        selected = Some((selection.clone(), body));
                    }
                }
            }
            let Some((selection, body)) = selected else {
                let boundary = program.machine(*requirement).is_some_and(|machine| {
                    machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                }) || program.public_trait_requirement_signature(*requirement).is_some_and(|member| member.owner.is_boundary)
                    || typed_trees::operator::declaration_by_symbol(program, *requirement).is_some_and(|operator| operator.is_boundary);
                if boundary {
                    // Declaration facts may retain an automatic semantic input
                    // provisionally, but never authorize its execution. The
                    // ordinary private preparation retains and checks the full
                    // intrinsic roster; admission and interpretation consume
                    // that same checked owner's exact occurrence query.
                    let automatic_input = facts
                        .requirement_realization(program, *requirement, *expression)
                        .ok()
                        .flatten()
                        .is_some_and(|meaning| meaning.is_compiler_intrinsic());
                    if !automatic_input {
                        return Err(vec![diagnostics::Diagnostic::error(
                            "source constant boundary application requires exact authored selection before evaluation",
                        )]);
                    }
                }
                continue;
            };
            let owner = owner.get_or_insert_with(|| {
                std::sync::Arc::new(typed_trees::FrozenTypedTrees::from(program.clone()))
            });
            let binding = SourceProviderBinding {
                selection, owner: owner.clone(), body,
                expression: *expression, origin: *origin, requirement: *requirement,
            };
            let row = crate::SelectedBuildTimeProviderBody::prepare_source(
                program, *expression, *origin, *requirement, binding, body,
            ).map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
            for machine in admission.reachable_machine_symbols(&[body.machine]) {
                if !machines.contains(&machine) {
                    machines.push(machine);
                    progress = true;
                }
            }
            bodies.push(row);
        }
        if !progress {
            break;
        }
    }
    // The evaluation domain still owns the executable declarations. Selected
    // source bodies cannot borrow a body outside that original source roster.
    if bodies.iter().any(|body| !domain.contains(body.provider_machine)) {
        return Ok(SourceProviderBodies::Retained("source constant body is outside its original evaluation roster"));
    }
    Ok(SourceProviderBodies::Complete(bodies))
}

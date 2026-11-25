//! Evaluates retained constant computations through the accepted invocation
//! bridge, then stores their snapshots in their original typed owner.
//! Dependency discovery reads identities only; it never interprets an
//! expression or chooses a result. Every job still passes typed preparation,
//! structural discharge, publication and hermetic invocation admission.

use std::sync::Arc;
use typed_trees_to_prepared_typed_trees::{PreparedValue, TargetInputs, ValueInputs};

use crate::source_state::TypedTrees;
use crate::source_state::constant::ConstDeclaration;
use crate::source_state::references::{ComputationDependencyRows, DeclarationSelection};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::compile_time_invocations::{
    FixedLayoutTargetScope, PureEvaluationRecord, SourceCompletionAccounting, invocation_error,
};

use super::super::demanded_values::{DemandedValues, RoundProgram};
use super::{References, decoded, family_position, key};

/// Keeps the machine that computes each unevaluated constant among the
/// program's machines, where the declarations it reaches can be read.
pub(in super::super) fn retain_evaluation_entries(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut entries = Vec::new();
    for declaration in program.const_declarations() {
        if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
            continue;
        }
        if !program
            .tables
            .machines
            .is_valid(declaration.evaluation_entry)
        {
            return Err(vec![Diagnostic::error(
                "retained constant lost its complete evaluation entry",
            )]);
        }
        let entry = program.tables.machines.get(declaration.evaluation_entry);
        if program.machine_by_symbol(entry.symbol).is_none() {
            entries.push(entry.clone());
        }
    }
    for entry in entries {
        program.push_machine(entry);
    }
    Ok(())
}

/// The deployments whose declaration closure reaches each constant, in
/// target order. A constant computed through an authored storage layout
/// runs once under each of them; others ignore this.
fn deployment_targets<'scope>(
    program: &TypedTrees,
    scopes: &'scope [FixedLayoutTargetScope],
    constants: &[SymbolHandle],
) -> Result<Vec<Vec<&'scope FixedLayoutTargetScope>>, String> {
    let mut reached = vec![Vec::new(); constants.len()];
    if constants.is_empty() {
        return Ok(reached);
    }
    let mut ordered = scopes
        .iter()
        .filter(|scope| {
            !scope.entry_declarations.is_empty() || !scope.provider_declarations.is_empty()
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.target.cmp(&right.target));
    if ordered.is_empty() {
        return Ok(reached);
    }
    let dependencies = ComputationDependencyRows::of(program);
    for scope in ordered {
        let selection = scope.demanded_declarations(program, &dependencies)?;
        for (position, constant) in constants.iter().enumerate() {
            let targets: &mut Vec<&FixedLayoutTargetScope> = &mut reached[position];
            if !selection.contains(*constant) {
                continue;
            }
            match targets.last() {
                Some(last) if last.target == scope.target => {
                    if last.facts != scope.facts {
                        return Err("layout scopes disagree on one target's immutable facts".into());
                    }
                }
                _ => targets.push(scope),
            }
        }
    }
    Ok(reached)
}

/// One demanded constant whose initializer is still a machine to run.
struct Job<'scope> {
    handle: arena::Handle<ConstDeclaration>,
    /// The constant. Every use names it, so it alone holds the value.
    symbol: [SymbolHandle; 1],
    /// The machine that computes the initializer, where evaluation starts.
    entry: [SymbolHandle; 1],
    /// This constant's row among the demanded constants' inputs.
    inputs: usize,
    deployments: Vec<&'scope FixedLayoutTargetScope>,
    /// Every declaration the initializer's computation reaches, sorted, as
    /// the demand order computed it.
    reached: Vec<SymbolHandle>,
}

/// The constants a request demands. Those written as a literal already have
/// their value; each of the others is a job.
pub(in super::super) struct Initializers<'scope> {
    jobs: Vec<Job<'scope>>,
    /// The inputs stage 04 prepares for every demanded constant, sorted by
    /// constant.
    values: Vec<ValueInputs>,
}

impl<'scope> Initializers<'scope> {
    /// Nominate computation from the same request the lowered-form checker
    /// will judge. This discovers identities only; neither membership nor an
    /// empty dependency row admits an invocation or supplies its result.
    pub(in super::super) fn demanded(
        program: &mut TypedTrees,
        selected: &DeclarationSelection,
        scopes: &'scope [FixedLayoutTargetScope],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut demanded = program
            .const_declarations()
            .iter()
            .filter(|constant| selected.contains(constant.symbol))
            .map(|constant| constant.symbol)
            .collect::<Vec<_>>();
        demanded.sort_unstable_by_key(|symbol| key(*symbol));
        demanded.dedup();
        let mut declarations = program
            .const_declarations()
            .iter()
            .filter(|decl| family_position(&demanded, decl.symbol).is_some())
            .collect::<Vec<_>>();
        declarations.sort_unstable_by_key(|decl| key(decl.symbol));
        let mut values = declarations
            .into_iter()
            .map(|declaration| {
                let inputs = if program
                    .expression_table
                    .expression_is_valid(declaration.authored_initializer)
                    && !program
                        .tables
                        .machines
                        .is_valid(declaration.evaluation_entry)
                {
                    super::request_references(program, declaration.authored_initializer)
                        .symbols
                        .into_iter()
                        .filter(|&symbol| {
                            program.symbols.symbols().nodes().is_valid(symbol)
                                && program.symbols.get(symbol).kind == symbols::SymbolKind::Const
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                ValueInputs::new(
                    program,
                    PreparedValue::Constant(declaration.symbol),
                    inputs,
                    if declaration.pending_target_observation {
                        TargetInputs::Deferred
                    } else {
                        TargetInputs::None
                    },
                )
            })
            .collect::<Vec<_>>();
        let found = program
            .roots
            .const_declarations
            .handles()
            .filter_map(|handle| {
                let constant = program.tables.const_declarations.get(handle);
                (family_position(&demanded, constant.symbol).is_some()
                    && constant.evaluated_value.is_none()
                    && program.tables.machines.is_valid(constant.evaluation_entry))
                .then(|| {
                    (
                        handle,
                        constant.symbol,
                        program
                            .tables
                            .machines
                            .get(constant.evaluation_entry)
                            .symbol,
                    )
                })
            })
            .collect::<Vec<_>>();
        if found.is_empty() {
            return Ok(Self {
                jobs: Vec::new(),
                values,
            });
        }
        let mut constants = program
            .const_declarations()
            .iter()
            .map(|constant| constant.symbol)
            .collect::<Vec<_>>();
        constants.sort_unstable_by_key(|symbol| key(*symbol));
        let symbols = found
            .iter()
            .map(|(_, symbol, _)| *symbol)
            .collect::<Vec<_>>();
        let deployments = deployment_targets(program, scopes, &symbols)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut jobs = Vec::with_capacity(found.len());
        for ((handle, symbol, entry), deployments) in found.into_iter().zip(deployments) {
            let inputs = family_position(&demanded, symbol).expect("constant job was demanded");
            let references = References {
                calls: vec![entry],
                ..Default::default()
            }
            .follow_calls(program);
            for dependency in references.symbols {
                if family_position(&constants, dependency).is_some() {
                    values[inputs].constants.push(dependency);
                }
            }
            jobs.push(Job {
                handle,
                symbol: [symbol],
                entry: [entry],
                inputs,
                deployments,
                reached: Vec::new(),
            });
        }
        Ok(Self {
            jobs,
            values,
        })
    }

    /// The inputs stage 04 prepares for every demanded constant, and the
    /// reach of each constant this request evaluated.
    pub(in super::super) fn into_inputs(self) -> (Vec<ValueInputs>, super::ConstantReaches) {
        let reaches = super::ConstantReaches::new(
            self.jobs
                .into_iter()
                .map(|job| (job.symbol[0], job.reached)),
        );
        (self.values, reaches)
    }

    fn evaluate_job(
        &mut self,
        value: usize,
        round: &RoundProgram,
        program: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        let job = &self.jobs[value];
        let (handle, symbol, entry) = (job.handle, job.symbol[0], job.entry[0]);
        let targets = job
            .deployments
            .iter()
            .map(|scope| (scope.target.as_str(), scope.facts.as_ref()))
            .collect::<Vec<_>>();
        let invocation = round.calls().prepare_entry(symbol, entry, &[])?;
        let module = invocation.accepted().module();
        let applications = module.scalars.scalar_applications.declarations();
        // A scalar produced with target inputs belongs to that deployment.
        // Publish the initializer so each use closes it with its own facts.
        use crate::compile_time_invocations::PreparedBuildMachineEntry;
        let observes_target = PreparedBuildMachineEntry::uses_target_layout(module);
        let scalar_result = matches!(module.entry,
            Some(terminal_psi::TargetMachineCallable::Machine(entry))
                if module.machine(entry).is_some_and(|machine| machine.result.scalar().is_some()));
        if observes_target && (targets.is_empty() || scalar_result)
            || applications.iter().any(|application| {
                matches!(
                    application,
                    terminal_psi::ScalarApplication::Evaluate { .. }
                )
            })
            || PreparedBuildMachineEntry::needs_selected_provider(module)
        {
            // The checked initializer is the recipe until a deployment
            // supplies its layout facts or selects its implementation.
            program
                .tables
                .const_declarations
                .get_mut(handle)
                .pending_target_observation = true;
            self.values[job.inputs].targets = TargetInputs::Deferred;
            return Ok(());
        }
        let first_receipt = accounting.receipts().len();
        let (outcomes, target_inputs) = invocation
            .invoke_recorded_in_targets(&targets, accounting)
            .map_err(invocation_error)?;
        // The final receipt join compares what the invocation read with the
        // program as it then stands. Values settle between the two, so the
        // receipt is tied here to the one program this round was judged in.
        for record in &accounting.receipts()[first_receipt..] {
            if let PureEvaluationRecord::Invocation(receipt) = record
                && !accounting
                    .constant_input(receipt)
                    .map_err(invocation_error)?
                    .is_some_and(|source| Arc::ptr_eq(source, round.source()))
            {
                return Err(invocation_error(
                    "constant receipt was judged in another program than its round's",
                ));
            }
        }
        self.values[job.inputs].targets = target_inputs;
        let carrier = program.tables.const_declarations.get(handle).declared_type;
        // A constant is one value for every deployment that reads it. Each
        // target's layout decides only where the bytes go, so the
        // semantic snapshot must be the same under every one of them.
        let mut folded = None;
        for value in &outcomes {
            let value = decoded(program, carrier, value).map_err(invocation_error)?;
            let encoding =
                application_identity::const_value::CanonicalConstIdentity::from_declaration_value(
                    &value,
                )
                .ok_or_else(|| {
                    invocation_error("constant result has no canonical declaration encoding")
                })?
                .encoding;
            match &folded {
                None => folded = Some((value, encoding)),
                Some((_, first)) if *first == encoding => {}
                Some(_) => {
                    return Err(invocation_error(
                        "constant result differs between the deployments that read it",
                    ));
                }
            }
        }
        let (value, encoding) =
            folded.ok_or_else(|| invocation_error("constant evaluation returned no result"))?;
        let node = program
            .materialize_canonical_const_value(carrier, &value)
            .ok_or_else(|| {
                invocation_error("constant result does not materialize in its declared carrier")
            })?;
        let expression = program.expression_table.insert(node);
        let span = program
            .tables
            .const_declarations
            .get(handle)
            .initializer_source_span;
        program.expression_table.set_source_span(expression, span);
        let declaration = program.tables.const_declarations.get_mut(handle);
        declaration.materialized_initializer = expression;
        declaration.evaluated_value = Some(encoding.into());
        Ok(())
    }
}

impl DemandedValues for Initializers<'_> {
    fn len(&self) -> usize {
        self.jobs.len()
    }

    fn holders(&self, value: usize) -> &[SymbolHandle] {
        &self.jobs[value].symbol
    }

    fn roots(&self, value: usize) -> &[SymbolHandle] {
        &self.jobs[value].entry
    }

    fn reaches(&mut self, _program: &TypedTrees, value: usize, reached: &[SymbolHandle]) {
        self.jobs[value].reached = reached.to_vec();
    }

    fn description(&self, program: &TypedTrees, value: usize) -> String {
        format!(
            "constant `{}`",
            program.symbols.name(self.jobs[value].symbol[0])
        )
    }

    fn source_span(&self, program: &TypedTrees, value: usize) -> Option<source_text::SourceSpan> {
        program
            .symbols
            .symbol_source_span(self.jobs[value].symbol[0])
    }

    fn evaluate(
        &mut self,
        value: usize,
        round: &RoundProgram,
        program: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        let symbol = self.jobs[value].symbol[0];
        self.evaluate_job(value, round, program, accounting)
            .map_err(|mut diagnostics| {
                let name = program.symbols.name(symbol);
                let declaration_span = program.symbols.symbol_source_span(symbol);
                for error in diagnostics
                    .iter_mut()
                    .filter(|diagnostic| diagnostic.is_error())
                {
                    error.message = format!("constant `{name}` evaluation: {}", error.message);
                    // Name the request while pointing at the operation that failed.
                    if error.resolved_location.is_none()
                        && error.source_span.unwrap_or_default()
                            == source_text::SourceSpan::default()
                    {
                        error.source_span = declaration_span;
                    }
                }
                diagnostics
            })
    }
}

//! Evaluate every compile-time value a request demands, each once and after
//! the values it reads.
//!
//! Before stage 04 freezes the typed trees, the driver computes four kinds
//! of value: the layout a policy plans for a storage type on a target, the
//! access plan of a domain's policy, a const argument or array extent written
//! as an expression, and a constant's initializer. Any of them can need any
//! other. `[u8; width()]` needs the constant that `width` returns, and a
//! constant built through `Buffer<capacity()>` needs that argument. No fixed
//! order of the four kinds serves both programs, so the order comes from
//! what each value reads ([`DemandOrder`]).
//!
//! A kind tells this owner three things about each of its values
//! ([`DemandedValues`]): the declarations that hold it, the declarations its
//! computation starts from, and how to evaluate it and store the result.
//! [`settle_in_demand_order`] treats every value alike from there.

use std::sync::Arc;

use checked_program_to_psi as production;
use declaration_scope::PackageDeclarationScope;
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::SymbolHandle;
use typed_trees_to_prepared_typed_trees::ValueInputs;

use super::demand_order::{DemandOrder, Round};
use super::{
    FixedLayoutTargetScope, InvocationSourceConsent, PlannedCall, PreparedBuildMachineProgram,
    SourceCompletionAccounting,
};
use crate::source_state::TypedTrees;
use crate::source_state::references::ComputationDependencyRows;

/// The request whose compile-time values are evaluated, with the custody
/// every evaluation runs under.
pub(crate) struct ValueDemand<'request> {
    pub compilation: production::TerminalSourceCompilation<'request>,
    pub layout_scopes: &'request [FixedLayoutTargetScope],
    pub authority: Option<Arc<PackageDeclarationScope>>,
    pub source_consent: Option<Arc<dyn InvocationSourceConsent>>,
}

/// The values of one kind that a request demands, numbered from zero.
pub(super) trait DemandedValues {
    fn len(&self) -> usize;

    /// The declarations that hold `value`. Another value that reads one of
    /// them waits until this one has its result.
    fn holders(&self, value: usize) -> &[SymbolHandle];

    /// The declarations the computation of `value` starts from.
    fn roots(&self, value: usize) -> &[SymbolHandle];

    /// Tells the kind every declaration `value` reaches from its roots,
    /// sorted. Called once, before any evaluation.
    fn reaches(&mut self, _program: &TypedTrees, _value: usize, _reached: &[SymbolHandle]) {}

    /// Names `value` for a person, as in "constant `WIDTH`".
    fn description(&self, program: &TypedTrees, value: usize) -> String;

    /// Where `value` is written.
    fn source_span(&self, program: &TypedTrees, value: usize) -> Option<SourceSpan>;

    /// Whether a value of this kind whose computation reaches its own holder
    /// waits for itself, and so is refused as a circle. A kind answers
    /// `false` only when its value is judged with its own result absent and
    /// every use inside that computation which needs the result is refused
    /// at the use: then reaching the holder alone asks nothing of it.
    fn waits_for_itself(&self) -> bool {
        true
    }

    /// Whether a settled value of this kind serves a request recorded on its
    /// holders (`frontier::refresh_evaluated_requests`).
    fn serves_recorded_requests(&self) -> bool {
        false
    }

    /// Names, before any value of `round` is evaluated, the calls `values`
    /// (this kind's values in the round) will make through `round.calls()`
    /// or `round.layout_policies()`. The round freezes their callers
    /// together (`calls::round_callers`). A value whose call cannot be named
    /// yet is left out and judged alone when it is evaluated; naming a call
    /// never refuses anything. A kind that judges its calls in a program of
    /// its own may share that program's callers here instead.
    fn plan_round(
        &mut self,
        _values: &[usize],
        _round: &RoundProgram,
        _program: &TypedTrees,
    ) -> Vec<PlannedCall> {
        Vec::new()
    }

    /// Evaluates `value` in `round`'s program and stores the result in
    /// `program`. Called once per value, after every value it waits for.
    fn evaluate(
        &mut self,
        value: usize,
        round: &RoundProgram,
        program: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>>;
}

/// The program every value of one round is judged in.
///
/// No value of a round waits for another, so storing one result cannot
/// change what another reads. They share this one immutable copy, and the
/// callers of their calls are frozen together in one more
/// ([`RoundProgram::share_calls`]). The next round copies the program
/// again, with this round's results in it.
pub(super) struct RoundProgram {
    number: usize,
    calls: PreparedBuildMachineProgram,
    layout_policies: PreparedBuildMachineProgram,
}

impl RoundProgram {
    fn of(
        program: &TypedTrees,
        demand: &ValueDemand<'_>,
        number: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let calls = PreparedBuildMachineProgram::prepare_for(
            program,
            demand.authority.clone(),
            demand.compilation.purpose,
            demand.source_consent.clone(),
        )?;
        Ok(Self {
            number,
            layout_policies: calls.for_layout_policies(),
            calls,
        })
    }

    /// Counts rounds from zero. A kind that derives data from the round's
    /// program derives it once per number.
    pub(super) fn number(&self) -> usize {
        self.number
    }

    pub(super) fn source(&self) -> &Arc<TypedTrees> {
        self.calls.source()
    }

    pub(super) fn calls(&self) -> &PreparedBuildMachineProgram {
        &self.calls
    }

    pub(super) fn layout_policies(&self) -> &PreparedBuildMachineProgram {
        &self.layout_policies
    }

    /// Freezes the callers of `plans` together for every call this round
    /// judges through `calls()` or `layout_policies()`.
    fn share_calls(&mut self, plans: Vec<PlannedCall>) {
        if plans.len() > 1 {
            self.calls.share_round_calls(plans);
            self.layout_policies = self.calls.for_layout_policies();
        }
    }
}

/// What evaluating a request's values produced.
pub(crate) struct EvaluatedValues {
    /// The inputs stage 04 prepares: each computed argument, then each
    /// demanded constant.
    pub inputs: Vec<ValueInputs>,
    /// The declarations each evaluated constant's computation reached, as
    /// the order saw them. The receipt join compares exactly these.
    pub constant_reaches: super::constants::ConstantReaches,
}

/// Evaluates everything `demand` asks of `program`.
pub(crate) fn evaluate_demanded_values(
    program: &mut TypedTrees,
    demand: &ValueDemand<'_>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<EvaluatedValues, Vec<Diagnostic>> {
    super::constants::retain_evaluation_entries(program)?;
    // Finding the demanded values changes no declaration's dependencies (a
    // layout demand only refreshes refusal rows), so one inventory of them
    // serves every kind and the demand order.
    let dependencies = ComputationDependencyRows::of(program);
    let mut layouts =
        super::fixed_layouts::Layouts::demanded(program, demand.layout_scopes, &dependencies)?;
    let demanded = super::demanded_declarations(
        program,
        demand.compilation,
        demand.layout_scopes,
        &dependencies,
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut plans = super::domain_access_plans::AccessPlans::demanded(program, &demanded);
    let mut arguments = super::constants::Arguments::demanded(program, &demanded);
    let mut initializers = super::constants::Initializers::demanded(
        program,
        &demanded,
        demand.layout_scopes,
        &dependencies,
    )?;
    settle_in_demand_order(
        program,
        demand,
        &mut [&mut layouts, &mut plans, &mut arguments, &mut initializers],
        dependencies,
        accounting,
    )?;
    let mut inputs = arguments.into_inputs();
    let (constants, constant_reaches) = initializers.into_inputs();
    inputs.extend(constants);
    Ok(EvaluatedValues {
        inputs,
        constant_reaches,
    })
}

fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Every declaration reachable from `start` through the declarations each
/// one names (`rows`, sorted by declaration), sorted.
fn reached(
    program: &TypedTrees,
    rows: &[(SymbolHandle, Vec<SymbolHandle>)],
    start: &[SymbolHandle],
) -> Vec<SymbolHandle> {
    let mut pending = start.to_vec();
    let mut reached = Vec::new();
    while let Some(symbol) = pending.pop() {
        if !program.symbols.symbols().nodes().is_valid(symbol) {
            continue;
        }
        let symbol = program.refusal_declaration(symbol);
        if let Err(position) = reached.binary_search_by_key(&key(symbol), |symbol| key(*symbol)) {
            reached.insert(position, symbol);
            if let Ok(index) = rows.binary_search_by_key(&key(symbol), |(symbol, _)| key(*symbol)) {
                pending.extend(rows[index].1.iter().copied());
            }
        }
    }
    reached
}

/// Evaluates every value of `kinds` once, each after the values it waits for.
/// `dependencies` must be built from `program` as it is now; they are
/// dropped before the first value changes it.
///
/// A value waits for another when its computation reaches a declaration that
/// holds the other. Values that wait for nothing unsettled form a round, are
/// judged in one copy of the program and store their results. Values that
/// all wait on each other are refused, each named in the diagnostic.
fn settle_in_demand_order(
    program: &mut TypedTrees,
    demand: &ValueDemand<'_>,
    kinds: &mut [&mut dyn DemandedValues],
    dependencies: ComputationDependencyRows,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    // A value's position in the order, as its kind and its number there.
    let places = kinds
        .iter()
        .enumerate()
        .flat_map(|(kind, values)| (0..values.len()).map(move |value| (kind, value)))
        .collect::<Vec<_>>();
    if places.is_empty() {
        return Ok(());
    }
    let mut reads = Vec::with_capacity(places.len());
    for &(kind, value) in &places {
        let reached = reached(program, dependencies.rows(), kinds[kind].roots(value));
        kinds[kind].reaches(program, value, &reached);
        reads.push(reached);
    }
    drop(dependencies);
    let mut order = {
        let holders = places
            .iter()
            .map(|&(kind, value)| kinds[kind].holders(value))
            .collect::<Vec<_>>();
        let waits_for_itself = places
            .iter()
            .map(|&(kind, _)| kinds[kind].waits_for_itself())
            .collect::<Vec<_>>();
        DemandOrder::new(&holders, &reads, &waits_for_itself)
    };
    drop(reads);
    // Every round holds at least one value and settles each value it holds,
    // and a round with no value refuses the request. So the loop takes at
    // most `places.len()` rounds, and no value is evaluated twice.
    for number in 0.. {
        let round = match order.next_round() {
            Round::Finished => break,
            Round::NoFirstStep(circle) => {
                return Err(no_first_step(program, kinds, &places, &circle));
            }
            Round::Evaluate(round) => round,
        };
        let mut judged = RoundProgram::of(program, demand, number)?;
        // Each kind names the calls its values in this round will make
        // before any of them is evaluated.
        let mut plans = Vec::new();
        for (kind, values) in kinds.iter_mut().enumerate() {
            let in_round = round
                .iter()
                .filter_map(|&position| {
                    let (owner, value) = places[position];
                    (owner == kind).then_some(value)
                })
                .collect::<Vec<_>>();
            if !in_round.is_empty() {
                plans.extend(values.plan_round(&in_round, &judged, program));
            }
        }
        judged.share_calls(plans);
        let mut served = false;
        for position in round {
            let (kind, value) = places[position];
            kinds[kind].evaluate(value, &judged, program, accounting)?;
            served |= kinds[kind].serves_recorded_requests();
            order.settle(position);
        }
        if served {
            super::frontier::refresh_evaluated_requests(program)?;
        }
    }
    Ok(())
}

/// The refusal for values that each wait for the next, the last for the
/// first. It points at the first and adds a note at each other one.
fn no_first_step(
    program: &TypedTrees,
    kinds: &[&mut dyn DemandedValues],
    places: &[(usize, usize)],
    circle: &[usize],
) -> Vec<Diagnostic> {
    let describe = |position: usize| {
        let (kind, value) = places[position];
        kinds[kind].description(program, value)
    };
    let span = |position: usize| {
        let (kind, value) = places[position];
        kinds[kind].source_span(program, value)
    };
    let Some((&first, rest)) = circle.split_first() else {
        return vec![Diagnostic::error(
            "compile-time evaluation has no first step",
        )];
    };
    let name = describe(first);
    let message = if rest.is_empty() {
        format!("{name} needs its own value, so its evaluation has no first step")
    } else {
        let mut message = format!("{name} needs ");
        for &position in rest {
            message.push_str(&describe(position));
            message.push_str(", which needs ");
        }
        message.push_str(&name);
        message.push_str(": none of them can be evaluated first");
        message
    };
    let mut error = Diagnostic::error(message);
    if let Some(span) = span(first) {
        error = error.with_source_span(span);
    }
    let mut diagnostics = vec![error];
    for &position in rest {
        let mut note = Diagnostic::note(format!("{} is part of that circle", describe(position)));
        if let Some(span) = span(position) {
            note = note.with_source_span(span);
        }
        diagnostics.push(note);
    }
    diagnostics
}

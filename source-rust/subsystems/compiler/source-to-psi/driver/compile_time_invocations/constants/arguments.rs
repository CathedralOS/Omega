//! Evaluate const arguments written as expressions (`Buffer<capacity()>`)
//! the way a constant's initializer is evaluated, then give each argument
//! the canonical value typing gives a literal.
//!
//! The declared carrier and canonical value identify the closed application.
//! Each expression becomes a zero-argument source machine. Ordinary checking
//! judges that machine with the parameter's carrier before execution. The
//! materialized value updates the original type argument through
//! [`data_applications::settle_const_argument`].

use std::sync::Arc;
use typed_trees::data_applications;
use typed_trees_to_prepared_typed_trees::{PreparedValue, TargetInputs, ValueInputs};

use crate::source_state::TypedTrees;
use crate::source_state::constant_recipes::{
    ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
    retain_constant_recipes,
};
use crate::source_state::data::TypeParameterKind;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::references::DeclarationSelection;
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use super::super::demanded_values::{DemandedValues, RoundProgram};
use super::super::typed_invocation::TypedInvocationDeclaration;
use super::{decoded, insert};
use crate::compile_time_invocations::{
    InvocationTarget, PlannedCall, PreparedBuildMachineProgram, SourceCompletionAccounting,
};

/// One value required by a type and still written as an expression.
struct Job {
    argument: TypeReferenceHandle,
    expression: ExpressionHandle,
    /// The argument's exact builtin carrier.
    carrier: TypeReferenceHandle,
    /// The demanded declaration the application is written in. The
    /// evaluation runs as its request.
    owner: SymbolHandle,
    /// Every declaration the application is written in, sorted. Each one
    /// carries the request's row and is refused where it is read until the
    /// argument has its value.
    holders: Vec<SymbolHandle>,
    /// The symbols the expression names, with the machines it calls.
    roots: Vec<SymbolHandle>,
    /// The application and parameter that ask for the value, if any.
    application: Option<(SymbolHandle, String)>,
}

impl Job {
    fn description(&self, program: &TypedTrees) -> String {
        match &self.application {
            Some((applied, parameter)) => {
                format!(
                    "const argument for `{parameter}` of `{}`",
                    program.symbols.name(*applied)
                )
            }
            None => "computed array extent".to_owned(),
        }
    }
}

fn pending_argument(
    program: &TypedTrees,
    jobs: &mut Vec<Job>,
    argument: TypeReferenceHandle,
    carrier: TypeReferenceHandle,
    application: Option<(SymbolHandle, &str)>,
) {
    let types = &program.type_reference_table;
    let TypeReferenceNode::ConstExpression(expression) = *types.type_reference(argument) else {
        return;
    };
    let builtin = types
        .contains_type_reference(carrier)
        .then(|| types.type_reference(carrier));
    let carrier_atom = match builtin {
        Some(TypeReferenceNode::Named { symbol, .. }) => program.symbols.builtin_type_atom(*symbol),
        _ => None,
    };
    if !(matches!(carrier_atom, Some(symbols::BuiltinTypeAtom::Integer(_)))
        || (application.is_some() && carrier_atom == Some(symbols::BuiltinTypeAtom::Bool)))
        || matches!(
            program.expression_table.expression(expression),
            crate::source_state::expression::ExpressionNode::Integer(_)
                | crate::source_state::expression::ExpressionNode::Boolean(_)
        )
        || program.pending_const_range_endpoints.contains(&expression)
        || !crate::source_state::domain::index_argument_is_closed(program, argument)
        || jobs.iter().any(|known| known.argument == argument)
    {
        return;
    }
    jobs.push(Job {
        argument,
        expression,
        carrier,
        owner: SymbolHandle::invalid(),
        holders: Vec::new(),
        roots: Vec::new(),
        application: application.map(|(symbol, parameter)| (symbol, parameter.to_owned())),
    });
}

/// The const arguments of type applications that are closed expressions and
/// not literals, each with the parameter it answers. A carrier other than a
/// builtin integer or `bool` (a domain, a data type, a type parameter of the
/// same header) asks more than a value of the carrier, so its argument is
/// left as written and stays refused where it is demanded.
fn pending(program: &TypedTrees) -> Vec<Job> {
    let mut found = Vec::new();
    let mut array_arguments = Vec::new();
    let mut generic_arguments = Vec::new();
    let expression = |argument: &TypeReferenceHandle| match program
        .type_reference_table
        .type_reference(*argument)
    {
        TypeReferenceNode::ConstExpression(expression) => Some(*expression),
        _ => None,
    };
    let has_expression = |arguments: &[TypeReferenceHandle]| {
        arguments
            .iter()
            .any(|argument| expression(argument).is_some())
    };
    let mut collect =
        |declaration: SymbolHandle,
         parameters: arena::RootRows<'_, crate::source_state::data::TypeParameter>,
         arguments: &[TypeReferenceHandle]| {
            if parameters.len() != arguments.len() {
                return;
            }
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if let TypeParameterKind::Const { type_reference } = parameter.kind
                    && expression(argument).is_some()
                {
                    generic_arguments.push(*argument);
                    pending_argument(
                        program,
                        &mut found,
                        *argument,
                        type_reference,
                        Some((declaration, parameter.name.as_str())),
                    );
                }
            }
        };
    for (_, node) in program.type_reference_table.type_reference_rows() {
        match node {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Expression(argument),
                ..
            } => {
                if let Some(expression) = expression(argument) {
                    array_arguments.push((*argument, expression));
                }
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if has_expression(arguments)
                    && program.symbols.symbols().nodes().is_valid(*base_symbol)
                {
                    collect(
                        *base_symbol,
                        program.declaration_type_parameters(*base_symbol),
                        arguments,
                    );
                }
            }
            TypeReferenceNode::Constrained { constraints, .. } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let crate::source_state::types::TypeConstraintNode::Domain(domain) =
                        constraint
                        && domain.subject
                            == crate::source_state::types::DomainConstraintSubject::Declared
                        && has_expression(&domain.arguments)
                        && let Some(definition) = program.domain_definition(domain.symbol)
                    {
                        collect(
                            domain.symbol,
                            crate::source_state::domain::index_parameters(program, definition),
                            &domain.arguments,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    // Array extents derived from a generic argument keep that parameter's
    // declared carrier. Only written array expressions supply their own.
    let coordinate =
        |argument: TypeReferenceHandle| (argument.arena_index(), argument.generation());
    generic_arguments.sort_unstable_by_key(|&argument| coordinate(argument));
    for (argument, expression) in array_arguments {
        if generic_arguments
            .binary_search_by_key(&coordinate(argument), |&argument| coordinate(argument))
            .is_err()
        {
            let carrier = program
                .expression_table
                .constructed_value_type_reference(expression);
            pending_argument(program, &mut found, argument, carrier, None);
        }
    }
    found
}

/// The symbols an expression names, with the machines it calls.
#[derive(Default)]
struct Named(Vec<SymbolHandle>);
impl Extend<SymbolHandle> for Named {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.0.extend(symbols);
    }
}
impl crate::source_state::references::TypedReferences for Named {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        self.0.push(symbol);
    }
}

/// The zero-argument machine whose value is `job`'s expression, added to
/// `probe`: `machine @const-argument<..>() -> Carrier { expression }`.
fn entry(probe: &mut TypedTrees, job: &Job) -> Result<SymbolHandle, String> {
    let name = match &job.application {
        Some((applied, parameter)) => format!(
            "@const-argument<{}::{parameter}>",
            probe.symbols.display_path(*applied, "::"),
        ),
        None => format!(
            "@array-extent<{}>",
            probe
                .expression_table
                .source_span(job.expression)
                .span
                .start
        ),
    };
    TypedInvocationDeclaration::new(probe, job.owner, &name)
        .define(probe, job.carrier, Vec::new())
        .return_expression(probe, job.expression)
}

/// Every const argument written as an expression in a declaration the
/// request demands, with the inputs stage 04 prepares for each.
pub(in super::super) struct Arguments {
    jobs: Vec<Job>,
    values: Vec<ValueInputs>,
    /// The program the current round judges its arguments in, when it has
    /// more than one.
    round: Option<ArgumentRound>,
}

/// One copy of a round's program holding the generated entry of every
/// argument the round evaluates (`entry`). The callers of their calls are
/// frozen together in it.
struct ArgumentRound {
    number: usize,
    program: PreparedBuildMachineProgram,
    /// Each argument's generated entry, by its value number.
    entries: Vec<(usize, SymbolHandle)>,
}

impl Arguments {
    pub(in super::super) fn demanded(
        program: &TypedTrees,
        demanded: &DeclarationSelection,
    ) -> Self {
        let found = pending(program);
        if found.is_empty() {
            return Self {
                jobs: Vec::new(),
                values: Vec::new(),
                round: None,
            };
        }
        let owners = crate::source_state::references::collect_program_operand_owners(program);
        let jobs = found
            .into_iter()
            .filter_map(|job| {
                let mut holders = Vec::new();
                for owner in owners
                    .type_owners(job.argument)
                    .filter(|owner| owner.is_valid())
                {
                    insert(&mut holders, program.refusal_declaration(owner));
                }
                // The application is its declaration's to carry; an argument no
                // demanded declaration holds is not asked for by this request.
                let owner = owners
                    .type_owners(job.argument)
                    .filter(|owner| owner.is_valid())
                    .map(|owner| program.refusal_declaration(owner))
                    .find(|owner| demanded.contains(*owner))?;
                let mut named = Named::default();
                crate::source_state::references::visit_expression(
                    program,
                    &mut named,
                    job.expression,
                );
                Some(Job {
                    owner,
                    holders,
                    roots: named.0,
                    ..job
                })
            })
            .collect::<Vec<_>>();
        let values = jobs
            .iter()
            .map(|job| {
                ValueInputs::new(
                    program,
                    PreparedValue::StaticArgument {
                        owner: job.owner,
                        site: job.argument,
                        expression: job.expression,
                        carrier: job.carrier,
                    },
                    Vec::new(),
                    TargetInputs::None,
                )
            })
            .collect();
        Self {
            jobs,
            values,
            round: None,
        }
    }

    pub(in super::super) fn into_inputs(self) -> Vec<ValueInputs> {
        self.values
    }

    fn evaluate_job(
        &mut self,
        value: usize,
        round: &RoundProgram,
        program: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), String> {
        let job = &self.jobs[value];
        let shared = self
            .round
            .as_ref()
            .filter(|shared| shared.number == round.number())
            .and_then(|shared| {
                let (_, entry) = shared.entries.iter().find(|(owner, _)| *owner == value)?;
                let plan = shared.program.plan_entry(job.owner, *entry, &[]).ok()?;
                shared.program.judge_shared(&plan)
            });
        // Without a shared caller, or when the shared judgment fails, the
        // argument is judged in a copy holding only its own entry, so its
        // refusal is the one it gets alone.
        let invocation = match shared {
            Some(invocation) => invocation,
            None => {
                let mut probe = TypedTrees::clone(round.source());
                let entry = entry(&mut probe, job)?;
                let prepared = round.calls().extended_to(Arc::new(probe));
                prepared.entry(job.owner, entry, &[])?
            }
        };
        let module = invocation.accepted().module();
        use crate::compile_time_invocations::PreparedBuildMachineEntry;
        if job.application.is_none()
            && (PreparedBuildMachineEntry::uses_target_layout(module)
                || PreparedBuildMachineEntry::needs_selected_provider(module))
        {
            let symbol = self::entry(program, job)?;
            let contexts =
                program
                    .constant_expression_contexts
                    .insert_many([ConstantExpressionContext {
                        owner: job.owner,
                        result_type: job.carrier,
                        evaluation_entry: symbol,
                        evaluation_expression: job.expression,
                    }]);
            let source_span = program.expression_table.source_span(job.expression);
            retain_constant_recipes(
                program,
                [RetainedConstantRecipe {
                    position: RetainedConstantPosition::ConstApplication {
                        site: job.argument,
                        root: job.expression,
                    },
                    contexts,
                    machine: job.owner,
                    source_span,
                    ..Default::default()
                }],
            );
            self.values[value].targets = TargetInputs::Deferred;
            return Ok(());
        }
        let result = invocation.invoke_recorded(
            InvocationTarget {
                name: None,
                layout_target: None,
                carrier_layout: None,
                address_model: None,
            },
            accounting,
        )?;
        use application_identity::const_value::DecodedCanonicalConstValue;
        let spelling = match decoded(program, job.carrier, &result)? {
            DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
            DecodedCanonicalConstValue::Boolean(value) => value.to_string(),
            _ => return Err("the value is neither an integer nor a Boolean".into()),
        };
        data_applications::settle_const_argument(program, job.argument, &spelling)
    }
}

impl DemandedValues for Arguments {
    fn len(&self) -> usize {
        self.jobs.len()
    }

    fn holders(&self, value: usize) -> &[SymbolHandle] {
        &self.jobs[value].holders
    }

    fn roots(&self, value: usize) -> &[SymbolHandle] {
        &self.jobs[value].roots
    }

    /// Adds every argument of the round to one copy of its program and
    /// freezes their callers together there. The calls are not judged in
    /// the round's own program, so this kind names none to it.
    fn plan_round(
        &mut self,
        values: &[usize],
        round: &RoundProgram,
        _program: &TypedTrees,
    ) -> Vec<PlannedCall> {
        self.round = None;
        if values.len() < 2 {
            return Vec::new();
        }
        let mut probe = TypedTrees::clone(round.source());
        let mut entries = Vec::with_capacity(values.len());
        for &value in values {
            // A failed entry can leave part of a declaration in the copy, so
            // every argument of the round is then judged alone.
            let Ok(symbol) = entry(&mut probe, &self.jobs[value]) else {
                return Vec::new();
            };
            entries.push((value, symbol));
        }
        let mut program = round.calls().extended_to(Arc::new(probe));
        let plans = entries
            .iter()
            .filter_map(|&(value, symbol)| {
                program.plan_entry(self.jobs[value].owner, symbol, &[]).ok()
            })
            .collect::<Vec<_>>();
        program.share_round_calls(plans);
        self.round = Some(ArgumentRound {
            number: round.number(),
            program,
            entries,
        });
        Vec::new()
    }

    fn reaches(&mut self, program: &TypedTrees, value: usize, reached: &[SymbolHandle]) {
        self.values[value].constants = reached
            .iter()
            .copied()
            .filter(|&symbol| program.symbols.get(symbol).kind == symbols::SymbolKind::Const)
            .collect();
    }

    fn description(&self, program: &TypedTrees, value: usize) -> String {
        let job = &self.jobs[value];
        format!(
            "the {} in `{}`",
            job.description(program),
            program.symbols.name(job.owner)
        )
    }

    fn source_span(&self, program: &TypedTrees, value: usize) -> Option<source_text::SourceSpan> {
        Some(
            program
                .expression_table
                .source_span(self.jobs[value].expression),
        )
    }

    /// The holders carry a type-expression or array-extent row until the
    /// argument settles, and a later value may read them.
    fn serves_recorded_requests(&self) -> bool {
        true
    }

    fn evaluate(
        &mut self,
        value: usize,
        round: &RoundProgram,
        program: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        self.evaluate_job(value, round, program, accounting)
            .map_err(|reason| {
                let job = &self.jobs[value];
                vec![
                    Diagnostic::error(format!("{}: {reason}", job.description(program)))
                        .with_source_span(program.expression_table.source_span(job.expression)),
                ]
            })
    }
}

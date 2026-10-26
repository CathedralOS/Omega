//! COMPTIME STAGE 1 -- const evaluation of FIXED-ARRAY LENGTHS.
//!
//! Each closed extent expression is folded by checked scalar evaluation or
//! retained as a compiler-private, zero-runtime-parameter entry. This entry
//! carries the complete expression and its original selections; authored
//! callees may take ordinary closed arguments. Length jobs use the existing
//! admission, dependency and selected-provider continuation for that entry.
//! A successful job publishes a literal extent. No unresolved recipe grants
//! layout or execution, and final product checking retains ordinary duties.
//!
//! TERMINATION: no new rule -- the language's existing discipline (no general
//! recursion, loops carry decreases) covers const callees. The interpreter
//! entry adds a ~100k-step fuel cap purely as defense-in-depth; exceeding it
//! is a compile error here.
//!
//! DETERMINISM: the interpreter width-adjusts the terminal value to the
//! machine's declared integer return type (the same wrap-on-write it applies
//! differentially), so the result carries TARGET integer semantics, never
//! host widths.

use crate::build_time_evaluation::EvaluationDomain;
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use crate::BuildTimeAdmissionPlan;

/// Detached type-arena nodes retain their identity but do not own evaluation
/// work. Snapshot only lengths reached from the current declaration roster.
pub(crate) fn pending_array_lengths(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Vec<(
    TypeReferenceHandle,
    symbols::SymbolHandle,
    String,
    source::SourceSpan,
)> {
    domain
        .references(typed)
        .type_references
        .into_iter()
        .filter_map(
            |handle| match typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::FixedArray {
                    length:
                        FixedArrayLength::ConstCall {
                            machine,
                            name,
                            source_span,
                        },
                    ..
                } => Some((handle, *machine, name.as_str().to_owned(), *source_span)),
                _ => None,
            },
        )
        .collect()
}

pub fn evaluate_const_array_lengths(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_const_array_lengths_in(typed, selection_authority, &EvaluationDomain::all())
}

pub(crate) fn evaluate_const_array_lengths_in(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    LengthJobs::new(
        selection_authority,
        domain,
        LengthProviderSelection::Independent,
    )
    .evaluate(typed)
    .map(|_| ())
}

/// Evaluate independent calls before deferring only the remaining work.
/// A pending provider-dependent field must not suppress an ordinary length
/// needed by the already-admitted Build invocation.
pub(crate) fn evaluate_independent_lengths(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &EvaluationDomain,
) -> Result<bool, Vec<Diagnostic>> {
    LengthJobs::new(authority, domain, LengthProviderSelection::Pending).evaluate(typed)
}

pub(crate) fn evaluate_selected_array_lengths(
    typed: &mut TypedTrees,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let crate::SelectedBuildTimeOperators {
        operators,
        provider_bodies,
    } = selected;
    // Admission and execution must see the same selected realization. When a
    // selected use means an ordinary checked provider body, rebind its
    // occurrence to the exact provider entry state on one private copy: the
    // provider joins the ordinary call closure and runs its real body, while
    // the caller's tree and its source-owned handles stay untouched.
    let selected_program =
        crate::machine_execution::selected_operators::prepare_selected_execution(
            typed,
            provider_bodies,
            operators,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut execution = selected_program.unwrap_or_else(|| typed.clone());
    LengthJobs::new(
        authority,
        domain,
        LengthProviderSelection::Selected(operators),
    )
    .evaluate(&mut execution)?;
    for (type_reference, _, _, _) in pending_array_lengths(typed, domain) {
        if let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(value),
            ..
        } = execution
            .type_reference_table
            .type_reference(type_reference)
        {
            typed
                .type_reference_table
                .set_fixed_array_length(type_reference, *value);
        }
    }
    Ok(())
}

/// Provider settlement is an existing lifecycle boundary, not an alternate
/// evaluator. Each phase uses the same demanded preparation and checking.
enum LengthProviderSelection<'selection> {
    Pending,
    Independent,
    Selected(&'selection [crate::SelectedBuildTimeBinaryOperator]),
}

struct LengthJobs<'selection> {
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &'selection EvaluationDomain,
    providers: LengthProviderSelection<'selection>,
    active: Vec<(TypeReferenceHandle, String)>,
}

impl<'selection> LengthJobs<'selection> {
    fn new(
        authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &'selection EvaluationDomain,
        providers: LengthProviderSelection<'selection>,
    ) -> Self {
        Self {
            authority,
            domain,
            providers,
            active: Vec::new(),
        }
    }

    /// Only successful source-owned lengths leave the private job graph. A
    /// job's type dependencies must finish before its body is checked or run;
    /// declaration order and unrelated pending product fields cannot supply
    /// zero lengths or grant execution. Final product checking still owns all
    /// declarations after the ordinary continuation finishes.
    fn evaluate(&mut self, typed: &mut TypedTrees) -> Result<bool, Vec<Diagnostic>> {
        let pending = pending_array_lengths(typed, self.domain);
        if pending.is_empty() {
            return Ok(false);
        }
        // The original execution roster is the authority boundary. Prepare
        // its static applications once, then keep all prerequisite results in
        // this one private arena. Shared leaves become literal slots rather
        // than being interpreted again for each path through a diamond.
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, self.domain)?;
        let mut execution = prepared.typed().clone();
        let mut deferred = false;
        let mut diagnostics = Vec::new();
        for (handle, _, _, _) in pending {
            match self.evaluate_length(&mut execution, handle) {
                Ok(Some(value)) => typed
                    .type_reference_table
                    .set_fixed_array_length(handle, value),
                Ok(None) => deferred = true,
                Err(errors) => diagnostics.extend(errors),
            }
        }
        if diagnostics.is_empty() {
            Ok(deferred)
        } else {
            Err(diagnostics)
        }
    }

    fn evaluate_length(
        &mut self,
        typed: &mut TypedTrees,
        handle: TypeReferenceHandle,
    ) -> Result<Option<usize>, Vec<Diagnostic>> {
        let (machine, name, source) = match typed.type_reference_table.type_reference(handle) {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(value),
                ..
            } => {
                return Ok(Some(*value));
            }
            TypeReferenceNode::FixedArray {
                length:
                    FixedArrayLength::ConstCall {
                        machine,
                        name,
                        source_span,
                    },
                ..
            } => (*machine, name.as_str().to_owned(), *source_span),
            _ => {
                return Err(vec![Diagnostic::error(
                    "length job lost its exact array type",
                )]);
            }
        };
        if let Some(position) = self.active.iter().position(|(active, _)| *active == handle) {
            let chain = self.active[position..]
                .iter()
                .map(|(reference, name)| format!("{name} at {reference:?}"))
                .chain(std::iter::once(format!("{name} at {handle:?}")))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![
                Diagnostic::error(format!("fixed-array length dependency cycle: {chain}"))
                    .with_source_span(source),
            ]);
        }
        self.active.push((handle, name.clone()));
        let result = self.prepare_and_invoke(typed, machine, source);
        self.active.pop();
        let value = result.map_err(|mut errors| {
            for error in &mut errors {
                error.message = format!(
                    "fixed-array length `{}`: const evaluation of `{name}` failed: {}",
                    typed.type_reference_table.display_name(handle),
                    error.message,
                );
                if error.source_span.is_none() {
                    error.source_span = Some(source);
                }
            }
            errors
        })?;
        if let Some(value) = value {
            typed
                .type_reference_table
                .set_fixed_array_length(handle, value);
        }
        Ok(value)
    }

    fn prepare_and_invoke(
        &mut self,
        typed: &mut TypedTrees,
        machine: symbols::SymbolHandle,
        source: source::SourceSpan,
    ) -> Result<Option<usize>, Vec<Diagnostic>> {
        let domain = EvaluationDomain::all().demanded_roots(typed, &[machine]);
        let mut execution = domain.execution_program(typed);
        let admission = BuildTimeAdmissionPlan::infer(&execution, self.authority.clone());
        if matches!(self.providers, LengthProviderSelection::Pending) {
            let facts = validation::derive_pre_flow_operator_selections(&execution);
            if admission.closure_needs_selected_execution(&execution, machine, &facts) {
                return Ok(None);
            }
        }
        // Type closure includes the declarations of length-producing helpers.
        // Visit fresh prerequisite jobs before an active type encountered
        // through that transitive closure so a cycle reports its whole chain.
        let mut deferred = false;
        let mut dependencies = pending_array_lengths(&execution, &EvaluationDomain::all());
        dependencies.sort_by_key(|(handle, _, _, _)| {
            self.active.iter().any(|(active, _)| active == handle)
        });
        for (dependency, _, _, _) in dependencies {
            if self.evaluate_length(typed, dependency)?.is_none() {
                deferred = true;
            }
        }
        if deferred {
            return Ok(None);
        }

        // A newly literal type can close a static application's remaining
        // tuple. Resume the same specialization owner in the same arena; it
        // appends real instances and cannot replay an evaluated leaf.
        typed_trees_to_typed_trees::specialize_static_machine_calls(typed)?;
        let domain = EvaluationDomain::all().demanded_roots(typed, &[machine]);
        execution = domain.execution_program(typed);

        let references = typed_trees::references::collect_program_references(&execution);
        let operators = match &self.providers {
            LengthProviderSelection::Selected(operators) => operators
                .iter()
                .filter(|row| references.expressions.contains(&row.expression))
                .cloned()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        if matches!(self.providers, LengthProviderSelection::Pending)
            && crate::const_evaluation::range_endpoints::pending_endpoint_calls_need_operator_selection(
                &execution, self.authority.clone(), &EvaluationDomain::all(),
            )?
        {
            return Ok(None);
        }
        crate::const_evaluation::range_endpoints::evaluate_selected_range_endpoints(
            &mut execution,
            self.authority.clone(),
            crate::SelectedBuildTimeOperators {
                operators: &operators,
                provider_bodies: &[],
            },
            &EvaluationDomain::all(),
        )?;
        let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
        let checking = typed_trees_to_checked_trees::CheckingRequest::new();
        let (execution, preparation) =
            typed_trees_to_typed_trees::prepare_typed_trees(execution, &preparation_request)?;
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(execution, preparation, &checking)?;
        // Admission follows selected executable meaning, while the checked
        // program retains its public requirement calls. Terminal installs only
        // those independently admitted numeric supplies; it does not rewrite
        // the source or borrow another occurrence's selected implementation.
        let admitted = crate::machine_execution::selected_operators::prepare_selected_execution(
            &checked.typed,
            &[],
            &operators,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let execution = admitted.as_ref().unwrap_or(&checked.typed);
        let root = execution.machine_by_symbol(machine).ok_or_else(|| {
            vec![Diagnostic::error(
                "array length lost its exact selected machine",
            )]
        })?;
        require_zero_argument_entry(execution, root)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let admission = BuildTimeAdmissionPlan::infer(execution, self.authority.clone())
            .with_selected_operators(execution, &operators)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        // The immutable checked entry consumes the same sealed occurrence row,
        // never a new provider selection or an unadmitted requirement body.
        admission
            .require_const_evaluable_invocation(
                execution,
                root,
                &[],
                crate::BuildTimeInvocationCustody::Source(source),
                false,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let prepared = crate::PreparedBuildMachineProgram::from_checked(checked);
        let entry = prepared
            .entry(machine)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let value = prepared
            .evaluate_checked_constant(&entry, &operators)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let execution = prepared.typed();
        let root = execution.machine_by_symbol(machine).ok_or_else(|| {
            vec![Diagnostic::error(
                "checked length job lost its exact machine",
            )]
        })?;
        decode_length_result(execution, root, value)
            .map(Some)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}

fn require_zero_argument_entry(typed: &TypedTrees, machine: &Machine) -> Result<(), String> {
    let parameter_count = entry_state_parameter_count(typed, machine);
    if parameter_count > 0 {
        let machine_name = machine.name.as_str();
        return Err(format!(
            "machine `{machine_name}` takes {parameter_count} parameter(s); a const-evaluated \
             array length must call a zero-argument machine (const arguments are not supported yet)"
        ));
    }
    Ok(())
}

fn decode_length_result(
    typed: &TypedTrees,
    machine: &Machine,
    value: terminal_interpreter::TerminalExecutionResult,
) -> Result<usize, String> {
    let value = match value {
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer { value, .. },
        ) => match value {
            semantic_vocabulary::IntegerValue::Signed(value) => BigInt::from_i128(value),
            semantic_vocabulary::IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        },
        _ => {
            let primitive = entry_state(typed, machine)
                .and_then(|state| typed.primitive_type_reference(state.return_type));
            return Err(match primitive {
                Some(primitive) => format!(
                    "machine `{}` returns `{}`, not an integer type",
                    machine.name.as_str(),
                    primitive.name(),
                ),
                None => format!(
                    "machine `{}` does not declare an integer return type",
                    machine.name.as_str(),
                ),
            });
        }
    };
    if value.is_negative() {
        return Err(format!(
            "the call returned {value}, but an array length must be a non-negative integer"
        ));
    }
    value
        .to_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("the call returned {value}, which does not fit an array length"))
}

pub fn evaluate_zero_argument_machine(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    machine_name: &str,
    position: &str,
    custody: Option<crate::BuildTimeInvocationCustody>,
) -> Result<BigInt, String> {
    let machine = typed
        .realized_machine_named(machine_name)
        .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;

    // Stage 1 scope: a zero-argument machine. (This also discharges the
    // "no `&mut`/out parameters" portion of the build-time contract.)
    let parameter_count = entry_state_parameter_count(typed, machine);
    if parameter_count > 0 {
        return Err(format!(
            "machine `{machine_name}` takes {parameter_count} parameter(s); a const-evaluated \
             {position} must call a zero-argument machine (const arguments are not supported yet)"
        ));
    }

    let value = match custody {
        Some(custody) => admission.evaluate_const_evaluable_machine_for_invocation(
            typed,
            machine_name,
            Vec::new(),
            custody,
        )?,
        None => admission.evaluate_const_evaluable_machine(typed, machine_name, Vec::new())?,
    };
    decode_integer_result(typed, machine, value)
}

/// Interpreter integers carry fixed-width bits in an i64 slot. Decode using
/// the admitted callee's return carrier before a const position narrows them:
/// u64::MAX is positive, while the same bits returned as i64 denote -1.
/// Array lengths, generic arguments and range endpoints share this conversion;
/// range normalization must never see a host-signed reinterpretation.
pub(crate) fn decode_integer_result(
    typed: &TypedTrees,
    machine: &Machine,
    value: crate::BuildTimeValue,
) -> Result<BigInt, String> {
    let machine_name = machine.name.as_str();
    let primitive = entry_state(typed, machine)
        .and_then(|state| typed.primitive_type_reference(state.return_type));
    if let (Some(primitive), crate::BuildTimeValue::Int(value)) = (primitive, &value)
        && primitive.accepts_integer_literal()
    {
        return Ok(if primitive.is_signed_integer() {
            BigInt::from_i64(*value)
        } else {
            BigInt::from_u64(*value as u64)
        });
    }
    match primitive {
        Some(primitive) => Err(format!(
            "machine `{machine_name}` returns `{}`, not an integer type",
            primitive.name()
        )),
        None => Err(format!(
            "machine `{machine_name}` does not declare an integer return type"
        )),
    }
}

/// The parameter count of the machine's entry state (the body of a free
/// machine; mirrors the interpreter's entry-state selection: the state named
/// like the machine's leaf, else the first state).
fn entry_state_parameter_count(typed: &TypedTrees, machine: &Machine) -> usize {
    entry_state(typed, machine)
        .map(|state| typed.state_parameters(state).len())
        .unwrap_or(0)
}

fn entry_state<'a>(
    typed: &'a TypedTrees,
    machine: &Machine,
) -> Option<&'a typed_trees::state::State> {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = typed.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}

//! Transport a current exclusive-referent goal through terminal calls.
//! Each node retains the original proposition's complete projected subjects.
//! Leaves read independently established current values, with the ordinary
//! dependency checks for completed helper calls. Entry snapshots and the
//! terminal callee's own postcondition cannot close its proof.

use crate::checks::current_scalar_values::CurrentScalarValues;
use checked_trees::{CheckFacts, FlowCallFact, FlowExitFact};
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
};
use validation::{
    CanonicalPlace, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
};

pub(super) struct ReferentGoal {
    owner: SymbolHandle,
    expression: ExpressionHandle,
    reads: Vec<ExpressionHandle>,
    pub(super) initial: Vec<Vec<CanonicalPlace>>,
}

impl ReferentGoal {
    pub(super) fn new(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        expression: ExpressionHandle,
        remaining: &mut usize,
    ) -> Option<Self> {
        let owner = machine.symbol;
        let trace = std::env::var_os("OMEGA_STRUCT_TRACE").is_some();
        let entry = program.machine_states(machine).first()?;
        if !crate::checks::contracts::prover::has_builtin_operators(
            program,
            &facts.operators,
            expression,
        ) {
            if trace {
                eprintln!(
                    "STRUCT terminal-referent declined owner={} reason=operator-meaning",
                    machine.name
                );
            }
            return None;
        }
        let mut tree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, expression, &mut tree);
        if tree.iter().any(|operand| {
            matches!(
                program.expression_table.expression(*operand),
                ExpressionNode::Call(_)
            ) || super::super::is_result_reference(program, machine, *operand)
        }) {
            if trace {
                eprintln!(
                    "STRUCT terminal-referent declined owner={} reason=call-or-result",
                    machine.name
                );
            }
            return None;
        }
        let mut reads = Vec::new();
        let mut places = Vec::new();
        for operand in tree {
            super::super::demands::charge(remaining)?;
            // Bind scalar reads, not their enclosing comparison or an entry
            // snapshot. The scoped arithmetic reader still refuses other free
            // operands, unsupported operators and unformed observations.
            if !matches!(
                program.expression_table.expression(operand),
                ExpressionNode::Name(_) | ExpressionNode::Member(_)
            ) {
                continue;
            }
            let Some(place) = validation::canonical_place_from_expression(program, operand) else {
                continue;
            };
            let Some(parameter) = program.state_parameters(entry).iter().find(|parameter| {
                place.root == PlaceRoot::Symbol(parameter.symbol)
                    && validation::reference_shell(program, parameter.type_reference)
                        .is_some_and(|shell| shell.access.is_exclusive())
            }) else {
                continue;
            };
            // Field-only coordinates have no selector evaluation to replay during
            // substitution. Their suffix can only shrink at a completing call.
            if !place
                .segments
                .iter()
                .all(|segment| matches!(segment, PlaceSegment::Field { .. }))
            {
                return None;
            }
            let Some(reference) = validation::parameter_expression_result_type_reference(
                program,
                entry.symbol,
                program.state_parameters(entry),
                operand,
            ) else {
                continue;
            };
            if super::super::fixed_integer(program, reference).is_none() {
                continue;
            }
            if !validation::state_reference_parameter_binding_is_stable(
                program,
                machine,
                entry,
                parameter.symbol,
            ) {
                if trace {
                    eprintln!(
                        "STRUCT terminal-referent declined owner={} reason=unstable-reference parameter={:?}",
                        machine.name, parameter.symbol
                    );
                }
                return None;
            }
            reads.push(operand);
            places.push(place);
        }
        if reads.is_empty() {
            if trace {
                eprintln!(
                    "STRUCT terminal-referent declined owner={} reason=no-scalar-reference parameters={:?}",
                    machine.name,
                    program.state_parameters(entry)
                );
            }
            return None;
        }
        let origins = crate::flow::activation_parameter_origins(program, machine);
        // This answer is shared by all owner exits. Include every authored body,
        // even an unreachable body, exactly as the result completion closure does.
        let mut initial = Vec::new();
        for state in program.machine_states(machine) {
            super::super::demands::charge(remaining)?;
            let origins = crate::flow::state_parameter_origins(program, machine, state, &origins);
            let mapped = places
                .iter()
                .map(|place| {
                    *remaining = remaining.checked_sub(place.segments.len().checked_add(1)?)?;
                    let source = place.root.as_symbol()?;
                    let mut matches = origins.iter().filter(|(entry, _)| *entry == source);
                    let (_, destination) = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    if !validation::state_reference_parameter_binding_is_stable(
                        program,
                        machine,
                        state,
                        *destination,
                    ) {
                        return None;
                    }
                    Some(CanonicalPlace {
                        root: PlaceRoot::Symbol(*destination),
                        segments: place.segments.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(places) = mapped else {
                if trace {
                    eprintln!(
                        "STRUCT terminal-referent declined owner={} reason=initial-subjects state={:?} origins={origins:?}",
                        machine.name, state.symbol
                    );
                }
                return None;
            };
            initial.push(places);
        }

        if trace {
            eprintln!(
                "STRUCT terminal-referent prepared owner={} reads={reads:?} initial={initial:?}",
                machine.name
            );
        }
        Some(Self {
            owner,
            expression,
            reads,
            initial,
        })
    }

    pub(super) fn forward<'program>(
        program: &'program dyn validation::CheckingInput,
        frames: &validation::CallFrameResolver<'program>,
        machine: &'program Machine,
        state: &State,
        call: &FlowCallFact,
        places: &[CanonicalPlace],
        remaining: &mut usize,
    ) -> Option<Vec<CanonicalPlace>> {
        let target = validation::call_target_state(program, call.target_symbol)?;
        let correspondence = frames.normal_return_reference_correspondence(
            machine,
            state,
            call,
            program.state_parameters(target),
        );
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT terminal-referent forward caller={} target={:?} writes_complete={} subjects={places:?} correspondence={correspondence:?}",
                machine.name, target.symbol, call.normal_return_writes_complete
            );
        }
        let correspondence = correspondence?;
        *remaining = remaining.checked_sub(correspondence.len())?;
        let comparison_work = correspondence
            .iter()
            .try_fold(1usize, |work, (_, source)| {
                work.checked_add(source.segments.len().checked_add(1)?)
            })?;
        *remaining = remaining.checked_sub(comparison_work.checked_mul(places.len())?)?;
        let places = places
            .iter()
            .map(|place| {
                *remaining = remaining.checked_sub(place.segments.len().checked_add(1)?)?;
                let mut suppliers = correspondence.iter().filter_map(|(parameter, source)| {
                    if source.root != place.root {
                        return None;
                    }
                    let suffix = place.segments.strip_prefix(source.segments.as_slice())?;
                    Some(CanonicalPlace {
                        root: PlaceRoot::Symbol(*parameter),
                        segments: suffix.to_vec(),
                    })
                });
                let place = suppliers.next()?;
                suppliers.next().is_none().then_some(place)
            })
            .collect::<Option<Vec<_>>>()?;
        // Roots range over declaration parameters, and every path is a suffix of
        // an original field path. Together with the complete node key, this bounds
        // cyclic exploration without unrolling or dropping a changed substitution.
        Some(places)
    }

    pub(super) fn proves_leaf<'program>(
        &self,
        program: &'program dyn validation::CheckingInput,
        frames: &validation::CallFrameResolver<'program>,
        machine: &'program Machine,
        state: &'program State,
        exit: &FlowExitFact,
        places: &[CanonicalPlace],
    ) -> bool {
        let Some(mut values) = CurrentScalarValues::at(
            program,
            machine,
            state,
            exit.statement_index,
            exit.transition_target,
            Some(frames),
        ) else {
            return false;
        };
        let returned = super::super::exit_return_expression(program, exit);
        if returned.is_valid() {
            values.retire_operand_effects(returned);
        }
        let bindings = self
            .reads
            .iter()
            .zip(places)
            .map(|(read, place)| {
                Some(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(*read),
                    value: values.scalar_value_at_place(place)?,
                })
            })
            .collect::<Option<Vec<_>>>();
        let Some(bindings) = bindings else {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "STRUCT terminal-referent leaf-unavailable owner={} state={:?} subjects={places:?}",
                    machine.name, state.symbol
                );
            }
            return false;
        };
        let goal = ScopedArithmeticExpression {
            owner: self.owner,
            expression: self.expression,
            bindings,
        };
        let proven = values.proves(program, machine, &goal);
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT terminal-referent leaf owner={} state={:?} goal={goal:?} proven={proven}",
                machine.name, state.symbol
            );
        }
        proven
    }
}

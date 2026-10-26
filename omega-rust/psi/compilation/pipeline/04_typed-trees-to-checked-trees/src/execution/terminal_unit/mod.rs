//! Executable Unit plans from checked ownership, control and call facts.
//!
//! We build executable source plans from already-checked ownership, control, and
//! call facts. A typed body can be legal without fitting a Terminal producer yet;
//! these catalogs describe implementation coverage, not additional language rules.
//! The lowerer must receive a complete plan, never a partly recognized body.
//!
//! Start at `build_checked_unit_effect_plans`, the checking
//! stage's entry into this module (`build_checked_unit_effect_plans` is its
//! test-only form without call frames). It also publishes the partial-affine
//! roster: an ordinary body whose return edge owes a projected owned
//! parameter's residual complement is the same candidate kept beside those
//! rows, and a callee like any other. Nominal caller metadata is built after
//! body publication; it cannot replace that body's cleanup coordinates.
//! We first collect boundary
//! signatures and ordinary single-state candidates through control.rs. A
//! single-state body has one producer: control/statement_sequence.rs walks the
//! authored statements in order and emits each one's operations — stores,
//! borrowed-window moves and repairs, selected operator and FMA applications,
//! structural and scalar bindings, calls and completion — after the prefix of
//! trivial affine locals that calls/affine_locals.rs establishes. Machine input
//! storage arrives through explicit parameters. We build composed candidates through
//! composed_control/assembly.rs, and reconcile composed calls against the completed
//! signatures. The composed builders include specialized control shapes and the
//! general state_graph.rs route; their current coverage differs, so a failed
//! candidate is not permission to omit its unsupported statements.
//!
//! Local construction is only the first gate. Ordinary and composed entries share
//! one immutable availability roster. We check operations once, then propagate
//! unavailable targets through reverse call edges. If A calls B and B calls an
//! unavailable C, rejecting B also rejects A without rechecking A's body. Checking
//! only A's own statements, or
//! pruning each catalog independently, would leave a seemingly complete root with
//! an unlowerable transitive call. Scalar calls can borrow an ordinary scalar-result
//! body from this same roster; their argument, claim and contract checks still use
//! the exact scalar signature. Boundary and structural-result calls retain their
//! own availability checks. Only after
//! pruning do we retain the referenced structural types and their transitive shapes.
//!
//! Every checked-body machine left without a plan carries one gap
//! (`plan_gaps.rs`): the construct its builder could not lower, the cleanup a
//! state owed, or the call whose callee has no body, at its authored span.
//! Lowering and entry establishment report the gap of a body they demand,
//! following callee gaps to the construct that removed the chain. The
//! builders still return `Option`; the construction trace names the phase
//! and statement where they stopped.
//!
//! Partial cleanup retains its residual body owner. Nominal actions remain
//! distinct from trivial discards in the shared normal-exit transaction;
//! their caller metadata owns no second executable body or disposal schedule.
//! Replacement joins those same selected dispositions into the statement's
//! operation stream: evaluate the RHS, dispose the old occurrence, then store
//! or rebind. `cleanup/replacements.rs` completes the death marker from source
//! custody once the hook catalog exists; the lowerer never recognizes an
//! initializer/call/store pattern to manufacture replacement semantics.
//! An owned local changes its current value home. Borrowed storage instead
//! retains its lender's home and exact restoration obligation. Treating these
//! as one extraction-and-repair operation invents debt for ordinary locals.
//!
//! # Executable companions
//!
//! Existing source examples are the executable companions to this explanation:
//! src/tests/flow/terminal_unit/calls.rs checks mixed case payload/view edges and
//! affine return disposal. In the sibling checked-trees-to-lowered-psi crate,
//! src/tests/composed_unit_transitive_internal_calls.rs follows Root::enter through
//! its helpers and rejects substituted targets and missing plans;
//! src/tests/byte_write_loop.rs exercises borrowed buffers and scalar-case returns.
//! Read those controls when widening a route. The owning source/Terminal coverage
//! contract is ../../terminal-production/README.md relative to this crate.
//!
//! # Erased parameters
//!
//! An `[erased]` binding occurrence
//! (contracts.md#explicit-erased-bindings) stays in the typed signature and in
//! proof identity but owns no ABI position. Every signature builder here
//! (calls/signatures.rs, calls/call_operations.rs, ../scalar/mod.rs)
//! skips it, and every caller-side argument producer
//! (values/scalar/computations.rs, values/scalar/call_lowering.rs,
//! flow/transfers/scalar_values/calls.rs) numbers its dense argument ordinals over
//! the retained positions only, so the two sides agree without a shared table.
//! `position` and `source_position` stay the authored typed-signature index and
//! therefore become sparse after a strip: the Terminal consumer rejoins each
//! retained parameter to `state_parameters(state)[position]` for its type, symbol
//! and custody, so renumbering densely would bind the wrong source parameter.
//! `strips_erased_parameter` decides which bindings may be stripped at all.
//! The dense scalar value namespace that `CheckedScalarExpression::Parameter`
//! indexes is the same partition: `values::scalar::occupies_scalar_position`
//! excludes erased bindings there, so a body reading a retained parameter after
//! an erased one gets the shifted position and a runtime read of the erased one
//! finds no position. The consumer, checked-trees-to-lowered-psi, reconstructs
//! every one of these partitions independently from the typed relevance
//! (unit/attached_unit/parameters.rs, expression_preparation/qualifications.rs,
//! scalar_lowering/computations/calls.rs)
//! and never trusts the plan's counts. What remains open is a contract that
//! names an erased binding (`requires n < bound`): Terminal contract
//! propositions have no proof-only value for it, so
//! pass/relevance/erased_parameter_proof_only stays a checked-only canary while
//! its two RUN siblings execute natively (canary_suite layouts_and_pending
//! erased_parameter_* tests).

use crate::execution::terminal_unit::cleanup::build_nominal_affine_unit_cleanup_machine;
use crate::execution::terminal_unit::control::{
    LocalConstructionTrace, build_boundary_machine, build_checked_machine_residual_parts,
    build_static_boundary_requirements,
};
use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
use crate::execution::terminal_unit::types::ShapeCollector;
use std::collections::{BTreeMap, BTreeSet};

use checked_trees::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan,
    CheckedClosedSumPayloadTransferPlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedGuardedJumpPlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralResultPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupRequirementPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitPlanGapReason,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, PermissionProvenance, SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::{
    data::{DataMember, DataShapeKind},
    domain::ProofFact,
    expression::ExpressionNode,
    signature::{SignatureContractKind, StateParameter},
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

mod atomic_operations;
mod borrowed_windows;
pub(crate) mod byte_operands;
pub(crate) mod calls;
mod candidate_closure;
pub(crate) mod cleanup;
mod composed_control;
pub(crate) mod control;
mod initialized_locals;
mod layout_queries;
mod local_storage;
pub(crate) mod plan_gaps;
mod providers;
mod receiver_aliases;
use validation::reference_result_custody as reference_results;
mod scalar_locals;
pub(crate) mod scalar_return_expressions;
mod scalar_targets;
pub(crate) mod state_graph;
mod storage_transfers;
mod structural_scalar_store;
mod structural_value_calls;
pub(crate) use structural_value_calls::retain as retain_structural_value_calls;
pub(crate) mod types;

pub(crate) use calls::structural_computation_argument;
pub(crate) use types::{is_reference, strips_erased_parameter};

/// Present when the planning pass ends in the candidate closure, which
/// resolves each scalar call to its callee's composed or ordinary
/// scalar-result Unit body. A pass without it (provider settlement) has no closure
/// to defer to and admits only callees it can settle itself.
#[derive(Clone, Copy)]
pub(crate) struct ScalarCalleeClosure;

pub(crate) fn build_checked_unit_effect_plans(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    boundary_signature_reaches: &[(symbols::SymbolHandle, language_semantics::ServiceReachRowId)],
    scalar_callees: ScalarCalleeClosure,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> (
    CheckedUnitEffectPlans,
    CheckedPartialAffineUnitCleanupPlans,
    structural_value_calls::PreparedCalls,
) {
    let mut shapes = ShapeCollector::new(program);
    let mut prepared_calls = structural_value_calls::PreparedCalls::default();
    let mut boundary_gaps = Vec::new();
    let (boundary_machines, dynamic_slots, boundary_symbols) =
        phase_profile::scope("05.d.8_10.a.3.1.boundary_and_dynamic", || {
            let mut boundary_machines = program
                .machines()
                .iter()
                // An opaque callable's signature is portable even when its physical
                // binding is not selected. Supply selects admission, not whether Psi
                // may retain that declaration's contract.
                .filter(|machine| !machine.supplies_checked_body())
                .filter_map(|machine| {
                    shapes.take_unselected_opaque();
                    match build_boundary_machine(program, facts, &mut shapes, machine) {
                        Ok(plan) => Some(plan),
                        Err(reason) => {
                            let reason = shapes.take_unselected_opaque().map_or(
                                CheckedUnitPlanGapReason::Unsupported(reason),
                                CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation,
                            );
                            boundary_gaps.push(plan_gaps::plan_gap(
                                program,
                                machine.symbol,
                                plan_gaps::GapSite::Machine,
                                reason,
                            ));
                            None
                        }
                    }
                })
                .collect::<Vec<_>>();
            boundary_machines.extend(build_static_boundary_requirements(
                program,
                facts,
                boundary_signature_reaches,
                &mut boundary_gaps,
                &mut shapes,
            ));
            let dynamic_slots = control::build_dynamic_slot_plans(program, &mut shapes);
            let boundary_symbols = boundary_machines
                .iter()
                .map(|plan| plan.machine)
                .collect::<Vec<_>>();
            (boundary_machines, dynamic_slots, boundary_symbols)
        });
    // Each checked body takes the one route its shape selects; a builder that
    // cannot plan it leaves the gap where it stopped. An ordinary body whose
    // return edge owes the residual complement of a projected owned
    // parameter move is published through the partial-affine carrier: the
    // same plan, kept beside the residual rows its return edge closes, so
    // the root-only roster never disposes that parameter whole. It stays a
    // callee like any ordinary body; only its return edge differs.
    let mut candidates = Vec::new();
    let mut composed_machines = Vec::new();
    let mut partial_affine_machines = Vec::new();
    let mut construction_gaps = BTreeMap::new();
    phase_profile::scope("05.d.8_10.a.3.2.candidate_bodies", || {
        for machine in program
            .machines()
            .iter()
            // A bodied `boundary machine` is a checked adapter: its body is
            // authored in-package, so it is planned like any other checked body.
            .filter(|machine| machine.supplies_checked_body())
        {
            let trace = LocalConstructionTrace::default();
            shapes.take_unselected_opaque();
            let mut attempt_calls = structural_value_calls::PreparedCalls::default();
            let route = phase_profile::scope("05.d.8_10.a.3.2a.body_route", || {
                body_route(program, machine)
            });
            let planned = match route {
                BodyRoute::Sequence => {
                    phase_profile::scope("05.d.8_10.a.3.2b.sequence_body", || {
                        build_checked_machine_residual_parts(
                            program,
                            facts,
                            scalar_callees,
                            &mut shapes,
                            &mut attempt_calls,
                            machine,
                            call_frames,
                            &trace,
                        )
                        .and_then(
                            |(
                                mut plan,
                                residual_affine_discards,
                                has_projected_parameter_moves,
                            )| {
                                structural_value_calls::prepare_state(
                                    program,
                                    facts,
                                    &mut shapes,
                                    &mut attempt_calls,
                                    plan.machine,
                                    plan.state,
                                    &structural_value_calls::StateCallInputs {
                                        parameters: &plan.structural_parameters,
                                        entry_claims: &plan.entry_claims,
                                        operations: &plan.operations,
                                        trivial_locals: &plan.trivial_affine_locals,
                                    },
                                )?;
                                trace.phase("single-state body: value call source custody");
                                structural_value_calls::consume_caller_results(
                                    program,
                                    plan.machine,
                                    plan.state,
                                    &mut plan.operations,
                                    attempt_calls.state_schedules(plan.machine, plan.state),
                                )?;
                                if has_projected_parameter_moves {
                                    partial_affine_machines.push(
                                        CheckedPartialAffineUnitCleanupMachinePlan {
                                            machine: plan,
                                            residual_affine_discards,
                                        },
                                    );
                                } else {
                                    candidates.push(plan);
                                }
                                Some(())
                            },
                        )
                    })
                }
                BodyRoute::Graph => phase_profile::scope("05.d.8_10.a.3.2c.graph_body", || {
                    state_graph::build_traced(
                        program,
                        facts,
                        scalar_callees,
                        &mut shapes,
                        &mut attempt_calls,
                        machine,
                        call_frames,
                        &trace,
                    )
                    .map(|plan| composed_machines.push(plan))
                }),
            };
            if planned.is_some() {
                prepared_calls.append(machine.symbol, attempt_calls);
            }
            if planned.is_none() {
                let mut gap = trace.gap(program, machine.symbol);
                // A shape the collector could not supply because the build
                // selects no representation for an opaque is the program's
                // missing selection; it displaces the phase the builder was in.
                if let Some(opaque) = shapes.take_unselected_opaque() {
                    gap.reason = CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation(opaque);
                }
                construction_gaps.insert(symbol_key(machine.symbol), gap);
            }
        }
    });

    let planned = planned_machines(&candidates, &composed_machines, &partial_affine_machines);
    let mut gaps = construction_gaps
        .into_iter()
        .filter(|(key, _)| !planned.contains(key))
        .map(|(_, gap)| gap)
        .collect::<Vec<_>>();
    gaps.extend(boundary_gaps);
    // Storage planning precedes executable hook selection. Complete the
    // contextual actions against the same immutable callable entry roster,
    // before mutating either body catalog. Entry metadata borrows the real
    // bodies; no temporary ordinary plan stands in for a state graph.
    let _s_cleanup = phase_profile::guard("05.d.8_10.a.3.3.cleanup_selections");
    let targets = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .map(CheckedUnitEffectMachinePlan::callable_entry)
        .chain(
            composed_machines
                .iter()
                .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
        )
        .collect::<Vec<_>>();
    // Ordinary and partial-affine sequence bodies share one completion
    // contract: a partial-affine body is an ordinary body whose owned
    // parameter some call projected, so it receives the same checked
    // normal-exit rows.
    let mut select_sequence_cleanup = |plan: &CheckedUnitEffectMachinePlan| {
        let temporary = cleanup::temporary_actions::operations(
            program,
            facts,
            &targets,
            &mut shapes,
            plan.machine,
            plan.state,
            &plan.structural_parameters,
            &plan.operations,
        )?;
        // Owned qualified arguments are not a pre-construction scalar-control
        // veto. Admission requires these complete current-exit rows, including
        // moved locals and selected residual owners; no candidate is published
        // until their ordinary cleanup actions have been selected and installed.
        let normal =
            cleanup::normal_edges::sequence_cleanups(program, facts, &targets, &mut shapes, plan)?;
        if plan.scalar_control.is_none() {
            let [row] = shapes.cleanup_control.normal_exits(normal) else {
                return None;
            };
            if row.exit.kind != checked_trees::FlowClaimJoinExitKind::FinalExpression {
                return None;
            }
        }
        Some((temporary, normal))
    };
    let mut temporary_selections = candidates
        .iter()
        .map(&mut select_sequence_cleanup)
        .collect::<Vec<_>>()
        .into_iter();
    let mut partial_affine_selections = partial_affine_machines
        .iter()
        .map(|plan| select_sequence_cleanup(&plan.machine))
        .collect::<Vec<_>>()
        .into_iter();
    let mut composed_selections = composed_machines
        .iter()
        .map(|plan| {
            plan.states
                .iter()
                .map(|state| {
                    let temporary = cleanup::temporary_actions::operations(
                        program,
                        facts,
                        &targets,
                        &mut shapes,
                        plan.machine,
                        state.state,
                        &state.structural_parameters,
                        &state.operations,
                    )
                    .ok_or((state.state, "temporary cleanup actions"))?;
                    let normal = cleanup::normal_edges::state_cleanups(
                        program,
                        facts,
                        &targets,
                        &mut shapes,
                        state,
                    )
                    .map_err(|reason| (state.state, reason))?;
                    Ok((temporary, normal))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Vec<_>>()
        .into_iter();
    drop(targets);
    // A body dropped here keeps the cleanup refusal of the state that owed it.
    // A machine whose other body survives has no gap.
    let mut cleanup_gaps = BTreeMap::new();
    composed_machines.retain_mut(|plan| match composed_selections.next() {
        Some(Ok(selected)) => {
            for (state, (selected, normal)) in plan.states.iter_mut().zip(selected) {
                if cleanup::temporary_actions::install(&mut state.operations, selected).is_none() {
                    return false;
                }
                state.normal_exit_cleanups = normal;
            }
            true
        }
        Some(Err((state, construct))) => {
            cleanup_gaps
                .entry(symbol_key(plan.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine,
                        GapSite::State(state),
                        CheckedUnitPlanGapReason::Unsupported(construct),
                    )
                });
            false
        }
        None => false,
    });
    let install_sequence_cleanup = |plan: &mut CheckedUnitEffectMachinePlan, (selected, normal)| {
        if cleanup::temporary_actions::install(&mut plan.operations, selected).is_none() {
            return false;
        }
        let Some(CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_discards,
            ..
        }) = plan.operations.last_mut()
        else {
            return false;
        };
        plan.normal_exit_cleanups = normal;
        // Parameter dispositions now have one authority and one schedule.
        // The former affine-only reconstruction must not dispose them again.
        trivial_affine_discards.clear();
        true
    };
    partial_affine_machines.retain_mut(|plan| match partial_affine_selections.next() {
        Some(Some(selection)) => install_sequence_cleanup(&mut plan.machine, selection),
        _ => {
            cleanup_gaps
                .entry(symbol_key(plan.machine.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine.machine,
                        GapSite::State(plan.machine.state),
                        CheckedUnitPlanGapReason::Unsupported("sequence cleanup actions"),
                    )
                });
            false
        }
    });
    candidates.retain_mut(|plan| match temporary_selections.next() {
        Some(Some(selection)) => install_sequence_cleanup(plan, selection),
        _ => {
            cleanup_gaps
                .entry(symbol_key(plan.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine,
                        GapSite::State(plan.state),
                        CheckedUnitPlanGapReason::Unsupported("sequence cleanup actions"),
                    )
                });
            false
        }
    });
    let planned = planned_machines(&candidates, &composed_machines, &partial_affine_machines);
    gaps.extend(
        cleanup_gaps
            .into_iter()
            .filter(|(key, _)| !planned.contains(key))
            .map(|(_, gap)| gap),
    );

    drop(_s_cleanup);
    let closure_gaps = phase_profile::scope("05.d.8_10.a.3.4.candidate_closure", || {
        candidate_closure::retain_available(
            program,
            facts,
            &boundary_symbols,
            &shapes.cleanup_control,
            &mut candidates,
            &mut composed_machines,
            &mut partial_affine_machines,
            &prepared_calls,
        )
    });
    gaps.extend(closure_gaps);
    // Failed cleanup or dependency closure discards the candidate's schedules
    // too. They cannot supply ownership evidence to another surviving body.
    prepared_calls.retain_bodies(|owner, state_symbol| {
        candidates
            .iter()
            .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
            .any(|plan| plan.machine == owner && plan.state == state_symbol)
            || composed_machines.iter().any(|plan| {
                plan.machine == owner && plan.states.iter().any(|state| state.state == state_symbol)
            })
    });
    gaps.sort_by_key(|gap| symbol_key(gap.machine));
    // Closure admission is final: do not delete a callee because it also has
    // a specialized return plan. Ordinary and composed callers consume the
    // same exact executable entry, including a body containing only Complete.
    // Competing executable bodies were resolved before closure above; the
    // separate return catalog is not another entry in this roster.
    // Retained values own the declarations of every evaluated producer, not
    // only their receiving leaf. A projected copy can keep an anonymous array
    // alive for its checked exit even though no local declares that array.
    let computations = &facts.values.scalar_computations;
    let mut structural_roots_by_expr: symbols::SymbolKeyMap<
        typed_trees::expression::ExpressionHandle,
        Vec<checked_trees::CheckedStructuralValueHandle>,
    > = symbols::SymbolKeyMap::default();
    for (_, root) in facts.values.structural_values.roots.iter() {
        structural_roots_by_expr
            .entry(root.expression)
            .or_default()
            .push(root.root);
    }
    let scalar_value_roots = computations
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => computations
                .structural_arguments
                .span(*structural_arguments),
            _ => None,
        })
        .flatten()
        .filter_map(|argument| match argument.as_place()?.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                Some(expression)
            }
            _ => None,
        })
        .flat_map(|expression| {
            structural_roots_by_expr
                .get(&expression)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .copied()
        });
    // Broad scalar discovery retains descriptions only. Successful body
    // schedules below supply executable ownership, including call-free roots.
    let catalogue_value_roots = scalar_value_roots
        .map(|value| {
            (
                facts.values.structural_values.nodes.get(value).machine,
                value,
            )
        })
        .collect();
    let catalogue_value_types =
        retain_value_types(program, facts, &mut shapes, catalogue_value_roots);
    let value_roots = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .flat_map(|plan| {
            plan.operations
                .iter()
                .map(move |operation| (plan.machine, operation))
        })
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.states.iter().flat_map(move |state| {
                state
                    .operation_dependencies()
                    .map(move |operation| (plan.machine, operation))
            })
        }))
        .flat_map(|(owner, operation)| {
            operation
                .with_value_calls()
                .map(move |operation| (owner, operation))
        })
        .filter_map(|(owner, operation)| match operation {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) => Some((owner, *value)),
            _ => None,
        })
        .chain(prepared_calls.value_roots())
        .collect::<Vec<_>>();
    let mut stored_type_roots = retain_value_types(program, facts, &mut shapes, value_roots);
    stored_type_roots.extend(
        candidates
            .iter()
            .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
            .flat_map(|plan| {
                plan.stored_type_roots()
                    .map(move |identity| (plan.machine, identity))
            })
            .chain(composed_machines.iter().flat_map(|plan| {
                plan.stored_type_roots()
                    .map(move |identity| (plan.machine, identity))
            }))
            .chain(
                prepared_calls
                    .operation_dependencies()
                    .flat_map(|(owner, operation)| {
                        operation.with_value_calls().flat_map(move |operation| {
                            operation
                                .stored_type_roots()
                                .map(move |identity| (owner, identity))
                        })
                    }),
            ),
    );
    stored_type_roots.sort_by_key(|(owner, identity)| (symbol_key(*owner), *identity));
    stored_type_roots.dedup();
    // Catalogue descriptions and cleanup vocabulary survive independently of
    // actual storage. Consumers receive the producer's body/root association;
    // they must not infer ownership from membership in this broader catalogue.
    let mut retained_type_identities = stored_type_roots
        .iter()
        .map(|(_, identity)| *identity)
        .chain(boundary_machines.iter().flat_map(|plan| {
            plan.attachment_type_identity
                .iter()
                .copied()
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity),
                )
                .chain(plan.result.structural_identity())
        }))
        .chain(dynamic_slots.iter().flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity)
                .chain(plan.result.structural_identity())
        }))
        .chain(
            candidates
                .iter()
                .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
                .flat_map(|plan| {
                    plan.attachment_type_identity.iter().copied().chain(
                        shapes
                            .cleanup_control
                            .normal_actions(plan.normal_exit_cleanups)
                            .map(|action| action.type_identity()),
                    )
                }),
        )
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.attachment_type_identity.iter().copied().chain(
                plan.states
                    .iter()
                    .flat_map(|state| {
                        shapes
                            .cleanup_control
                            .normal_actions(state.normal_exit_cleanups)
                    })
                    .map(|action| action.type_identity()),
            )
        }))
        .collect::<BTreeSet<_>>();
    retained_type_identities.extend(
        catalogue_value_types
            .into_iter()
            .map(|(_, identity)| identity),
    );
    // A local loan's referee may have no signature or operation root.
    // Publish its complete storage type, including scalar restrictions, so
    // observations rejoin the same exact identity requested by lowering.
    let loaned_storage_types = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .map(|plan| plan.machine)
        .chain(composed_machines.iter().map(|plan| plan.machine))
        .collect::<Vec<_>>();
    let mut retained_loan_identities = Vec::new();
    for machine in loaned_storage_types {
        let Some(source_machine) = program.machine(machine) else {
            continue;
        };
        for state in program.machine_states(source_machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let TypeReferenceNode::Reference { referee, .. } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    continue;
                };
                if let Some(identity) = shapes.add_type(*referee, &[], &[]) {
                    retained_loan_identities.push(identity);
                }
            }
        }
    }
    for identity in &retained_loan_identities {
        retained_type_identities.insert(*identity);
    }
    // Computed structural operands declare their descriptor even when no source
    // binding has that type. This retains storage vocabulary, not evaluation or
    // custody: the owning scalar computation still establishes it selectively.
    let operand_identities = facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let operand_type = match node.kind {
                checked_trees::CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                    operand_type,
                    ..
                }
                | checked_trees::CheckedScalarComputationKind::ByteSequencePredicate {
                    operand_type,
                    ..
                } => operand_type,
                _ => return None,
            };
            // The observation may consume a stored reference rather than its
            // peeled backing. Retain both actual operation dependencies.
            let backing = shapes.add_type(operand_type, &[], &[])?;
            let carrier = shapes.add_stored_type(operand_type, &[], &[])?;
            Some([backing, carrier])
        })
        .flatten()
        .collect::<Vec<_>>();
    retained_type_identities.extend(operand_identities.iter().copied());
    // A performed field observation demands only the concrete declarations
    // along its finite storage path, including nested descriptor crossings.
    for (_, node) in facts.values.scalar_computations.nodes.iter() {
        if let checked_trees::CheckedScalarComputationKind::StructuralField {
            source_type,
            subject,
            field,
            field_identity,
            atomic_ordering,
            ..
        } = &node.kind
        {
            match validation::observed_storage_type_references(
                program,
                *source_type,
                &subject.path,
                *field,
                field_identity,
                atomic_ordering.is_some(),
            ) {
                Ok(references) => {
                    for (reference, substitutions) in references.entries() {
                        if let Some(identity) = shapes.add_type(reference, &[], substitutions) {
                            retained_type_identities.insert(identity);
                        }
                        if let Some(identity) =
                            shapes.add_stored_type(reference, &[], substitutions)
                        {
                            retained_type_identities.insert(identity);
                        }
                    }
                }
                // This global discovery roster includes undemanded and
                // unapplied bodies. A missing finite catalog path is producer
                // coverage, not a new Source formation rule. Demanded lowering
                // runs this same query and reports its complete path error.
                Err(_) => {}
            }
        }
    }
    // Every collection capture names its actual declaration carrier before
    // evaluating that collection's selector. Retain its vocabulary even when
    // an inline descriptor field otherwise has no standalone catalog row.
    for (_, node) in facts.values.scalar_computations.nodes.iter() {
        if let checked_trees::CheckedScalarComputationKind::StorageObservation {
            collections, ..
        } = node.kind
        {
            for reference in facts
                .values
                .scalar_computations
                .observation_collections
                .span(collections)
                .into_iter()
                .flatten()
            {
                if let Some(identity) = shapes.add_type(*reference, &[], &[]) {
                    retained_type_identities.insert(identity);
                }
                if let Some(identity) = shapes.add_stored_type(*reference, &[], &[]) {
                    retained_type_identities.insert(identity);
                }
            }
        }
    }

    // A checked formation membership is a catalog dependency wherever the
    // value lands: lowering selects each node's `qualification_decisions` and
    // needs the carrier theory even when no signature, local declaration or
    // boundary requirement spelled that domain. Register every decided
    // domain against the value's own checked carrier, as the scalar-array and
    // completion binders already do for their own initializers, and retain
    // the carrier beside the other produced types.
    let mut formation_binders = BTreeMap::new();
    for (_, node) in facts.values.structural_values.nodes.iter() {
        if !node.type_reference.is_valid() {
            continue;
        }
        let Some(decisions) = facts
            .qualifications
            .value_formations
            .span(node.qualification_decisions)
        else {
            continue;
        };
        if decisions.is_empty() {
            continue;
        }
        let binders = formation_binders
            .entry(symbol_key(node.machine))
            .or_insert_with(|| {
                program
                    .machine_by_symbol(node.machine)
                    .map(|machine| types::machine_binders(program, machine))
                    .unwrap_or_default()
            });
        for decision in decisions {
            if let Some(identity) =
                shapes.add_domain(decision.semantic_domain, node.type_reference, binders)
            {
                retained_type_identities.insert(identity);
            }
        }
    }
    let stored_operand_identities = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .flat_map(|plan| {
            plan.operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .map(move |operation| (plan.machine, plan.state, operation))
        })
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.states.iter().flat_map(move |state| {
                state
                    .operation_dependencies()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .map(move |operation| (plan.machine, state.state, operation))
            })
        }))
        .flat_map(|(machine, state, operation)| {
            byte_operands::store_operand_type_identities(
                program,
                &mut shapes,
                machine,
                state,
                operation,
            )
        })
        .collect::<Vec<_>>();
    retained_type_identities.extend(stored_operand_identities.iter().copied());
    // A successor's retained address owns a descriptor independently of its
    // borrowed referent parameter. Keep that declaration through the same
    // transitive type roster as ordinary reference formations.
    retained_type_identities.extend(
        composed_machines
            .iter()
            .flat_map(|machine| &machine.states)
            .flat_map(|state| state.successors())
            .flat_map(|edge| &edge.transfers)
            .filter_map(|transfer| match &transfer.source {
                checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                    carrier_type_identity,
                    ..
                } if carrier_type_identity.is_valid() => Some(*carrier_type_identity),
                _ => None,
            }),
    );
    // A partial-affine return edge names the residual complement's shapes
    // beside the body's own call results.
    retained_type_identities.extend(partial_affine_machines.iter().flat_map(|plan| {
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.type_identity)
            .chain(plan.machine.operations.iter().filter_map(|operation| {
                let result = operation.call()?.structural_result()?;
                Some(result.type_identity)
            }))
    }));
    // Runtime and erased proof observations share the checked carrier owner.
    // Publish their exact subjects before pruning the transitive type catalog.
    facts.layout_query_subjects = layout_queries::retain(
        program,
        facts,
        &mut shapes,
        &mut retained_type_identities,
        &mut gaps,
    );
    shapes.retain_transitive(&retained_type_identities);
    types::retain_domain_predicates(program, &facts.operators, &mut shapes.domains);

    let partial_affine_plans = CheckedPartialAffineUnitCleanupPlans {
        machines: partial_affine_machines,
    };
    (
        CheckedUnitEffectPlans {
            cleanup_control: shapes.cleanup_control,
            stored_type_roots,
            structural_types: shapes.types.into_values().collect(),
            structural_domains: {
                shapes.domains.sort_by_key(|domain| domain.domain.0);
                shapes.domains
            },
            boundary_machines,
            dynamic_slots,
            machines: candidates,
            composed_machines,
            gaps,
        },
        partial_affine_plans,
        prepared_calls,
    )
}

pub(super) fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

fn planned_machines(
    candidates: &[CheckedUnitEffectMachinePlan],
    composed_machines: &[CheckedComposedUnitControlMachinePlan],
    partial_affine_machines: &[CheckedPartialAffineUnitCleanupMachinePlan],
) -> BTreeSet<(u32, u32)> {
    candidates
        .iter()
        .map(|plan| symbol_key(plan.machine))
        .chain(
            composed_machines
                .iter()
                .map(|plan| symbol_key(plan.machine)),
        )
        .chain(
            partial_affine_machines
                .iter()
                .map(|plan| symbol_key(plan.machine.machine)),
        )
        .collect()
}

/// The one plan route of a checked body, chosen by its authored shape before
/// any builder runs. A builder that cannot plan its route's body reports the
/// gap; the other route is never tried, so no body has two descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyRoute {
    /// One state whose result the graph cannot describe, or one without
    /// transitions, state contracts or nominal-cleanup owners whose result is
    /// Unit or a scalar: a statement sequence that completes or returns at its
    /// end.
    Sequence,
    /// Everything else: the general graph of states, transitions, normal-exit
    /// cleanup and successor transfers.
    Graph,
}

fn body_route(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> BodyRoute {
    // A bodied boundary adapter is planned as a sequence: the graph builder
    // admits only ordinary checked bodies.
    if machine.supply_mode != MachineSupplyMode::CheckedBody {
        return BodyRoute::Sequence;
    }
    let [state] = program.machine_states(machine) else {
        return BodyRoute::Graph;
    };
    // A result the graph's vocabulary cannot describe is only a sequence's.
    // The classification uses a scratch shape namespace: choosing a route
    // registers no type.
    if state_graph::result_plan(program, &mut ShapeCollector::new(program), machine).is_none() {
        return BodyRoute::Sequence;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // Transitions are control: loops, guarded arms and returned values out
    // of arms. A state's own contracts, and a local or by-value parameter
    // whose owner may run a nominal `::drop` at return, need the graph's
    // state signature and normal-exit cleanup; the sequence has neither. A
    // linear owner is consumed explicitly by construction, never dropped at
    // return.
    let owes_nominal_drop = |type_reference| {
        !types::is_reference(program, type_reference)
            && crate::checks::type_multiplicity(program, type_reference) != Multiplicity::Linear
            && types::type_graph_requires_nominal_drop(
                program,
                type_reference,
                &[],
                &mut std::collections::BTreeSet::new(),
            )
    };
    if statements.iter().any(|statement| match statement {
        StatementNode::Transition(_) => true,
        StatementNode::LocalData(local) => owes_nominal_drop(local.type_reference),
        _ => false,
    }) || program
        .state_parameters(state)
        .iter()
        .any(|parameter| owes_nominal_drop(parameter.type_reference))
        || !program.state_contracts(state).is_empty()
    {
        return BodyRoute::Graph;
    }
    if !state.return_type.is_valid()
        || types::is_unit(program, state.return_type)
        || program
            .primitive_type_reference(state.return_type)
            .is_some()
    {
        BodyRoute::Sequence
    } else {
        BodyRoute::Graph
    }
}

/// Retain direct nominal caller prerequisites for admitted declarations.
///
/// The executable body and its source-selected normal-exit transactions have
/// already been published. This metadata catalogue supplies signature
/// requirements and entry identity; it cannot replace or reconstruct that body.
pub(crate) fn build_checked_nominal_affine_unit_cleanup_plans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    CheckedNominalAffineUnitCleanupPlans,
    Vec<CheckedUnitStructuralTypePlan>,
) {
    let mut shapes = ShapeCollector::new(program);
    let targets = unit_effects
        .machines
        .iter()
        .map(CheckedUnitEffectMachinePlan::callable_entry)
        .chain(
            unit_effects
                .composed_machines
                .iter()
                .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
        )
        .collect::<Vec<_>>();
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            let plan = build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                &targets,
                &mut shapes,
                machine,
                diagnostics,
            )?;
            Some(plan)
        })
        .collect::<Vec<_>>();
    (
        CheckedNominalAffineUnitCleanupPlans { machines },
        shapes.types.into_values().collect(),
    )
}

// Shape and signature admission shared by the plan builders above.

/// A computed structural operand owns its storage vocabulary independently of
/// source parameters. Retain both its canonical backing and its exact stored
/// carrier: inline aggregate fields can materialize a descriptor even when the
/// field's geometry names no standalone type. Access and evaluation remain with
/// the operation plan.
pub(super) fn scalar_operand_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    shapes.add_type(reference, &[], &[])?;
    shapes.add_stored_type(reference, &[], &[])?;
    Some(shapes.types.into_values().collect())
}

/// Scalar and Unit value construction share the exact formation dependency.
/// The source identity names the borrowed leaf; a computed containing value
/// retains its own storage separately through `scalar_operand_shapes`.
pub(super) fn reference_backing_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    source_identity: typed_trees::type_identity::TypeIdentityId,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    shapes.add_reference_backing(reference, source_identity, &[])?;
    Some(shapes.types.into_values().collect())
}

/// Construction and structural results share the same closed sum admission.
/// The namespace includes real value types even when no parameter names them.
pub(super) fn scalar_case_value_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    // A primitive result owns no structural value shape.
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } =
        state_graph::returns::signature(program, &mut shapes, reference)?
    {
        return None;
    }
    Some(shapes.types.into_values().collect())
}

/// Register the declarations of retained value producers and their operands.
/// Projection leaves and their evaluated owners survive catalog pruning
/// independently; retaining a shape grants no construction or cleanup custody.
fn retain_value_types(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut types::ShapeCollector<'_>,
    mut pending: Vec<(SymbolHandle, checked_trees::CheckedStructuralValueHandle)>,
) -> Vec<(SymbolHandle, typed_trees::type_identity::TypeIdentityId)> {
    let plans = &facts.values.structural_values;
    let mut identities = Vec::new();
    let mut visited = Vec::new();
    while let Some((owner, handle)) = pending.pop() {
        if visited.contains(&(owner, handle)) {
            continue;
        }
        visited.push((owner, handle));
        let node = plans.nodes.get(handle);
        let binders = program
            .machine_by_symbol(node.machine)
            .map(|machine| types::machine_binders(program, machine))
            .unwrap_or_default();
        if node.type_reference.is_valid()
            && let Some(identity) = shapes.add_stored_type(node.type_reference, &binders, &[])
        {
            identities.push((owner, identity));
        }
        match &node.kind {
            checked_trees::CheckedStructuralValueKind::Place(source) => {
                identities.push((owner, source.type_identity));
            }
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                identities.push((owner, source.type_identity));
            }
            checked_trees::CheckedStructuralValueKind::Reference {
                source,
                source_value,
                ..
            } => {
                identities.push((owner, source.type_identity));
                if source_value.is_valid() {
                    pending.push((owner, *source_value));
                }
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. }
            | checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. }
            | checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                pending.push((owner, *source));
            }
            checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                pending.extend(arguments.iter().map(|value| (owner, *value)));
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span_or_empty(*fields) {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(nested) =
                        field.value
                    {
                        pending.push((owner, nested));
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().map(|value| (owner, *value)));
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(
                    plans
                        .dispatch_arms
                        .span_or_empty(*arms)
                        .iter()
                        .map(|arm| (owner, arm.value)),
                );
            }
            _ => {}
        }
    }
    identities
}

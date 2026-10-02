//! The declaration predicate roster crosses into Terminal.
//!
//! `Grid` states its cells with a collection invariant. Every machine whose
//! parameters reach a `Grid` publishes each cell's checked consequence,
//! `0 <= cell <= 1`, as an entry requirement, and an exclusive borrow also
//! publishes it as a normal-return guarantee. Terminal verification then
//! re-proves the rows independently: a callee's stores must restore them
//! before it returns, a caller must hold them before the call, and the Exact
//! sum reads its operand bounds from them. The fact-only `cell_is_bit`
//! citations leave no call or machine behind.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IntegerValue;
use terminal_psi::OperationKind;

const PREDICATE: &str = r#"
machine cells_valid(cells: &[i32; 4]) -> bool {
    transition cells[0] == 0 || cells[0] == 1 {
        true -> at_1(cells)
        _ -> (false)
    }
    state at_1(cells: &[i32; 4]) -> bool {
        transition cells[1] == 0 || cells[1] == 1 {
            true -> at_2(cells)
            _ -> (false)
        }
    }
    state at_2(cells: &[i32; 4]) -> bool {
        transition cells[2] == 0 || cells[2] == 1 {
            true -> at_3(cells)
            _ -> (false)
        }
    }
    state at_3(cells: &[i32; 4]) -> bool {
        transition cells[3] == 0 || cells[3] == 1 {
            true -> (true)
            _ -> (false)
        }
    }
}

machine cell_is_bit(cells: &[i32; 4], index: u64)
requires cells_valid(cells) == true
requires index < 4
ensures cells[index] >= 0 && cells[index] <= 1
{
}

data Grid [copy] where cells_valid(&board) == true, cells_valid(&next) == true {
    board: [i32; 4];
    next: [i32; 4];
    offset: i32 in Wrapping;
}

data Main {
    g: Grid;
    total: i32 in Wrapping;
}

machine Main::seed(context: &mut Main) {
    context.g.board[1] = 1;
    context.g.board[2] = 1;
    context.g.offset = 68;
}

// One generation of cell 0: cite its neighbours, then store through states.
machine Main::advance(context: &mut Main) {
    cell_is_bit(&context.g.board, 1);
    cell_is_bit(&context.g.board, 2);
    let neighbours: i32 in Wrapping = (context.g.board[1] + context.g.board[2]) as i32 in Wrapping;
    transition neighbours == 2 {
        true -> live(context)
        _ -> die(context)
    }
    state live(context: &mut Main) { context.g.next[0] = 1; }
    state die(context: &mut Main) { context.g.next[0] = 0; }
}

machine Main::sum(context: &mut Main) -> i32 {
    cell_is_bit(&context.g.next, 0);
    cell_is_bit(&context.g.next, 1);
    cell_is_bit(&context.g.next, 2);
    cell_is_bit(&context.g.next, 3);
    transition {
        _ -> (context.g.next[0] + context.g.next[1] + context.g.next[2] + context.g.next[3])
    }
}
"#;

fn source(main_body: &str) -> String {
    format!(
        "{PREDICATE}
         machine Main::main(context: &mut Main) {{
             {main_body}
             Main::seed(context);
             Main::advance(context);
             let cells: i32 in Wrapping = Main::sum(context) as i32 in Wrapping;
             context.total = cells + context.g.offset;
         }}"
    )
}

fn lowered(main_body: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(&source(main_body));
    lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a collection invariant lowers through its declaration predicate rows")
}

fn verify(module: &terminal_psi::TerminalModule, lowered: &lowered_psi::LoweredPsi) -> bool {
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .is_ok()
}

fn has_operation(machine: &terminal_psi::TerminalMachine, kind: &OperationKind) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| operation.kind == *kind)
}

fn integer(value: i128) -> OperationKind {
    OperationKind::IntegerConstant {
        value: IntegerValue::Signed(value),
    }
}

/// Replace every integer constant `from` with `to` in the machines `select`
/// admits, returning how many operations changed.
fn substitute_constant(
    module: &mut terminal_psi::TerminalModule,
    select: impl Fn(&terminal_psi::TerminalMachine) -> bool,
    from: i128,
    to: i128,
) -> usize {
    let mut changed = 0;
    for machine in module.machines.iter_mut().filter(|machine| select(machine)) {
        for operation in machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if operation.kind == integer(from) {
                operation.kind = integer(to);
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn roster_rows_carry_the_collection_invariant_through_calls_stores_and_sums() {
    let lowered = lowered("");
    assert!(verify(&lowered.semantic_module, &lowered));
    // Main, seed, advance and sum; the citations leave neither a call nor a
    // machine.
    assert_eq!(lowered.semantic_module.machines.len(), 4);
    for machine in &lowered.semantic_module.machines {
        // Each machine reaches `Grid` through its exclusive `context`: each
        // cell's two bounds are separate entry requirements, and the normal
        // return restores all eight cell intervals in one guarantee.
        assert_eq!(machine.contract.requires.len(), 16, "{:?}", machine.id);
        assert_eq!(machine.contract.ensures.len(), 1, "{:?}", machine.id);
    }
}

#[test]
fn a_callee_store_that_breaks_the_invariant_cannot_restore_it_at_return() {
    let lowered = lowered("");
    let mut broken = lowered.semantic_module.clone();
    // `seed` alone stores the offset 68; its two cell stores write 1.
    let seed = |machine: &terminal_psi::TerminalMachine| has_operation(machine, &integer(68));
    assert_eq!(substitute_constant(&mut broken, seed, 1, 7), 2);
    assert!(
        !verify(&broken, &lowered),
        "a cell of 7 cannot establish the returned `0 <= cell <= 1` row"
    );
}

#[test]
fn a_caller_store_that_breaks_the_invariant_cannot_discharge_the_call() {
    let lowered = lowered("context.g.board[3] = 0;");
    assert!(verify(&lowered.semantic_module, &lowered));
    let mut broken = lowered.semantic_module.clone();
    // `main` alone calls; its one store writes 0 before calling `seed`.
    let main = |machine: &terminal_psi::TerminalMachine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
    };
    assert_eq!(substitute_constant(&mut broken, main, 0, 5), 1);
    assert!(
        !verify(&broken, &lowered),
        "a cell of 5 cannot discharge the callee's `0 <= cell <= 1` requirement"
    );
}

/// An IEEE range fact rides the same roster with its exact format: each
/// machine reaching `Gauge` requires the range at entry and restores it at
/// return. `count` never writes the level, so its guarantee is the entry row
/// itself, and `main`'s second call is discharged by the first call's
/// imported guarantee.
const GAUGE: &str = "
    data Gauge where level >= 0.0 && level <= 1.0 { level: f32; }
    data Main { gauge: Gauge; count: u8 in Wrapping; }
    machine Main::count(context: &mut Main) {
        context.count = context.count + 1;
    }
    machine Main::main(context: &mut Main) {
        Main::count(context);
        Main::count(context);
    }
";

#[test]
fn an_ieee_range_row_keeps_its_format_through_calls() {
    let checked = crate::front_end::checked_program(GAUGE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("an IEEE range fact lowers through its declaration predicate row");
    assert!(verify(&lowered.semantic_module, &lowered));
    for machine in &lowered.semantic_module.machines {
        assert!(
            machine.contract.requires.iter().all(|requirement| matches!(
                requirement,
                semantic_vocabulary::Proposition::ScalarIeeeFloatComparison {
                    format: semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ..
                }
            )),
            "{:?}",
            machine.contract.requires
        );
        assert_eq!(machine.contract.requires.len(), 2);
        assert_eq!(machine.contract.ensures.len(), 1);
    }
}

/// A callee restores an IEEE range row from its last store: the transient
/// 2.0 is an invariant window the 0.5 store closes, and Terminal proves the
/// returned row by following the level to the stored bits.
const GAUGE_STORES: &str = "
    data Gauge where level >= 0.0 && level <= 1.0 { level: f32; }
    data Main { gauge: Gauge; }
    machine Main::fill(context: &mut Main) {
        context.gauge.level = 2.0;
        context.gauge.level = 0.5;
    }
    machine Main::main(context: &mut Main) { Main::fill(context); }
";

fn ieee(value: f32) -> OperationKind {
    OperationKind::IeeeFloatConstant {
        value: semantic_vocabulary::IeeeFloatValue::Binary32(value.to_bits()),
    }
}

#[test]
fn an_ieee_range_row_is_restored_from_the_final_store() {
    let checked = crate::front_end::checked_program(GAUGE_STORES);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a stored IEEE range row is proved from the stored bits");
    assert!(verify(&lowered.semantic_module, &lowered));
    let mut broken = lowered.semantic_module.clone();
    let mut changed = 0;
    for operation in broken
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if operation.kind == ieee(0.5) {
            operation.kind = ieee(2.0);
            changed += 1;
        }
    }
    assert_eq!(changed, 1, "only the restoring store writes 0.5");
    assert!(
        !verify(&broken, &lowered),
        "a final 2.0 cannot establish the returned `level <= 1.0` row"
    );
}

/// A loop proves declaration rows at authored state arrivals and normal return.
#[test]
fn a_looping_callee_and_its_caller_prove_declared_restoration() {
    let checked = crate::front_end::checked_program(&format!(
        "{PREDICATE}
         machine Main::scan(context: &mut Main) {{
             transition {{ _ -> step(context, 0) }}
             state step(context: &mut Main, round: u8) {{
                 transition round < 3 {{
                     true -> again(context, round)
                     _ -> done(context)
                 }}
             }}
             state again(context: &mut Main, round: u8) {{
                 context.total = context.g.offset;
                 transition {{ _ -> step(context, round + 1) }}
             }}
             state done(context: &mut Main) {{ }}
         }}
         machine Main::main(context: &mut Main) {{ Main::scan(context); }}"
    ));
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a looping callee proves its retained declaration obligations");
    assert!(verify(&lowered.semantic_module, &lowered));
    let rows = lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| {
            (
                machine.contract.requires.len(),
                machine.contract.ensures.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows, vec![(16, 1), (16, 1)]);
}

const GAUGE_LOOP: &str = "
    data Main where value >= 0.0 && value <= 1.0 { value: f32; }
    machine Main::fill(context: &mut Main) {
        transition { _ -> step(context, 0) }
        state step(context: &mut Main, round: u8) {
            transition round < 3 { true -> again(context, round) _ -> done(context) }
        }
        state again(context: &mut Main, round: u8) {
            context.value = 2.0;
            context.value = 0.25;
            transition { _ -> step(context, round + 1) }
        }
        state done(context: &mut Main) {}
    }
    machine Main::observe(context: &mut Main) {}
    machine Main::main(context: &mut Main) {
        Main::fill(context);
        Main::observe(context);
    }
";

#[test]
fn a_loop_restores_float_rows_for_the_next_call_and_source_free_replay() {
    let checked = crate::front_end::checked_program(GAUGE_LOOP);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("loop arrivals and normal return restore the downstream observer's field rows");
    assert!(verify(&lowered.semantic_module, &lowered));
    assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic).unwrap(),
        &terminal_codec::decode_proof_bundle(&evidence).unwrap(),
        &AdmissionProfile::default(),
    )
    .expect("loop restoration survives independent source-free reload");
    let mut broken = lowered.semantic_module.clone();
    let mut changed = 0;
    for operation in broken
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if operation.kind == ieee(0.25) {
            operation.kind = ieee(2.0);
            changed += 1;
        }
    }
    assert_eq!(changed, 1);
    assert!(
        !verify(&broken, &lowered),
        "the final store must restore the field before the backedge"
    );
}

#[test]
fn authored_state_declaration_rows_rejoin_the_exact_checked_source() {
    for remove in [false, true] {
        let mut checked = crate::front_end::checked_program(GAUGE_LOOP);
        let state = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .find(|state| !state.requires.is_empty())
            .expect("retained float declaration rows");
        if remove {
            state.requires.pop();
        } else {
            state.requires[0] = Some(checked_trees::ClosedScalarContractValue::Predicate(
                checked_trees::CheckedBooleanExpression::Constant(true),
            ));
        }
        let error = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
            .expect_err(
                "neither a missing row nor a same-typed replacement may weaken a state contract",
            );
        assert!(
            format!("{error:?}").contains("state declaration predicates differ from source"),
            "{error:?}"
        );
    }
}

#[test]
fn an_open_float_invariant_window_cannot_cross_an_authored_state_transition() {
    let source = GAUGE_LOOP.replace(
        "context.value = 0.25;",
        "transition { _ -> restore(context, round) } } state restore(context: &mut Main, round: u8) { context.value = 0.25;",
    );
    let errors = crate::front_end::checked_program_result(&source)
        .expect_err("restoring in the next state is too late for the authored transition");
    assert!(
        format!("{errors:?}").contains("OPEN invariant window"),
        "{errors:?}"
    );
}

#[test]
fn final_state_store_invalidates_earlier_names_before_return_restoration() {
    let checked = crate::front_end::checked_program(
        "        data Main where value >= 0.0 && value <= 1.0 { value: f32; }
        machine Main::main(context: &mut Main) {
            transition { _ -> finish(context) }
            state finish(context: &mut Main) { context.value = 0.25; }
        }",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the final state restores the original invocation storage");
    assert!(verify(&lowered.semantic_module, &lowered));
    let mut broken = lowered.semantic_module.clone();
    let mut changed = 0;
    for operation in broken
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if operation.kind == ieee(0.25) {
            operation.kind = ieee(2.0);
            changed += 1;
        }
    }
    assert_eq!(changed, 1);
    assert!(
        !verify(&broken, &lowered),
        "no later state arrival protects this return: old entry-name facts must expire on the aliased write"
    );
    let mut reproduced = lowered;
    reproduced.semantic_module = broken;
    reproduced.proof_bundle.evidence.clear();
    assert!(matches!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut reproduced),
        Err(crate::LoweringError::OperationProofUnavailable(_))),
        "fresh proof production must also reject stale invocation-name facts");
}

#[test]
fn a_local_alternative_cannot_publish_the_invocation_parameters_field_fact() {
    const SOURCE: &str = "
        data Main { value: f32; }
        machine Main::main(context: &mut Main, choose: bool) {
            context.value = 0.0;
            let mut other: Main;
            transition choose {
                true -> finish(context)
                _ -> finish(&mut other)
            }
            state finish(selected: &mut Main) { selected.value = 0.25; }
        }
    ";
    for local_alternative in [false, true] {
        let source = if local_alternative { SOURCE.to_owned() }
            else { SOURCE.replace("finish(&mut other)", "finish(context)") };
        let checked = crate::front_end::checked_program(&source);
        let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
            .expect("each selected loan writes its own backing");
        let next = terminal_verifier::maximum_registered_obligation_id(&lowered.semantic_module)
            .unwrap().checked_add(1).unwrap();
        let machine = &mut lowered.semantic_module.machines[0];
        let root = machine.structural_parameters[0].place;
        let field = machine.blocks.iter().flat_map(|block| &block.operations).find_map(|operation| {
            if let OperationKind::StructuralScalarFieldStore { field, .. } = operation.kind { Some(field) } else { None }
        }).unwrap();
        machine.contract.ensures.push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(next).unwrap(),
            proposition: semantic_vocabulary::Proposition::Equal(
                semantic_vocabulary::ScalarTerm::ieee_float_field_path(root,
                    vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(field)],
                    semantic_vocabulary::IeeeFloatFormat::Binary32),
                semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(semantic_vocabulary::IeeeFloatValue::Binary32(0.25_f32.to_bits())),
            ),
        });
        lowered.proof_bundle.evidence.clear();
        let result = crate::proofs::operation_proofs::finalize_operation_proofs(&mut lowered);
        if local_alternative {
            assert!(matches!(result, Err(crate::LoweringError::OperationProofUnavailable(_))),
                "a local backing is not silently discarded when deciding origin identity: {result:?}");
        } else {
            result.expect("all incoming origins select the invocation parameter");
            assert!(verify(&lowered.semantic_module, &lowered));
        }
    }
}

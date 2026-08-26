use omega_control_flow::StateKey;
use omega_runtime_abi::RuntimeAbiPlan;
use omega_state_calls::StateCallPlan;
use omega_target_operations::{
    DynamicConformanceTable, DynamicConformanceTableRow, TargetDataObject, TargetDataObjectKind,
    TargetDataPlan,
};
use psi_checked_trees::{CheckedTrees, DynamicConformanceRowFact, DynamicConformanceRowSource};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn collect_dynamic_conformance_tables(
    program: &CheckedTrees,
    state_calls: &StateCallPlan,
    runtime_abi: RuntimeAbiPlan,
    data: &mut TargetDataPlan,
) -> Result<(), Diagnostic> {
    for (_, call) in state_calls.calls.iter() {
        let arguments = state_calls.arguments.span(call.arguments).ok_or_else(|| {
            Diagnostic::error("dynamic table planning found an invalid state-call argument span")
        })?;
        for descriptor in arguments
            .iter()
            .filter_map(|argument| argument.dynamic_conformance.as_ref())
        {
            retain_dynamic_table(
                program,
                call.source_key,
                call.statement_index,
                descriptor.target_trait,
                descriptor.conformance,
                &descriptor.rows,
                runtime_abi,
                data,
            )?;
        }
        if let Some(dispatch) = &call.dynamic_dispatch {
            for candidate in &dispatch.candidates {
                retain_dynamic_table(
                    program,
                    call.source_key,
                    call.statement_index,
                    dispatch.target_trait,
                    candidate.conformance,
                    &candidate.rows,
                    runtime_abi,
                    data,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn retain_dynamic_table(
    program: &CheckedTrees,
    source_key: StateKey,
    source_statement: usize,
    target_trait: psi_symbols::SymbolHandle,
    conformance: psi_symbols::SymbolHandle,
    retained_rows: &[DynamicConformanceRowFact],
    runtime_abi: RuntimeAbiPlan,
    data: &mut TargetDataPlan,
) -> Result<(), Diagnostic> {
    let (trait_identity, conformance_identity, rows) =
        validated_table_identity(program, target_trait, conformance, retained_rows)?;
    if rows.is_empty()
        || data.dynamic_conformance_tables.iter().any(|(_, table)| {
            table.trait_identity.as_ref() == trait_identity
                && table.conformance_identity.as_ref() == conformance_identity
                && table.rows == rows
        })
    {
        return Ok(());
    }
    let symbol = dynamic_table_symbol(&trait_identity, &conformance_identity, &rows);
    if data
        .objects
        .iter()
        .any(|(_, object)| object.symbol.as_ref() == symbol)
    {
        return Err(Diagnostic::error(
            "dynamic conformance table private-symbol identity collided",
        ));
    }
    let alignment = runtime_abi.pointer_alignment.max(1);
    let padding = (alignment - data.bytes.len() % alignment) % alignment;
    data.bytes.insert_many(std::iter::repeat_n(0, padding));
    let offset = data.bytes.len();
    let byte_count = rows
        .len()
        .checked_mul(runtime_abi.pointer_size)
        .ok_or_else(|| Diagnostic::error("dynamic conformance table size overflow"))?;
    let bytes = data.bytes.insert_many(std::iter::repeat_n(0, byte_count));
    let object = data.objects.insert(TargetDataObject {
        symbol: Arc::from(symbol),
        kind: TargetDataObjectKind::DynamicConformanceTable,
        offset,
        bytes,
        alignment,
        source_key,
        source_statement,
    });
    data.dynamic_conformance_tables
        .insert(DynamicConformanceTable {
            object,
            target_trait,
            conformance,
            trait_identity: Arc::from(trait_identity),
            conformance_identity: Arc::from(conformance_identity),
            rows,
        });
    Ok(())
}

fn validated_table_identity(
    program: &CheckedTrees,
    target_trait: psi_symbols::SymbolHandle,
    conformance_symbol: psi_symbols::SymbolHandle,
    retained_rows: &[DynamicConformanceRowFact],
) -> Result<(String, String, Vec<DynamicConformanceTableRow>), Diagnostic> {
    let trait_definition = exact_trait(program, target_trait)?;
    let conformance = exact_conformance(program, conformance_symbol)?;
    if trait_definition.is_boundary
        || conformance.trait_symbol != target_trait
        || conformance.trait_name != trait_definition.name
    {
        return Err(Diagnostic::error(
            "dynamic table conformance no longer belongs to its retained trait",
        ));
    }
    let declared_rows = program
        .closed_conformance_rows(conformance)
        .ok_or_else(|| {
            Diagnostic::error("dynamic table planning received a bodyless conformance")
        })?;
    if declared_rows.len() != retained_rows.len() {
        return Err(Diagnostic::error(
            "dynamic table row map is partial or expanded",
        ));
    }

    let mut rows = Vec::new();
    for (declared, retained) in declared_rows.iter().zip(retained_rows) {
        let expected_source = match declared.source {
            psi_checked_trees::trait_definition::ConformanceRowSource::Inline => {
                DynamicConformanceRowSource::Inline
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::Reference => {
                DynamicConformanceRowSource::Reference
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                DynamicConformanceRowSource::TraitDefault
            }
        };
        if declared.declaring_trait != retained.declaring_trait
            || declared.requirement != retained.requirement
            || declared.realization_machine != retained.realization_machine
            || declared.realization_state != retained.realization_state
            || expected_source != retained.source
        {
            return Err(Diagnostic::error(
                "dynamic table row order or exact identity drifted",
            ));
        }
        let declaring_trait = exact_trait(program, retained.declaring_trait)?;
        let requirement = exact_requirement(program, declaring_trait, retained.requirement)?;
        let requirement_identity = program
            .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
            .identity();
        if requirement_identity != retained.requirement_identity {
            return Err(Diagnostic::error(
                "dynamic table normalized requirement identity drifted",
            ));
        }
        let realization = exact_machine(program, retained.realization_machine)?;
        let realization_identity = program
            .normalized_machine_overload_identity(realization)
            .ok_or_else(|| {
                Diagnostic::error("dynamic table realization has no normalized callable identity")
            })?
            .identity();
        if realization_identity != retained.realization_identity
            || !program.machine_states(realization).iter().any(|state| {
                state.symbol == retained.realization_state
                    && state.symbol == declared.realization_state
            })
        {
            return Err(Diagnostic::error(
                "dynamic table normalized realization identity drifted",
            ));
        }
        if program
            .dynamic_signature_eligibility(declaring_trait, requirement)
            .is_ok()
        {
            rows.push(DynamicConformanceTableRow {
                requirement_identity: Arc::from(requirement_identity),
                realization_identity: Arc::from(realization_identity),
                realization: StateKey {
                    machine: retained.realization_machine,
                    state: retained.realization_state,
                    ..StateKey::default()
                },
            });
        }
    }
    rows.sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
    if rows
        .windows(2)
        .any(|pair| pair[0].requirement_identity == pair[1].requirement_identity)
    {
        return Err(Diagnostic::error(
            "dynamic table contains a duplicate normalized requirement slot",
        ));
    }
    Ok((
        program.symbols.display_path(target_trait, "::"),
        program.symbols.display_path(conformance_symbol, "::"),
        rows,
    ))
}

fn exact_trait(
    program: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_checked_trees::trait_definition::TraitDefinition, Diagnostic> {
    let mut matches = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let value = matches
        .next()
        .ok_or_else(|| Diagnostic::error("dynamic table trait identity is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "dynamic table trait identity is duplicated",
        ));
    }
    Ok(value)
}

fn exact_conformance(
    program: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_checked_trees::trait_definition::Conformance, Diagnostic> {
    let mut matches = program
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == symbol);
    let value = matches
        .next()
        .ok_or_else(|| Diagnostic::error("dynamic table conformance identity is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "dynamic table conformance identity is duplicated",
        ));
    }
    Ok(value)
}

fn exact_requirement<'program>(
    program: &'program CheckedTrees,
    declaring_trait: &'program psi_checked_trees::trait_definition::TraitDefinition,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&'program psi_checked_trees::signature::StateSignature, Diagnostic> {
    let mut matches = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .filter(|requirement| requirement.symbol == symbol);
    let value = matches
        .next()
        .ok_or_else(|| Diagnostic::error("dynamic table requirement identity is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "dynamic table requirement identity is duplicated",
        ));
    }
    Ok(value)
}

fn exact_machine(
    program: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_checked_trees::machine::Machine, Diagnostic> {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let value = matches
        .next()
        .ok_or_else(|| Diagnostic::error("dynamic table realization identity is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "dynamic table realization identity is duplicated",
        ));
    }
    Ok(value)
}

fn dynamic_table_symbol(
    trait_identity: &str,
    conformance_identity: &str,
    rows: &[DynamicConformanceTableRow],
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for value in std::iter::once(trait_identity)
        .chain(std::iter::once(conformance_identity))
        .chain(rows.iter().flat_map(|row| {
            [
                row.requirement_identity.as_ref(),
                row.realization_identity.as_ref(),
            ]
        }))
    {
        hash ^= value.len() as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        for byte in value.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("omega_dynamic_conformance_{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_platform_interface::HostCallPlan;
    use omega_runtime_branching::RuntimeBranchingCallPlan;
    use omega_runtime_text::RuntimeTextPlan;
    use omega_state_graph::build_runtime_flow_plan;
    use omega_state_storage::StateStoragePlan;
    use omega_state_values::StateValuePlan;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;

    fn checked_program_and_calls() -> (CheckedTrees, StateCallPlan) {
        let source = r#"
            trait Shape { machine code(&self) -> i32; }
            data Item {}
            Primary: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 7 } }
            }
            machine dispatch(erased: &dyn Shape) -> i32 {
                let result: i32 = erased.code();
                transition { _ -> result }
            }
            machine run(item: Item) -> i32 {
                let erased: &dyn Shape = &item as &dyn Item::Primary;
                let first: i32 = dispatch(erased);
                let second: i32 = dispatch(erased);
                transition { _ -> second }
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate");
        let checked = lower_typed_trees(typed).expect("check");
        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = omega_state_graph_to_control_flow::build_control_flow_plan(&state_graph)
            .expect("control flow");
        let run = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry = control_flow
            .states
            .span(run.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run entry");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry).expect("runtime flow");
        let calls = omega_state_calls::build_state_call_plan(
            &control_flow,
            &HostCallPlan::default(),
            &runtime_flow,
        );
        (checked, calls)
    }

    fn build(checked: &CheckedTrees, calls: &StateCallPlan) -> Result<TargetDataPlan, Diagnostic> {
        crate::build_target_data_plan_with_dynamic_conformances(
            checked,
            &HostCallPlan::default(),
            &StateStoragePlan::default(),
            &StateValuePlan::default(),
            &RuntimeBranchingCallPlan::default(),
            &RuntimeTextPlan::default(),
            calls,
            RuntimeAbiPlan {
                pointer_size: 8,
                pointer_alignment: 8,
            },
        )
    }

    #[test]
    fn repeated_pass_through_uses_one_private_aligned_exact_table() {
        let (checked, calls) = checked_program_and_calls();
        let data = build(&checked, &calls).expect("dynamic table plan");
        let [table] = data.dynamic_conformance_tables.storage_slice() else {
            panic!("one deduplicated table")
        };
        let object = data.objects.get(table.object);
        assert_eq!(object.kind, TargetDataObjectKind::DynamicConformanceTable);
        assert_eq!(object.offset % 8, 0);
        assert_eq!(data.bytes.span(object.bytes), Some(&[0; 8][..]));
        assert_eq!(table.rows.len(), 1);
        assert!(!table.rows[0].requirement_identity.is_empty());
        assert!(!table.rows[0].realization_identity.is_empty());
        assert!(table.rows[0].realization.is_valid());
    }

    #[test]
    fn indirect_dispatch_owns_table_demand_without_forwarded_argument_evidence() {
        let (checked, mut calls) = checked_program_and_calls();
        let argument_handles = calls
            .arguments
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in argument_handles {
            calls.arguments.get_mut(handle).dynamic_conformance = None;
        }
        assert!(
            calls
                .calls
                .iter()
                .any(|(_, call)| call.dynamic_dispatch.is_some()),
            "fixture must retain indirect dispatch evidence"
        );
        let data = build(&checked, &calls).expect("dispatch-owned dynamic table plan");
        assert_eq!(data.dynamic_conformance_tables.len(), 1);
    }

    #[test]
    fn normalized_row_identity_drift_fails_before_byte_publication() {
        let (checked, mut calls) = checked_program_and_calls();
        let argument = calls
            .arguments
            .iter()
            .find(|(_, argument)| argument.dynamic_conformance.is_some())
            .map(|(handle, _)| handle)
            .expect("dynamic argument");
        calls
            .arguments
            .get_mut(argument)
            .dynamic_conformance
            .as_mut()
            .unwrap()
            .rows[0]
            .requirement_identity = "drifted".to_owned();
        let error = build(&checked, &calls).expect_err("identity drift must reject");
        assert!(error.message.contains("requirement identity drifted"));
    }
}

//! A layout policy returns its public fields as ordinary data. Its sealed
//! placement calls record the actual second argument and the exact slot selected
//! at that call. Only executed calls contribute a private field.
use checked_program_to_psi as production;
use super::PrivateLayoutPlacementReceipt;
use crate::compile_time_invocations::BuildTimeValue;
use crate::source_state::expression::{ExpressionHandle, StaticMachineArgument};
use crate::source_state::{PlanLaidGeometry, PlanLaidSchemaField, PrivateCallbackSlotApplication};
use language_core::sealed_intrinsics::SealedIntrinsic;
use layout_plans::{LayoutPlacementReport, PrivateCallbackLayoutDemandReport};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_interpreter::{TerminalCallObservation, TerminalProgram, TerminalStorageContents};
use terminal_psi::{StructuralTypeShape, TerminalModule};

pub(super) struct Site<'source> {
    pub call: TerminalCallObservation,
    expression: ExpressionHandle,
    slot: &'source StaticMachineArgument,
    parameter: Option<StructuralTypeId>,
}

pub(super) fn executed_sites<'s>(
    accepted: &'s production::AcceptedTerminalSource,
    module: &TerminalModule,
) -> Result<Vec<Site<'s>>, String> {
    let typed = accepted.source();
    let placements: Vec<_> = accepted
        .closure()
        .elaboration()
        .machines
        .iter()
        .filter_map(|(id, &symbol)| {
            (typed.symbols.sealed_intrinsic(symbol)
                == Some(SealedIntrinsic::PrivateLayoutPlacement))
            .then_some(id)
        })
        .collect();
    if placements.is_empty() {
        return Ok(Vec::new());
    }
    // Match both directions: missing correspondence would make a private slot
    // look like padding. Index only the calls to the sealed placement helpers.
    let mut emitted: Vec<_> = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .module_invocations()
                .map(move |call| ((machine.id, call.site), call.inputs.callee))
        })
        .filter(|(_, callee)| placements.contains(callee))
        .collect();
    emitted.sort_unstable_by_key(|(site, _)| *site);
    let mut sites = Vec::with_capacity(emitted.len());
    for row in accepted.emitted_source_calls() {
        let crate::source_state::SourceNode::Expression(expression) = row.source else {
            continue;
        };
        let Some(call) = typed.expression_table.call_node(expression) else {
            continue;
        };
        let Some(private) = &call.private_layout_operation else {
            continue;
        };
        let symbol = typed.symbols.get(call.target_symbol).parent;
        if typed.symbols.sealed_intrinsic(symbol) != Some(SealedIntrinsic::PrivateLayoutPlacement) {
            return Err("private layout placement did not call the sealed core declaration".into());
        }
        let position = emitted
            .binary_search_by_key(&(row.machine, row.site), |(site, _)| *site)
            .map_err(|_| "private layout call has no emitted invocation")?;
        let callee = emitted[position].1;
        if accepted.closure().elaboration().machines.get(callee) != Some(&symbol) {
            return Err("private layout call targets another machine".into());
        }
        let target = module
            .machine(callee)
            .ok_or("private layout call lost its callee")?;
        let argument = 1;
        let parameter = target
            .structural_parameters
            .iter()
            .find(|p| p.position == argument)
            .map(|p| p.structural_type);
        sites.push(Site {
            call: TerminalCallObservation {
                caller: row.machine,
                site: row.site,
                callee,
                argument,
            },
            expression,
            slot: &private.selected_slot,
            parameter,
        });
    }
    sites.sort_unstable_by_key(|row| (row.call.caller, row.call.site));
    if sites.windows(2).any(|pair| {
        (pair[0].call.caller, pair[0].call.site) == (pair[1].call.caller, pair[1].call.site)
    }) {
        return Err("private layout repeats an emitted call site".into());
    }
    if emitted.len() != sites.len() {
        return Err("an emitted private placement lacks its authored slot selection".into());
    }
    Ok(sites)
}

pub(super) fn decode(
    program: &TerminalProgram<'_>,
    sites: &[Site<'_>],
    events: Vec<terminal_interpreter::ObservedTerminalCall>,
) -> Result<Vec<PrivateLayoutPlacementReceipt>, String> {
    events
        .into_iter()
        .map(|event| {
            let i = sites
                .binary_search_by_key(&(event.call.caller, event.call.site), |row| {
                    (row.call.caller, row.call.site)
                })
                .map_err(|_| "unrecognized private layout execution")?;
            let site = &sites[i];
            if site.call != event.call {
                return Err("private layout execution names another callee or argument".into());
            }
            let scalar = |value: &TerminalStorageContents| match value {
                TerminalStorageContents::Scalar { value, .. } => {
                    build_value_projection::scalar_result(program, *value)
                }
                _ => Err("private placement argument has a non-scalar leaf".into()),
            };
            let placement = if let Some(parameter) = site.parameter {
                let owner = program
                    .module()
                    .structural_type(parameter)
                    .ok_or("private placement lost its FieldPlan declaration")?;
                let StructuralTypeShape::Sum { cases } = &owner.shape else {
                    return Err("private placement requires a FieldPlan sum".into());
                };
                let case_id = event
                    .values
                    .iter()
                    .find_map(|row| match (&row.path[..], &row.contents) {
                        ([], TerminalStorageContents::Case(id)) => Some(*id),
                        _ => None,
                    })
                    .ok_or("private placement lacks its executed FieldPlan case")?;
                let case = cases
                    .iter()
                    .find(|case| case.id == case_id)
                    .ok_or("private placement has an unknown FieldPlan case")?;
                if event.values.len() != case.fields.len() + 1 {
                    return Err("private placement has incomplete or extra FieldPlan values".into());
                }
                let label = |identity| {
                    program
                        .module()
                        .identities
                        .text(identity)
                        .ok_or("private placement lost a FieldPlan member identity")
                };
                let mut payload = Vec::with_capacity(case.fields.len());
                for field in &case.fields {
                    let row = event
                        .values
                        .iter()
                        .find(|row| row.path == [Path::Case(case_id), Path::Field(field.id)])
                        .ok_or("private placement lost an active FieldPlan field")?;
                    payload.push((label(field.identity)?.to_owned(), scalar(&row.contents)?));
                }
                BuildTimeValue::Case {
                    variant: label(case.identity)?.to_owned(),
                    payload,
                }
            } else {
                let [row] = event.values.as_slice() else {
                    return Err("private offset placement has no single scalar argument".into());
                };
                if !row.path.is_empty() {
                    return Err("private offset placement has a non-root argument".into());
                }
                scalar(&row.contents)?
            };
            Ok(PrivateLayoutPlacementReceipt {
                operation_expression: site.expression,
                selected_slot: site.slot.clone(),
                placement,
            })
        })
        .collect()
}

fn placement(value: &BuildTimeValue) -> Result<LayoutPlacementReport, String> {
    fn count(value: &BuildTimeValue) -> Result<u64, String> {
        match value {
            BuildTimeValue::Int(n) => Ok(*n as u64),
            _ => Err("private callback placement needs a u64".into()),
        }
    }
    if let BuildTimeValue::Int(_) = value {
        return Ok(LayoutPlacementReport::At {
            offset: count(value)?,
        });
    }
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err("private placement needs a FieldPlan".into());
    };
    let field = |name| {
        payload
            .iter()
            .find(|(label, _)| label == name)
            .ok_or_else(|| format!("private FieldPlan lacks {name}"))
            .and_then(|(_, value)| count(value))
    };
    match variant.as_str() {
        "At" => Ok(LayoutPlacementReport::At {
            offset: field("offset")?,
        }),
        "Bits" => Ok(LayoutPlacementReport::Bits {
            container: field("container")?,
            container_width: field("container_width")?,
            destination_lsb: field("destination_lsb")?,
            source_lsb: field("source_lsb")?,
            width: field("width")?,
        }),
        _ => Err("private callback placement needs a pointer offset or bit fragment".into()),
    }
}

pub(super) fn close(
    typed: &mut crate::source_state::TypedTrees,
    accepted: &crate::source_state::FrozenTypedTrees,
    receipts: &[PrivateLayoutPlacementReceipt],
    owner: SymbolHandle,
    policy: &StaticMachineArgument,
    semantic: crate::source_state::types::TypeReferenceHandle,
    sources: &[(source_text::SourceId, [u8; 32])],
) -> Result<Vec<PrivateCallbackLayoutDemandReport<PrivateCallbackSlotApplication>>, String> {
    use crate::source_state::type_identity::{
        IndexIdentityArgument, portable_applied_index_identity,
    };
    if receipts.is_empty() {
        return Ok(Vec::new());
    }
    let identity = |argument| {
        portable_applied_index_identity(typed, argument, owner, &[])
            .and_then(|id| typed.type_identity_text(id))
            .ok_or("active layout policy lost its applied identity")
    };
    let layout = identity(IndexIdentityArgument::StaticArgument(policy))?;
    let semantic = identity(IndexIdentityArgument::TypeArgument(semantic))?;
    let mut rows = receipts.iter().collect::<Vec<_>>();
    rows.sort_unstable_by_key(|r| {
        (
            r.selected_slot.symbol.arena_index(),
            r.selected_slot.symbol.generation(),
        )
    });
    let mut demands = Vec::new();
    for group in
        rows.chunk_by(|left, right| left.selected_slot.symbol == right.selected_slot.symbol)
    {
        let slot =
            typed.private_callback_slot_application(&group[0].selected_slot, owner, sources)?;
        let selected = &slot.selection;
        if selected.declaration_identity
            != accepted.normalized_hermetic_symbol_identity(group[0].selected_slot.symbol)?
        {
            return Err("the layout receipt selects another source declaration".into());
        }
        if selected.layout_identity != layout || selected.semantic_identity != semantic {
            return Err("private callback slot belongs to another layout or semantic type".into());
        }
        let mut placements = Vec::with_capacity(group.len());
        for receipt in group {
            if receipt.selected_slot.const_literal.is_some()
                || receipt.selected_slot.evidence_projection.is_some()
                || receipt.selected_slot.application.is_some()
                || receipt
                    .selected_slot
                    .authored_machine_application
                    .is_valid()
            {
                return Err("private callback slot changes its closed application".into());
            }
            placements.push(placement(&receipt.placement)?);
        }
        placements.sort_unstable_by_key(layout_plans::layout_placement_sort_key);
        demands.push(PrivateCallbackLayoutDemandReport {
            slot_identity: format!(
                "{}#{:016x}",
                selected.declaration_identity, selected.application_report_fingerprint
            ),
            layout_subject_identity: layout.clone(),
            callback_requirement_identity: selected.requirement_identity.clone(),
            slot_application: slot,
            placements,
        });
    }
    demands.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(demands)
}

pub(super) fn validate(
    geometry: &PlanLaidGeometry,
    fields: &[PlanLaidSchemaField],
    target: &layout_plans::LayoutTargetFacts,
) -> Result<(), String> {
    if geometry.private_callback_demands.is_empty() {
        return Ok(());
    }
    let layout = &geometry.validated_layout;
    let size = layout
        .size
        .ok_or("private callback slots require fixed storage")?;
    let offsets = layout
        .offsets
        .as_deref()
        .ok_or("private layout lost its public field offsets")?;
    if offsets.len() != fields.len() {
        return Err("private layout public offsets differ from its schema".into());
    }
    let mut occupied = Vec::new();
    for (field, offset) in fields.iter().zip(offsets) {
        if field.byte_size == 0 {
            continue;
        }
        let start = offset
            .checked_mul(8)
            .ok_or("private layout field start overflows")?;
        let end = offset
            .checked_add(field.byte_size)
            .and_then(|end| end.checked_mul(8))
            .ok_or("private layout field end overflows")?;
        occupied.push((start, end));
    }
    for demand in &geometry.private_callback_demands {
        occupied.extend(layout_plans::validate_private_callback_placements(
            &demand.placements,
            target.pointer_size,
            target.pointer_alignment,
            size,
            layout.align,
        )?);
    }
    occupied.sort_unstable();
    if occupied.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("private callback placement overlaps other storage in its layout".into());
    }
    Ok(())
}

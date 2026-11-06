//! Loading an owned value must reject duplicate storage claims and unknown
//! cases, even when a valid record lists mutually exclusive payloads.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, TerminalModule};

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let selected = find_alternative_claims(module).ok_or(
        "ownership controls need a constructed record with alternative case payload claims",
    )?;
    let operation =
        &module.machines[selected.machine].blocks[selected.block].operations[selected.operation];
    let result = operation
        .result
        .as_structural()
        .ok_or("ownership control lost its result")?;
    let left = &result.claims[selected.left].path;
    let mut unknown = result.claims[selected.right].path.clone();
    unknown[selected.case] = StructuralPathSegment::Case(unknown_case(module)?);
    // Keep the actual constructor, claim identities and order. Each control
    // changes only one path, so an unrelated refusal cannot mask acceptance.
    for (name, path, reason) in [
        (
            "duplicate owned payload",
            left.clone(),
            "aggregate claim bindings overlap",
        ),
        (
            "ancestor owned payload",
            left[..selected.case].to_vec(),
            "aggregate claim bindings overlap",
        ),
        (
            "unknown alternative payload",
            unknown,
            "structural path has an unknown case",
        ),
    ] {
        let mut invalid = module.clone();
        let OperationResult::Structural(result) = &mut invalid.machines[selected.machine].blocks
            [selected.block]
            .operations[selected.operation]
            .result
        else {
            return Err("ownership control lost its constructor result".into());
        };
        result.claims[selected.right].path = path;
        super::reject_module_at_ingress(&invalid, name, reason)?;
    }
    Ok(())
}

struct AlternativeClaims {
    machine: usize,
    block: usize,
    operation: usize,
    left: usize,
    right: usize,
    case: usize,
}

fn find_alternative_claims(module: &TerminalModule) -> Option<AlternativeClaims> {
    for (machine, body) in module.machines.iter().enumerate() {
        for (block, body) in body.blocks.iter().enumerate() {
            for (operation, row) in body.operations.iter().enumerate() {
                if !matches!(row.kind, OperationKind::EstablishRecord { .. }) {
                    continue;
                }
                let Some(result) = row.result.as_structural() else {
                    continue;
                };
                for (left, a) in result.claims.iter().enumerate() {
                    for (right, b) in result.claims.iter().enumerate().skip(left + 1) {
                        if let Some(case) = alternative_case(&a.path, &b.path) {
                            return Some(AlternativeClaims {
                                machine,
                                block,
                                operation,
                                left,
                                right,
                                case,
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

fn alternative_case(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> Option<usize> {
    if left.len() != right.len() {
        return None;
    }
    let case = left.iter().zip(right).position(|(a, b)| a != b)?;
    if !matches!(
        (&left[case], &right[case]),
        (
            StructuralPathSegment::Case(_),
            StructuralPathSegment::Case(_)
        )
    ) {
        return None;
    }
    left.iter()
        .zip(right)
        .enumerate()
        .all(|(index, (a, b))| index == case || a == b)
        .then_some(case)
}

fn unknown_case(module: &TerminalModule) -> Result<semantic_vocabulary::StructuralCaseId, String> {
    let maximum = module
        .structural_types
        .iter()
        .filter_map(|declaration| {
            if let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape {
                Some(cases.iter().map(|case| case.id.get()).max().unwrap_or(0))
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);
    maximum
        .checked_add(1)
        .and_then(semantic_vocabulary::StructuralCaseId::new)
        .ok_or_else(|| "ownership control cannot allocate an unknown case identity".into())
}

//! Project selected calling evidence onto its checked direct realization.
//!
//! The source row already proves the requirement/realization correspondence.
//! Complete semantic storage, static operands and callback binders must agree
//! with the actual Product declaration before its own ABI identity is minted.
//! Equal byte geometry alone never supplies this correspondence. The selected
//! validated plan and all policy inputs stay unchanged; the original receipt
//! remains in the leaf's satisfied schema.

use calling_conventions::{BoundaryCallingApplication, callback_nominal_report_fingerprint};

pub(super) fn project(
    module: &terminal_psi::TerminalModule,
    callable: &str,
    selected: &[BoundaryCallingApplication],
) -> Result<Vec<BoundaryCallingApplication>, String> {
    let mut originals = Vec::with_capacity(selected.len());
    for application in selected {
        let declaration = module
            .calling_abi_signature(application.abi.id())
            .ok_or("selected direct calling application lost its Product declaration")?;
        if declaration.identity(&module.identities)? != application.abi
            || application.recomputed_commitment()? != application.commitment
            || callback_nominal_report_fingerprint(
                b"omega.boundary-plan-application-report.v3",
                &[&application.commitment],
            ) != application.report_fingerprint
        {
            return Err("selected direct calling application has stale declaration custody".into());
        }
        let storage = declaration.storage.as_ref().map_err(Clone::clone)?;
        originals.push((application, declaration, storage));
    }
    let mut projected: Vec<BoundaryCallingApplication> = Vec::new();
    for declaration in module.calling_abi_signatures.iter().filter(|declaration| {
        module.identities.text(declaration.requirement_identity) == Some(callable)
            && declaration.storage.is_ok()
    }) {
        let abi = declaration.identity(&module.identities)?;
        let storage = declaration.storage.as_ref().map_err(Clone::clone)?;
        let mut matched = false;
        for (original, source, source_storage) in &originals {
            if abi.operands != original.abi.operands
                || declaration.formals != source.formals
                || storage != *source_storage
            {
                continue;
            }
            matched = true;
            let mut application = (**original).clone();
            application.abi = abi.clone();
            application.requirement_identity = callable.to_owned();
            application.commitment = application.recomputed_commitment()?;
            application.report_fingerprint = callback_nominal_report_fingerprint(
                b"omega.boundary-plan-application-report.v3",
                &[&application.commitment],
            );
            if let Some(previous) = projected.iter().find(|previous| previous.abi == abi) {
                if previous != &application {
                    return Err(
                        "actual direct declaration has competing selected calling policies".into(),
                    );
                }
            } else {
                projected.push(application);
            }
        }
        if !matched {
            return Err(
                "actual direct declaration differs from the complete selected calling signature"
                    .into(),
            );
        }
    }
    if projected.is_empty() {
        return Err(
            "direct realization has no complete corresponding Product ABI application".into(),
        );
    }
    Ok(projected)
}

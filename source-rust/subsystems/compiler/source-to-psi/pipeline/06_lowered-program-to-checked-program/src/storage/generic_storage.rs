//! A generic type's storage keeps the binder that declared it. Its bounds
//! and unknown contents cannot be replaced with copy or cleanup permission,
//! even if a producer changes the storage declaration.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::StructuralTypeShape;

pub(crate) fn declared(elaboration: &crate::Elaboration) -> Checked<()> {
    let machine = MachineId::new(1).expect("one is nonzero");
    let fault = |detail| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Machine,
            detail,
        )
    };
    let types = &elaboration.module.types.structural_types;
    if types.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(fault(
            "symbolic storage requires the ordered type declaration roster",
        ));
    }
    let mut declared = elaboration
        .generic_storage
        .iter()
        .map(|source| source.storage)
        .collect::<Vec<_>>();
    declared.sort_unstable();
    if declared.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(fault("symbolic storage repeats a source binder carrier"));
    }
    for source in &elaboration.generic_storage {
        if !source.parameter.is_valid()
            || elaboration.generic_binders.get(source.ordinal as usize) != Some(&source.parameter)
        {
            return Err(fault("symbolic storage changed its exact source binder"));
        }
        let row = types
            .binary_search_by_key(&source.storage, |row| row.id)
            .ok()
            .and_then(|position| types.get(position))
            .ok_or_else(|| fault("symbolic storage lost its declared carrier"))?;
        let identity = elaboration.module.identities.text(row.identity);
        let declaration = elaboration.module.identities.text(row.declaration_identity);
        if !matches!(row.shape, StructuralTypeShape::TypeParameter)
            || source.identity.is_empty()
            || identity != Some(source.identity.as_str())
            || declaration != identity
            || row.multiplicity != source.multiplicity
            || row.owns_linear_claim
            || row.no_code_affine_cleanup
            || row.descriptor_cleanup
            || row.nominal_cleanup.is_some()
            || row.quotient_carrier.is_some()
            || row.scalar_restriction.is_some()
            || !row.requires.is_empty()
        {
            return Err(fault(
                "symbolic storage changed its declared bound or unknown contents",
            ));
        }
    }
    if types.iter().any(|row| {
        matches!(row.shape, StructuralTypeShape::TypeParameter)
            && declared.binary_search(&row.id).is_err()
    }) {
        return Err(fault(
            "symbolic storage has no complete source binder context",
        ));
    }
    Ok(())
}

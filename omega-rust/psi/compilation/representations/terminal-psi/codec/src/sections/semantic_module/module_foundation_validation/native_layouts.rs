//! Validate portable native provenance at receiving ingress, never by rerunning
//! the Source policy. Semantic geometry and nominal associations must agree.

use crate::codec_error::CodecError;
use terminal_psi::{StructuralRecordStorage, StructuralTypeShape, TerminalModule};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let fail = |reason: &str| CodecError::NativeLayout(reason.to_owned());
    let expected = module
        .structural_types
        .iter()
        .filter_map(|row| matches!(row.shape, StructuralTypeShape::Record { .. }).then_some(row.id))
        .collect::<Vec<_>>();
    let observed = module
        .native_layouts
        .records()
        .map(|row| row.record)
        .collect::<Vec<_>>();
    if observed != expected || observed.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(fail(
            "native layout catalog differs from the complete ordered record inventory",
        ));
    }
    for native in module.native_layouts.records() {
        let record = module
            .structural_type(native.record)
            .ok_or_else(|| fail("native layout has no semantic record"))?;
        let StructuralTypeShape::Record { fields, storage } = &record.shape else {
            return Err(fail("native layout names a non-record"));
        };
        for identity in [native.data_identity, native.policy_identity] {
            if module.identities.text(identity).is_none() {
                return Err(fail("native layout has an out-of-range nominal identity"));
            }
        }
        let nominal_fields = module
            .native_layouts
            .fields(native)
            .map_err(CodecError::NativeLayout)?;
        if nominal_fields.len() != fields.len()
            || nominal_fields.iter().zip(fields).any(|(nominal, field)| {
                nominal.field != field.id
                    || module
                        .identities
                        .text(nominal.declaration_identity)
                        .is_none()
            })
        {
            return Err(fail(
                "native layout nominal field inventory differs from its record",
            ));
        }
        let realizations = module
            .native_layouts
            .realizations(native)
            .map_err(CodecError::NativeLayout)?;
        if native.policy_identity.is_absent() {
            if !realizations.is_empty()
                || matches!(storage, StructuralRecordStorage::TargetLayout(_))
            {
                return Err(fail(
                    "natural native layout carries authored policy storage",
                ));
            }
            continue;
        }
        let semantic = match storage {
            StructuralRecordStorage::TargetLayout(id) => Some(
                module
                    .target_layout_applications
                    .iter()
                    .find(|application| {
                        application.id == *id && application.record == native.record
                    })
                    .ok_or_else(|| fail("native layout lost its exact semantic application"))?,
            ),
            StructuralRecordStorage::Authored(_) => None,
            _ => {
                return Err(fail(
                    "planned native layout has no authored semantic storage",
                ));
            }
        };
        let expected_targets = semantic.map(|application| {
            application
                .realizations
                .iter()
                .map(|row| row.target.as_str())
                .collect::<Vec<_>>()
        });
        let actual_targets = realizations
            .iter()
            .filter_map(|row| row.target.as_ref().map(|target| target.target.as_str()))
            .collect::<Vec<_>>();
        if match &expected_targets {
            Some(targets) => {
                *targets != actual_targets || actual_targets.len() != realizations.len()
            }
            None => realizations.len() != 1 || realizations[0].target.is_some(),
        } {
            return Err(fail(
                "native layout alternatives differ from their complete semantic roster",
            ));
        }
        let policy = module
            .identities
            .text(native.policy_identity)
            .ok_or_else(|| fail("native layout lost its policy identity"))?;
        for realization in realizations {
            let report = &realization.report;
            let physical = match &realization.target {
                Some(target) => {
                    let selected = semantic
                        .and_then(|application| {
                            application
                                .realizations
                                .iter()
                                .find(|row| row.target == target.target)
                        })
                        .ok_or_else(|| fail("native target member lost its semantic storage"))?;
                    if selected.pointer_size != target.pointer_size
                        || selected.pointer_alignment != target.pointer_alignment
                        || selected.little_endian != target.little_endian
                    {
                        return Err(fail("native layout changed its exact target facts"));
                    }
                    selected.storage
                }
                None => {
                    let StructuralRecordStorage::Authored(storage) = storage else {
                        return Err(fail("invariant native layout has pending semantic storage"));
                    };
                    *storage
                }
            };
            if report.layout.size != Some(physical.byte_size)
                || report.layout.align != physical.byte_alignment
            {
                return Err(fail(
                    "native layout report differs from semantic extent or alignment",
                ));
            }
            let slots = &report.private_callback_demands;
            if slots
                .windows(2)
                .any(|pair| pair[0].slot_identity >= pair[1].slot_identity)
            {
                return Err(fail("native layout slots are not uniquely ordered"));
            }
            for slot in slots {
                slot.slot_application
                    .validate_identity_encoding()
                    .map_err(CodecError::NativeLayout)?;
                if slot.slot_identity
                    != format!(
                        "{}#{:016x}",
                        slot.slot_application.declaration_identity,
                        slot.slot_application.application_report_fingerprint
                    )
                    || slot.layout_subject_identity != policy
                    || slot.slot_application.layout_identity != policy
                    || slot.callback_requirement_identity
                        != slot.slot_application.requirement_identity
                {
                    return Err(fail(
                        "native layout slot changed its selected nominal application or roles",
                    ));
                }
                // Pointer geometry closes later with the selected target, not
                // with the host that receives an invariant report.
                if slot.offset > physical.byte_size {
                    return Err(fail(
                        "native layout slot starts outside its containing storage",
                    ));
                }
            }
        }
    }
    Ok(())
}

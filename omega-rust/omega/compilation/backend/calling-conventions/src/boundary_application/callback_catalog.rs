/*
We retain source-free callback layout geometry (`BoundaryCallbackLayout`,
`CallbackDataGeometry`, `CallbackInlineFieldGeometry`, `CallbackSlotGeometry`),
arena-backed private callback placement fragments
(`PrivateCallbackPlacements`), authenticated callback placement admission
(`authenticated_callback_placements`), and Terminal slot selection recovery
(`callback_destination_slot`). Structural replay checks containment and the
complete ordered nominal path, while authenticity comes from joining the
complete untrusted payload to an independently selected method commitment and
native layout realization.
*/

use super::{BoundaryCallingApplication, BoundaryNativeParameterShape};
use crate::digest_encoder::DomainDigestEncoder;
use crate::{
    BoundaryNativeParameterOrigin, CallbackMaterializationContext, CallbackRequirementId,
    LayoutPlanId, LayoutSlotId, NativeParameterApplication, NativePlace, StaticMachineBinderId,
    callback_layout_field_slot_id, callback_layout_slot_id, callback_requirement_id,
};
use arena::{Arena, HandleSpan};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use layout_plans::LayoutPlacementReport;
use target::NativeTarget;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackDataGeometry {
    pub identity: String,
    pub policy: Option<String>,
    pub layout: LayoutPlanId,
    pub size: u64,
    pub alignment: u64,
}

impl Default for CallbackDataGeometry {
    fn default() -> Self {
        Self {
            identity: String::new(),
            policy: None,
            layout: LayoutPlanId::new(1).expect("nonzero arena sentinel"),
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackInlineFieldGeometry {
    pub identity: String,
    pub offset: u64,
    pub child: CallbackDataGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackSlotGeometry {
    pub identity: String,
    pub policy: String,
    pub requirement_identity: String,
    pub layout: LayoutPlanId,
    pub slot: LayoutSlotId,
    pub requirement: CallbackRequirementId,
    pub conformance_commitment: [u8; 32],
    pub placements: PrivateCallbackPlacements,
    pub size: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackLayout {
    pub formal_ordinal: u32,
    pub native_ordinal: u32,
    pub destination: NativePlace,
    pub root: CallbackDataGeometry,
    fields: Arena<CallbackInlineFieldGeometry>,
    path: HandleSpan<CallbackInlineFieldGeometry>,
    pub terminal: CallbackSlotGeometry,
}

impl Default for BoundaryCallbackLayout {
    fn default() -> Self {
        Self::new(
            0,
            0,
            NativePlace::Parameter(crate::NativeParameterId::new(1).expect("arena sentinel")),
            CallbackDataGeometry::default(),
            &[],
            CallbackSlotGeometry {
                identity: String::new(),
                policy: String::new(),
                requirement_identity: String::new(),
                layout: LayoutPlanId::new(1).expect("arena sentinel"),
                slot: LayoutSlotId::new(1).expect("arena sentinel"),
                requirement: CallbackRequirementId::new(1).expect("arena sentinel"),
                conformance_commitment: [0; 32],
                placements: PrivateCallbackPlacements::default(),
                size: 0,
                alignment: 1,
            },
        )
    }
}

impl BoundaryCallbackLayout {
    pub fn new(
        formal_ordinal: u32,
        native_ordinal: u32,
        destination: NativePlace,
        root: CallbackDataGeometry,
        fields: &[CallbackInlineFieldGeometry],
        terminal: CallbackSlotGeometry,
    ) -> Self {
        let mut arena = Arena::with_capacity(fields.len());
        let mut path = HandleSpan::empty();
        for field in fields {
            arena.append_to_span(&mut path, field.clone());
        }
        Self {
            formal_ordinal,
            native_ordinal,
            destination,
            root,
            fields: arena,
            path,
            terminal,
        }
    }

    pub fn fields(&self) -> Result<&[CallbackInlineFieldGeometry], String> {
        self.fields
            .span(self.path)
            .ok_or_else(|| "callback geometry has stale path storage".to_owned())
    }

    pub fn field_placements(
        &self,
        target: NativeTarget,
    ) -> Result<Vec<LayoutPlacementReport>, String> {
        let NativePlace::Field {
            layout, field_path, ..
        } = &self.destination
        else {
            return Err("callback catalog field has a parameter destination".to_owned());
        };
        if *layout != self.root.layout || !self.root.valid() {
            return Err("callback catalog root identity is invalid".to_owned());
        }
        let mut parent = &self.root;
        let mut offset = 0u64;
        let mut path = Vec::new();
        let mut seen = vec![parent.identity.as_str()];
        for field in self.fields()? {
            if field.identity.is_empty()
                || !field.child.valid()
                || seen.contains(&field.child.identity.as_str())
                || !parent.contains(field.offset, field.child.size, field.child.alignment)
            {
                return Err("callback catalog inline field geometry is invalid".to_owned());
            }
            path.push(callback_layout_field_slot_id(
                parent.layout,
                &field.identity,
            ));
            offset = offset
                .checked_add(field.offset)
                .ok_or("callback offset overflow")?;
            parent = &field.child;
            seen.push(parent.identity.as_str());
        }
        let terminal = &self.terminal;
        if parent.policy.as_deref() != Some(terminal.policy.as_str())
            || terminal.identity.is_empty()
            || terminal.requirement_identity.is_empty()
            || terminal.slot != callback_layout_slot_id(terminal.layout, &terminal.identity)
            || terminal.requirement != callback_requirement_id(&terminal.requirement_identity)
            || terminal.conformance_commitment == [0; 32]
            || terminal.size != target.pointer_size as u64
            || terminal.alignment != target.pointer_alignment as u64
        {
            return Err("callback catalog terminal slot geometry is invalid".to_owned());
        }
        layout_plans::validate_private_callback_placements(
            terminal.placements.as_slice(),
            terminal.size,
            terminal.alignment,
            parent.size,
            parent.alignment,
        )?;
        path.push(terminal.slot);
        if path != *field_path {
            return Err("callback catalog ordered path is invalid".to_owned());
        }
        let placements = layout_plans::compose_private_callback_placements(
            terminal.placements.as_slice(),
            offset,
        )?;
        layout_plans::validate_private_callback_placements(
            &placements,
            terminal.size,
            terminal.alignment,
            self.root.size,
            self.root.alignment,
        )?;
        Ok(placements)
    }

    pub(super) fn encode(&self, encoder: &mut DomainDigestEncoder) -> Result<(), String> {
        encoder.u32(self.formal_ordinal);
        encoder.u32(self.native_ordinal);
        match &self.destination {
            NativePlace::Field {
                parameter,
                layout,
                field_path,
            } => {
                encoder.u64(parameter.get());
                encoder.u64(layout.get());
                encoder.len(field_path.len());
                for field in field_path {
                    encoder.u64(field.get());
                }
            }
            _ => return Err("callback catalog contains a non-field destination".to_owned()),
        }
        self.root.encode(encoder);
        encoder.len(self.fields()?.len());
        for field in self.fields()? {
            encoder.string(&field.identity);
            encoder.u64(field.offset);
            field.child.encode(encoder);
        }
        let terminal = &self.terminal;
        encoder.string(&terminal.identity);
        encoder.string(&terminal.policy);
        encoder.string(&terminal.requirement_identity);
        encoder.u64(terminal.layout.get());
        encoder.u64(terminal.slot.get());
        encoder.u64(terminal.requirement.get());
        encoder.raw(&terminal.conformance_commitment);
        encoder.len(terminal.placements.as_slice().len());
        for placement in terminal.placements.as_slice() {
            let (kind, first, second, third, fourth, fifth) =
                layout_plans::layout_placement_sort_key(placement);
            encoder.u64(u64::from(kind));
            for value in [first, second, third, fourth, fifth] {
                encoder.u64(value);
            }
        }
        encoder.u64(terminal.size);
        encoder.u64(terminal.alignment);
        Ok(())
    }
}

impl CallbackDataGeometry {
    fn valid(&self) -> bool {
        !self.identity.is_empty()
            && self.policy.as_ref().is_none_or(|p| !p.is_empty())
            && self.alignment.is_power_of_two()
    }
    fn contains(&self, offset: u64, size: u64, alignment: u64) -> bool {
        alignment.is_power_of_two()
            && self.alignment >= alignment
            && offset.is_multiple_of(alignment)
            && offset.checked_add(size).is_some_and(|end| end <= self.size)
    }
    fn encode(&self, encoder: &mut DomainDigestEncoder) {
        encoder.string(&self.identity);
        encoder.optional_string(self.policy.as_deref());
        encoder.u64(self.layout.get());
        encoder.u64(self.size);
        encoder.u64(self.alignment);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackPlacements {
    storage: Arena<LayoutPlacementReport>,
    placements: HandleSpan<LayoutPlacementReport>,
}

impl PrivateCallbackPlacements {
    pub fn new(placements: &[LayoutPlacementReport]) -> Self {
        let mut storage = Arena::with_capacity(placements.len());
        let mut span = HandleSpan::empty();
        for placement in placements {
            storage.append_to_span(&mut span, *placement);
        }
        Self {
            storage,
            placements: span,
        }
    }

    pub fn as_slice(&self) -> &[LayoutPlacementReport] {
        self.storage
            .span(self.placements)
            .expect("private callback placement storage")
    }
}

impl Default for PrivateCallbackPlacements {
    fn default() -> Self {
        Self::new(&[])
    }
}

pub fn encode_private_callback_placements(
    bytes: &mut Vec<u8>,
    placements: &PrivateCallbackPlacements,
) {
    bytes.extend_from_slice(&(placements.as_slice().len() as u64).to_le_bytes());
    for placement in placements.as_slice() {
        use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
        match *placement {
            LayoutPlacementReport::At { offset } => {
                bytes.push(0);
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&offset.to_le_bytes());
                bytes.extend_from_slice(&stored_width.to_le_bytes());
                bytes.push(match interpretation {
                    IntegerInterpretation::Signed => 0,
                    IntegerInterpretation::Unsigned => 1,
                });
            }
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => {
                bytes.push(2);
                for value in [
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                ] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }
}

impl BoundaryCallingApplication {
    pub fn authenticated_callback_placements(
        &self,
        expected_commitment: [u8; 32],
        binder: StaticMachineBinderId,
        destination: &NativePlace,
        parameter: &NativeParameterApplication,
        context: &CallbackMaterializationContext,
    ) -> Result<Option<Vec<LayoutPlacementReport>>, String> {
        if expected_commitment == [0; 32]
            || self.commitment != expected_commitment
            || self.recomputed_commitment()? != expected_commitment
            || !self.callback_context_closed
        {
            return Err(
                "callback application does not match the admitted method commitment".to_owned(),
            );
        }
        let native = self
            .signature
            .native_parameters
            .span(self.signature.native_parameter_rows)
            .ok_or("callback application has stale native telescope")?;
        let catalog = self
            .signature
            .callback_layouts
            .span(self.signature.callback_layout_rows)
            .ok_or("callback application has stale layout catalog")?;
        let rows = &self.plan.plan().call.callback_materializations;
        if context.binders.len() != rows.len() || context.demands.len() != rows.len() {
            return Err(
                "callback context changed its authenticated application coverage".to_owned(),
            );
        }
        for row in rows {
            let requirement = match &row.destination {
                NativePlace::Parameter(identity) => {
                    let mut matching = native.iter().filter(|entry| entry.identity == *identity);
                    let entry = matching.next().ok_or("callback parameter is absent")?;
                    if matching.next().is_some() {
                        return Err("callback parameter is ambiguous".to_owned());
                    }
                    match entry.origin {
                        BoundaryNativeParameterOrigin::PrivateCallback {
                            binder,
                            requirement,
                        } if binder == row.binder => requirement,
                        _ => return Err("callback parameter changed its binder origin".to_owned()),
                    }
                }
                NativePlace::Field { .. } => {
                    let mut matching = catalog
                        .iter()
                        .filter(|entry| entry.destination == row.destination);
                    let entry = matching
                        .next()
                        .ok_or("callback destination has no authenticated catalog entry")?;
                    if matching.next().is_some() {
                        return Err("callback catalog destination is ambiguous".to_owned());
                    }
                    entry.terminal.requirement
                }
            };
            if context
                .binders
                .iter()
                .filter(|entry| entry.binder == row.binder && entry.requirement == requirement)
                .count()
                != 1
                || context
                    .demands
                    .iter()
                    .filter(|entry| {
                        entry.destination == row.destination && entry.requirement == requirement
                    })
                    .count()
                    != 1
            {
                return Err(
                    "callback context changed an authenticated binder or requirement".to_owned(),
                );
            }
        }
        if rows
            .iter()
            .filter(|row| row.binder == binder && row.destination == *destination)
            .count()
            != 1
        {
            return Err(
                "callback occurrence has no unique authenticated binder destination".to_owned(),
            );
        }
        let identity = match destination {
            NativePlace::Parameter(identity)
            | NativePlace::Field {
                parameter: identity,
                ..
            } => identity,
        };
        let mut matching = native.iter().filter(|entry| entry.identity == *identity);
        let entry = matching
            .next()
            .ok_or("callback root has no native parameter")?;
        let ordinal = self
            .call_projection()?
            .parameter_ordinal(entry.native_ordinal)?
            .ok_or("dispatch-only formal cannot carry a native callback")?;
        if matching.next().is_some()
            || entry.identity != parameter.parameter
            || usize::try_from(parameter.native_ordinal).ok() != Some(ordinal)
            || self.plan.plan().call.parameters.get(ordinal) != Some(&parameter.placement)
            || parameter.shape != parameter.placement.shape
        {
            return Err("callback root changed its authenticated parameter application".to_owned());
        }
        match destination {
            NativePlace::Parameter(_) => {
                if !matches!(entry.shape, BoundaryNativeParameterShape::TargetFunctionPointer { byte_size, alignment }
                    if usize::from(byte_size) == self.target.pointer_size && usize::from(alignment) == self.target.pointer_alignment)
                {
                    return Err("callback parameter changed its target pointer geometry".to_owned());
                }
                Ok(None)
            }
            NativePlace::Field { .. } => {
                let layout = catalog
                    .iter()
                    .find(|layout| layout.destination == *destination)
                    .ok_or("callback field has no authenticated geometry")?;
                if entry.origin
                    != (BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: layout.formal_ordinal,
                    })
                    || entry.native_ordinal != layout.native_ordinal
                    || !matches!(entry.shape, BoundaryNativeParameterShape::Semantic(_))
                {
                    return Err("callback field changed its semantic formal association".to_owned());
                }
                Ok(Some(layout.field_placements(self.target)?))
            }
        }
    }
}

pub fn callback_destination_slot<'module>(
    module: &'module TerminalModule,
    target_profile: target::TargetProfile,
    registrar: &BoundaryCallingApplication,
    layout: &BoundaryCallbackLayout,
) -> Result<&'module PrivateCallbackSlotSelection, String> {
    let signature = module
        .calling_abi_signature(registrar.abi.id())
        .ok_or("installed registrar lost its exact declaration ABI")?;
    if signature.identity(&module.identities)? != registrar.abi {
        return Err("installed registrar changed its complete declaration ABI".into());
    }
    let storage = signature.storage.as_ref().map_err(Clone::clone)?;
    let formal = storage
        .telescope
        .parameters
        .get(layout.formal_ordinal as usize)
        .ok_or("installed callback layout has no registrar formal")?;
    if !matches!(layout.destination, NativePlace::Field { parameter, .. } if parameter == formal.identity)
        || layout.native_ordinal != formal.native_ordinal
    {
        return Err("installed callback layout changed its exact registrar formal".into());
    }
    let mut record_id = storage
        .parameters
        .get(layout.formal_ordinal as usize)
        .and_then(|parameter| parameter.callback_backing)
        .ok_or("installed callback destination has no retained formal backing")?;
    let mut expected_identity = layout.root.identity.as_str();
    for field in layout.fields()? {
        let record = module
            .native_layouts
            .record(record_id)
            .ok_or("installed callback field lost its native record")?;
        if module.identities.text(record.data_identity) != Some(expected_identity) {
            return Err("installed callback path changed its owning record application".into());
        }
        let mut matching = module
            .native_layouts
            .fields(record)?
            .iter()
            .filter(|candidate| {
                module.identities.text(candidate.declaration_identity)
                    == Some(field.identity.as_str())
            });
        let (Some(selected), None) = (matching.next(), matching.next()) else {
            return Err("installed callback path has no unique declared field".into());
        };
        record_id = module
            .structural_type(record_id)
            .and_then(|record| record.shape.record_fields())
            .and_then(|fields| fields.iter().find(|field| field.id == selected.field))
            .and_then(|field| field.field_type.as_structural())
            .ok_or("installed callback path does not retain an inline child record")?;
        expected_identity = &field.child.identity;
    }
    let record = module
        .native_layouts
        .record(record_id)
        .ok_or("installed callback slot lost its containing native record")?;
    if module.identities.text(record.data_identity) != Some(expected_identity) {
        return Err("installed callback slot changed its containing record application".into());
    }
    let realization = module
        .native_layouts
        .for_target(record, target_profile.target_name())?
        .ok_or("installed callback slot has no selected authored layout")?;
    let mut demands = realization
        .report
        .private_callback_demands
        .iter()
        .filter(|demand| demand.slot_identity == layout.terminal.identity);
    let (Some(demand), None) = (demands.next(), demands.next()) else {
        return Err("installed callback destination has no unique selected slot demand".into());
    };
    let slot = &demand.slot_application;
    slot.validate_identity_encoding()?;
    if slot.application_commitment != layout.terminal.conformance_commitment
        || slot.requirement_identity != layout.terminal.requirement_identity
        || demand.callback_requirement_identity != layout.terminal.requirement_identity
        || slot.layout_identity != layout.terminal.policy
        || demand.layout_subject_identity != layout.terminal.policy
        || demand.placements.as_slice() != layout.terminal.placements.as_slice()
        || module.identities.text(record.semantic_identity) != Some(slot.semantic_identity.as_str())
    {
        return Err("installed callback layout differs from its selected slot demand".into());
    }
    Ok(slot)
}

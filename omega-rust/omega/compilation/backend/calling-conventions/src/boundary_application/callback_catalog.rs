//! Source-free callback geometry. Structural replay checks containment and the
//! complete ordered nominal path. Authenticity is supplied separately by the
//! selected, admitted method's application commitment; self-consistency cannot
//! authenticate a producer's choice of offset.

use crate::digest_encoder::DomainDigestEncoder;
use crate::{
    CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativePlace, callback_layout_field_slot_id,
    callback_layout_slot_id, callback_requirement_id,
};
use arena::{Arena, HandleSpan};
use target::NativeTarget;

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
    pub offset: u64,
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
                offset: 0,
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

    pub fn field_offset(&self, target: NativeTarget) -> Result<usize, String> {
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
            || !parent.contains(terminal.offset, terminal.size, terminal.alignment)
        {
            return Err("callback catalog terminal slot geometry is invalid".to_owned());
        }
        path.push(terminal.slot);
        offset = offset
            .checked_add(terminal.offset)
            .ok_or("callback offset overflow")?;
        if path != *field_path
            || !self
                .root
                .contains(offset, terminal.size, terminal.alignment)
        {
            return Err("callback catalog ordered path or root containment is invalid".to_owned());
        }
        usize::try_from(offset).map_err(|_| "callback offset exceeds host range".to_owned())
    }

    pub(super) fn encode(&self, encoder: &mut DomainDigestEncoder) -> Result<(), String> {
        encoder.u32(self.formal_ordinal);
        encoder.u32(self.native_ordinal);
        // The destination's nominal parameter is part of the application, not
        // merely the physical layout/path that happens to receive its pointer.
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
        encoder.u64(terminal.offset);
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

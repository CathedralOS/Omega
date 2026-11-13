// We pair canonical callback geometry with geometry-free source-review addresses
// (`BoundaryCallbackLayoutEntry`) and replay the retained named catalog's joins
// against native and semantic parameter associations without reconstructing
// source-shaped layouts from canonical physical values.

use super::{
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, MaterializedBoundarySignature,
};
use calling_conventions::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry,
    CallbackSlotGeometry, NativeCallbackDemand, NativePlace,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackLayoutEntry {
    pub(crate) geometry: BoundaryCallbackLayout,
}

impl BoundaryCallbackLayoutEntry {
    pub fn geometry(&self) -> &BoundaryCallbackLayout {
        &self.geometry
    }
    pub const fn formal_ordinal(&self) -> u32 {
        self.geometry.formal_ordinal
    }
    pub const fn native_ordinal(&self) -> u32 {
        self.geometry.native_ordinal
    }
    pub const fn destination(&self) -> &NativePlace {
        &self.geometry.destination
    }
    pub fn root_layout(&self) -> &CallbackDataGeometry {
        &self.geometry.root
    }
    pub fn inline_fields(&self) -> Result<&[CallbackInlineFieldGeometry], String> {
        self.geometry.fields()
    }
    pub fn terminal_slot(&self) -> &CallbackSlotGeometry {
        &self.geometry.terminal
    }
    pub fn composed_placements(&self) -> Result<Vec<layout_plans::LayoutPlacementReport>, String> {
        let base = self
            .geometry
            .fields()?
            .iter()
            .try_fold(0u64, |base, field| {
                base.checked_add(field.offset)
                    .ok_or("callback composed offset overflows")
            })?;
        layout_plans::compose_private_callback_placements(
            self.geometry.terminal.placements.as_slice(),
            base,
        )
    }
    pub(crate) fn source_free(&self) -> Result<BoundaryCallbackLayout, String> {
        Ok(self.geometry.clone())
    }

    /// Join the actual canonical record path and selected native slot. Source
    /// review addresses are deliberately outside this execution representation.
    pub fn from_canonical(
        module: &terminal_psi::TerminalModule,
        geometry: BoundaryCallbackLayout,
        coordinates: &layout::CallbackLayoutCoordinates,
        target: target::TargetProfile,
    ) -> Result<Self, String> {
        if geometry.destination != coordinates.destination {
            return Err("callback coordinates name another native destination".into());
        }
        let mut parent = coordinates.root;
        for identity in &coordinates.fields {
            let field = module
                .structural_type(parent)
                .and_then(|row| row.shape.record_fields())
                .and_then(|rows| rows.iter().find(|field| field.id == *identity))
                .ok_or("callback inline path lost its exact canonical field")?;
            parent = field
                .field_type
                .as_structural()
                .ok_or("callback inline field has no canonical child record")?;
        }
        if parent != coordinates.terminal_record
            || coordinates.fields.len() != geometry.fields()?.len()
        {
            return Err("callback path differs from the canonical containing record".into());
        }
        let native = module
            .native_layouts
            .record(parent)
            .ok_or("callback slot lost its native record")?;
        let selected = module
            .native_layouts
            .for_target(native, target.target_name())?
            .ok_or("callback slot lost its selected native realization")?;
        let slot = selected
            .report
            .private_callback_demands
            .get(coordinates.slot_ordinal as usize)
            .ok_or("callback slot lost its exact native declaration ordinal")?;
        if slot.slot_application.application_commitment != geometry.terminal.conformance_commitment
        {
            return Err(
                "callback slot correspondence substituted its exact selected application".into(),
            );
        }
        Ok(Self { geometry })
    }
}

pub(super) fn validate(
    signature: &MaterializedBoundarySignature,
    realization_demands: &[NativeCallbackDemand],
) -> Result<(), String> {
    if signature.callback_demands != realization_demands
        || realization_demands
            .windows(2)
            .any(|pair| pair[0].destination >= pair[1].destination)
    {
        return Err("callback layout catalog lost its exact ordered demand context".to_owned());
    }
    let mut entries = signature.callback_layout_catalog.iter();
    for demand in realization_demands {
        if !matches!(demand.destination, NativePlace::Field { .. }) {
            continue;
        }
        let entry = entries.next().ok_or_else(|| {
            "callback layout catalog is missing a declared private field demand".to_owned()
        })?;
        if *entry.destination() != demand.destination
            || entry.terminal_slot().requirement != demand.requirement
        {
            return Err("callback layout catalog changed its exact field demand".to_owned());
        }
        validate_entry(signature, entry)?;
    }
    if entries.next().is_some() {
        return Err(
            "callback layout catalog contains an undeclared private field demand".to_owned(),
        );
    }
    Ok(())
}

fn validate_entry(
    signature: &MaterializedBoundarySignature,
    entry: &BoundaryCallbackLayoutEntry,
) -> Result<(), String> {
    let NativePlace::Field { parameter, .. } = entry.destination() else {
        return Err("callback layout catalog contains a non-field destination".to_owned());
    };
    let native_index = usize::try_from(entry.native_ordinal())
        .map_err(|_| "callback layout catalog native ordinal is out of range")?;
    let native = signature
        .native_parameters
        .get(native_index)
        .ok_or_else(|| "callback layout catalog has no exact native parameter".to_owned())?;
    let formal_index = usize::try_from(entry.formal_ordinal())
        .map_err(|_| "callback layout catalog formal ordinal is out of range")?;
    if native.identity != *parameter
        || native.native_ordinal != entry.native_ordinal()
        || native.origin
            != (BoundaryNativeParameterOrigin::SemanticFormal {
                formal_ordinal: entry.formal_ordinal(),
            })
        || signature.parameters.get(formal_index).is_none_or(|shape| {
            native.shape != BoundaryNativeParameterShape::Semantic(*shape)
                || usize::from(*shape) >= signature.shapes.len()
        })
        || signature
            .native_parameters
            .iter()
            .enumerate()
            .any(|(index, other)| {
                index != native_index
                    && (other.identity == native.identity
                        || other.native_ordinal == native.native_ordinal
                        || other.origin == native.origin)
            })
    {
        return Err(
            "callback layout catalog changed its native or semantic parameter association"
                .to_owned(),
        );
    }
    entry
        .geometry()
        .field_placements(signature.native_target)
        .map(|_| ())
}

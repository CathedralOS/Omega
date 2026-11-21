//! Current image-address rows and final code-layout descriptions. Object
//! construction projects these once; image construction never reopens the
//! physical producer to rediscover them.

use super::{Error, current_image_selections::CurrentImageSelections};
use machine_code::PlacedImageAddress;
use machine_code::code_layout::NativeCodeLayout;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectImageData {
    pub addresses: Vec<PlacedImageAddress>,
    pub selections: Option<CurrentImageSelections>,
    pub native_layouts: Arc<[NativeCodeLayout]>,
}

impl ObjectImageData {
    pub(super) fn prepare(
        source: &crate::object_input::ObjectEmissionInput,
    ) -> Result<Self, Error> {
        let placed = source.text_section();
        let selections = CurrentImageSelections::prepare_for(
            &placed.unresolved_image_addresses,
            &source.emission_facts().physical.image_origins,
        )?;
        let native_layouts = source
            .publication()
            .native_code_layouts()
            .map_err(Error::NativeCodeLayout)?;
        Ok(Self {
            addresses: placed.unresolved_image_addresses.clone(),
            selections,
            native_layouts,
        })
    }
}

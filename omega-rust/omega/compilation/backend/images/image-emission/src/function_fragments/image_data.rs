//! Current image-address rows and final code-layout descriptions. Object
//! construction projects these once; image construction never reopens the
//! physical producer to rediscover them.

use super::{Error, current_image_selections::CurrentImageSelections};
use machine_code::PlacedImageAddress;
use machine_code::code_layout::NativeCodeLayout;
use function_layout::code_layout::describe_native_code_layout_from_records;
use selected_instructions::SelectedImageAddressReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectImageData {
    pub addresses: Vec<PlacedImageAddress>,
    pub selections: Option<CurrentImageSelections>,
    pub native_layouts: Vec<NativeCodeLayout>,
}

impl ObjectImageData {
    pub(super) fn prepare(
        source: &crate::object_input::ObjectEmissionInput,
    ) -> Result<Self, Error> {
        let placed = source.text_section();
        let selections = CurrentImageSelections::prepare_for(
            &placed.unresolved_image_addresses,
            &source.emission_facts().image_origins,
        )?;
        let mut native_layouts = Vec::<NativeCodeLayout>::new();
        for address in &placed.unresolved_image_addresses {
            if let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference
                && !native_layouts
                    .iter()
                    .any(|layout| layout.machine == machine)
            {
                native_layouts.push(
                    describe_native_code_layout_from_records(
                        source.text_section(),
                        &source.frames,
                        machine,
                    )
                    .map_err(Error::NativeCodeLayout)?,
                );
            }
        }
        Ok(Self {
            addresses: placed.unresolved_image_addresses.clone(),
            selections,
            native_layouts,
        })
    }
}

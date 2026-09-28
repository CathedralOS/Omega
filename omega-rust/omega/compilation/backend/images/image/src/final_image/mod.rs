//! The final image record: sections, memory layout, symbols, relocations, and
//! the placed executable/data region inventories that emission and
//! publication read. The crate root re-exports the public surface by name.

mod data_regions;
mod executable_regions;
mod layout;
mod memory;
mod relocations;
mod root;
mod symbols;

pub use data_regions::{
    FinalDataRegion, FinalDataRegionOrigin, FinalInitializedDataDigest, PlacedDataGap,
    PlacedDataGapBytesDigest, PlacedDataRegion, PlacedDataRegionBytesDigest,
    PlacedDataRegionInventory, PlacedDataRegionInventoryDigest, place_data_extent,
    place_data_regions, validate_placed_data_region_inventory,
};
pub(crate) use executable_regions::validate_placed_executable_region_inventory_digest;
pub use executable_regions::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalExecutableTextDigest,
    PlacedExecutableGap, PlacedExecutableGapBytesDigest, PlacedExecutableRegion,
    PlacedExecutableRegionBytesDigest, PlacedExecutableRegionInventory,
    PlacedExecutableRegionInventoryDigest, StateFootprintEvidenceDigest,
    bind_compiler_entry_footprint, place_executable_regions,
    validate_placed_executable_region_inventory,
};
#[cfg(test)]
pub(crate) use executable_regions::{
    executable_inventory_digest, executable_inventory_report_fingerprint,
};
pub use layout::FinalImageLayout;
pub use memory::FinalImageMemory;
pub use relocations::{FinalImageRelocation, FinalImageRelocationTable};
pub use root::FinalImage;
pub use symbols::{
    FinalImageImport, FinalImageImportPlan, FinalImageSection, FinalImageSymbol,
    FinalImageSymbolDigest, FinalImageSymbolHandle, FinalImageSymbolTable,
    final_image_symbol_digest,
};

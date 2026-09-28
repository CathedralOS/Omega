//! The UEFI memory map buffer.

/// Growable, custody-scoped storage for one `GetMemoryMap` call family.
///
/// The carrier owns the descriptor storage and the four call cells privately;
/// the only writer past construction is the executed provider edge. Callers
/// observe capacity, the firmware-occupied map prefix, and nothing else — the
/// sealed outputs reach consumers only through `UefiMemoryMapAcquisition`. A
/// zero-capacity buffer is the UEFI probe shape: firmware reports the required
/// size through the size cell and returns `EFI_BUFFER_TOO_SMALL`.
#[must_use = "UEFI memory-map buffer custody must bind into a provider call or stay owned"]
pub struct UefiMemoryMapBuffer {
    pub(super) map: Vec<u8>,
    pub(super) map_size: usize,
    pub(super) map_key: usize,
    pub(super) descriptor_size: usize,
    pub(super) descriptor_version: u32,
    pub(super) occupied: usize,
}

impl UefiMemoryMapBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: vec![0; capacity],
            map_size: 0,
            map_key: 0,
            descriptor_size: 0,
            descriptor_version: 0,
            occupied: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.map.len()
    }

    /// Grow storage to at least the firmware-reported requirement. The
    /// previously occupied map prefix is retired: after a grow the buffer
    /// holds no live map until a later acquisition succeeds.
    pub fn grow(&mut self, required_map_bytes: usize) {
        if self.map.len() < required_map_bytes {
            self.map.resize(required_map_bytes, 0);
        }
        self.occupied = 0;
    }

    /// The byte prefix firmware reported written by the last admitted
    /// successful acquisition; empty before any success and after `grow`.
    pub fn occupied_bytes(&self) -> &[u8] {
        &self.map[..self.occupied]
    }

    pub const fn occupied_map_bytes(&self) -> usize {
        self.occupied
    }
}

impl Default for UefiMemoryMapBuffer {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl std::fmt::Debug for UefiMemoryMapBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiMemoryMapBuffer")
            .field("capacity", &self.capacity())
            .field("occupied", &self.occupied)
            .finish_non_exhaustive()
    }
}

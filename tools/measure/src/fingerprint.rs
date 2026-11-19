//! A hash of a layer's output, for showing that two builds produced the same thing.

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A running 64-bit FNV-1a hash.
///
/// A tool mixes in everything that makes up its layer's output, in order. Two builds that print
/// the same fingerprint for the same input produced the same output, which is how a change meant
/// only to be faster is shown to have changed nothing else. This is a check against accidents,
/// not against anyone trying to fool it.
pub struct Fingerprint {
    hash: u64,
}

impl Fingerprint {
    pub fn new() -> Self {
        Self { hash: FNV_OFFSET_BASIS }
    }

    /// Mixes in one value, low byte first.
    pub fn mix(
        &mut self,
        mixed_value: u64,
    ) {
        for mixed_byte in mixed_value.to_le_bytes() {
            self.hash ^= u64::from(mixed_byte);
            self.hash = self.hash.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn get_value(&self) -> u64 {
        self.hash
    }

    /// Sixteen lowercase hexadecimal digits, the form the tools print and the logs quote.
    pub fn get_hex_text(&self) -> String {
        format!("{:016x}", self.hash)
    }
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::fingerprint::Fingerprint;

    #[test]
    fn nothing_mixed_in_is_the_fnv_offset_basis() {
        assert_eq!(Fingerprint::new().get_hex_text(), "cbf29ce484222325");
    }

    #[test]
    fn the_same_values_in_the_same_order_give_the_same_fingerprint() {
        let mut first_fingerprint = Fingerprint::new();
        let mut second_fingerprint = Fingerprint::new();

        for mixed_value in [3u64, 0, 17, u64::MAX] {
            first_fingerprint.mix(mixed_value);
            second_fingerprint.mix(mixed_value);
        }

        assert_eq!(first_fingerprint.get_value(), second_fingerprint.get_value());
    }

    #[test]
    fn order_changes_the_fingerprint() {
        let mut forward_fingerprint = Fingerprint::new();
        let mut backward_fingerprint = Fingerprint::new();

        forward_fingerprint.mix(1);
        forward_fingerprint.mix(2);
        backward_fingerprint.mix(2);
        backward_fingerprint.mix(1);

        assert_ne!(forward_fingerprint.get_value(), backward_fingerprint.get_value());
    }
}

use psi_checked_trees::name::Identifier;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendStringStorage {
    pub identity_strings: usize,
    pub identity_bytes: usize,
    pub payload_strings: usize,
    pub payload_bytes: usize,
    pub generated_symbol_strings: usize,
    pub generated_symbol_bytes: usize,
    pub report_strings: usize,
    pub report_bytes: usize,
}

impl BackendStringStorage {
    pub fn total_strings(self) -> usize {
        self.identity_strings
            + self.payload_strings
            + self.generated_symbol_strings
            + self.report_strings
    }

    pub fn total_bytes(self) -> usize {
        self.identity_bytes + self.payload_bytes + self.generated_symbol_bytes + self.report_bytes
    }

    pub(in crate::identity) fn count_identity(&mut self, value: &str) {
        count_string(&mut self.identity_strings, &mut self.identity_bytes, value);
    }

    pub(in crate::identity) fn count_payload(&mut self, value: &[u8]) {
        self.payload_strings += 1;
        self.payload_bytes += value.len();
    }

    pub(in crate::identity) fn count_generated_symbol(&mut self, value: &str) {
        count_string(
            &mut self.generated_symbol_strings,
            &mut self.generated_symbol_bytes,
            value,
        );
    }

    pub(in crate::identity) fn count_report(&mut self, value: &str) {
        count_string(&mut self.report_strings, &mut self.report_bytes, value);
    }

    pub(in crate::identity) fn count_program_name_identity(&mut self, name: &Identifier) {
        if !name.as_str().is_empty() {
            self.count_identity(name.as_str());
        }
    }
}

fn count_string(count: &mut usize, bytes: &mut usize, value: &str) {
    *count += 1;
    *bytes += value.len();
}

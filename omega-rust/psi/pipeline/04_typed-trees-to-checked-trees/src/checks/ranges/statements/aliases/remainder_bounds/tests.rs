use crate::tests::front_end::checked_program_result;

fn accepts(source: &str) {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

fn rejects_index(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("index remains unproved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{source}: {diagnostics:#?}",
    );
}

#[test]
fn captured_unsigned_remainders_bound_full_width_fields_and_saturated_values() {
    accepts(
        r#"
        data Task {
            name: [u8; 256];
            name_len: u64;
            identifier: [u8; 256];
            identifier_len: u64;
            queue: [u8; 64];
            head: u64;
            count: u64;
        }
        machine Task::enqueue(&mut self) {
            let tail: u64 = (((self.head as u64 in Saturating)
                + (self.count as u64 in Saturating)) as u64) % 64;
            self.queue[tail] = 7;
        }
        machine Task::get_name<'a>(&self) -> &'a [u8] {
            let end: u64 = self.name_len % 257;
            self.name[0..end]
        }
        machine Task::get_identifier<'a>(&self) -> &'a [u8] {
            let end: u64 = self.identifier_len % 257;
            self.identifier[0..end]
        }
        "#,
    );
}

#[test]
fn captured_remainder_survives_source_mutation_and_transfers_to_state() {
    accepts(
        r#"
        data Buffer { bytes: [u8; 64]; head: u64; }
        machine Buffer::write(&mut self) {
            let index: u64 = self.head % 64;
            self.head = 18446744073709551615;
            let copied: u64 = index;
            transition { _ -> store(copied) }
            state store(&mut self, position: u64) {
                self.bytes[position] = 7;
            }
        }
        "#,
    );
}

#[test]
fn reassignment_retires_and_reestablishes_captured_remainder_bounds() {
    rejects_index(
        r#"
        machine write(output: &mut [u8; 64], input: u64, unknown: u64) {
            let mut index: u64 = input % 64;
            index = unknown;
            output[index] = 7;
        }
        "#,
    );
    accepts(
        r#"
        machine write(output: &mut [u8; 64], input: u64) {
            let mut index: u64 = input;
            index = input % 64;
            output[index] = 7;
        }
        "#,
    );
}

#[test]
fn signed_or_selected_remainders_do_not_supply_unsigned_bounds() {
    rejects_index(
        r#"
        machine write(output: &mut [u8; 64], input: i64) {
            let index: i64 = input % 64;
            output[index] = 7;
        }
        "#,
    );
    rejects_index(
        r#"
        boundary operator % Meaning::remainder(left: u64, right: u64) -> u64;
        machine write(output: &mut [u8; 64], input: u64) {
            let index: u64 = input % 64;
            output[index] = 7;
        }
        "#,
    );
}

#[test]
fn remainder_bound_must_fit_the_selected_collection() {
    rejects_index(
        r#"
        machine write(output: &mut [u8; 64], input: u64) {
            let index: u64 = input % 65;
            output[index] = 7;
        }
        "#,
    );
}

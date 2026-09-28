//! Fixtures shared by the write-only borrow tests.

mod length_metadata_and_slices;
mod record_path_fixed_byte_arrays;
mod whole_replacement_and_fixed_arrays;
mod write_only_subloans;

use crate::tests::front_end::checked_program_result;

fn rendered_rejection(source: &str) -> String {
    checked_program_result(source)
        .expect_err("source should be rejected")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

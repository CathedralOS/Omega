/*
Semantic judgments for byte-sequence carriers, byte-view operations, and little-endian primitive
byte-window reads and stores.
*/

mod fixed_arrays;

#[cfg(test)]
mod descriptor_predicate_tests;

pub(crate) mod operation_bounds;

pub(crate) mod byte_carriers;
pub(crate) mod byte_sequence_concatenation;
pub(crate) mod byte_sequence_predicates;

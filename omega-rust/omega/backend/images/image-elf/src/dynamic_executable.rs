//! The dynamic lane: a linear chain of 22 stages driven from
//! `image-emission/src/dynamic_elf.rs`, grouped here by phase in chain order.
//!
//! ```text
//!   import_sections    plan_elf_dynamic_link_inputs -> plan_elf_dynamic_sections
//!                      -> serialize_elf_dynamic_sections
//!                      -> plan_elf_dynamic_section_descriptors
//!   procedure_linkage  plan_elf_procedure_linkage_relocations
//!                      -> plan_elf_procedure_linkage_templates
//!                      -> plan_elf_procedure_linkage_section_descriptors
//!   dynamic_table      plan_elf_dynamic_tags -> serialize_elf_dynamic_table
//!                      -> plan_elf_dynamic_table_section_descriptor
//!   section_headers    plan_elf_section_name_table -> plan_elf_dynamic_section_roster
//!                      -> serialize_elf_section_header_table
//!                      -> plan_elf_indexed_section_payloads
//!                      -> plan_elf_relative_section_payload_layout
//!   load_placement     plan_elf_dynamic_load_layout
//!                      -> apply_elf_section_header_placements
//!                      -> apply_elf_dynamic_address_fixups
//!   file_assembly      serialize_elf_dynamic_file_envelope
//!                      -> apply_elf_procedure_linkage_fixups
//!                      -> assemble_elf_dynamic_file -> admit_elf_dynamic_executable
//! ```
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input: there is no way to call stage N without holding a value that only
//! stage N-1 can construct. The crate root re-exports every stage's entry
//! function, carrier and error.

mod checked;
pub(crate) mod dynamic_table;
pub(crate) mod file_assembly;
pub(crate) mod import_sections;
pub(crate) mod load_placement;
pub(crate) mod procedure_linkage;
pub(crate) mod section_headers;

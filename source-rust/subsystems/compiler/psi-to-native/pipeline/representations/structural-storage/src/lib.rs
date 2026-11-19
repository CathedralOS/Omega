//! Storage questions shared by the native forms. The retained catalog carries
//! structural declarations, view plans and the sequence bounds supplied by input
//! preparation. Its queries resolve projections, joins and backing relationships
//! against those records.

#![forbid(unsafe_code)]

pub mod access_reference_validation;
pub mod address_joins;
pub mod owner_loans;
pub mod storage_projection;
pub mod structural_type_catalog;

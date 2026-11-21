//! Native stages resolve field projections, address joins and loans against retained structural declarations.

#![forbid(unsafe_code)]

pub mod access_reference_validation;
pub mod address_joins;
pub mod owner_loans;
pub mod storage_projection;

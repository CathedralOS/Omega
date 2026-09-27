//! Type-reference syntax: `parse_type` reads a type reference, retaining
//! value arguments that need semantic admission later, and `properties` the
//! property brackets on a type.

pub(crate) mod parse_type;
pub(crate) mod properties;
#[cfg(test)]
mod qualified_names;
#[cfg(test)]
mod remainder_tests;

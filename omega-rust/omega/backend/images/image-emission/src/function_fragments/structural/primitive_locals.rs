//! Publication joins for initialized primitive activation storage.
mod access;
mod establishment;
mod frame_location;

pub(in crate::function_fragments) use access::operation_retained;
pub(super) use frame_location::location;

#[cfg(test)]
mod tests;

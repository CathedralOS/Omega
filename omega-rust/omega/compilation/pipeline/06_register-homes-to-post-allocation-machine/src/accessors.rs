//! One owner for the carried-field accessors.
//!
//! A carrier lists its retained fields and the shape each getter returns; the
//! getter body is written once here instead of once per field per carrier.
//! Slice views and computed getters stay explicit on their carrier.

/// `pub const fn field(&self) -> T { self.field }` for every `Copy` field.
macro_rules! copied_accessors {
    ($($(#[$meta:meta])* $name:ident : $ty:ty),* $(,)?) => {
        $(
            $(#[$meta])*
            pub const fn $name(&self) -> $ty {
                self.$name
            }
        )*
    };
}

/// `pub const fn field(&self) -> &T { &self.field }` for every borrowed field.
macro_rules! borrowed_accessors {
    ($($(#[$meta:meta])* $name:ident : $ty:ty),* $(,)?) => {
        $(
            $(#[$meta])*
            pub const fn $name(&self) -> &$ty {
                &self.$name
            }
        )*
    };
}

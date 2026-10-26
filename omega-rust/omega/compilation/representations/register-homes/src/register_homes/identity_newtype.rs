//! The identity newtype every artifact identity in this crate family declares.
//!
//! Each artifact's identity is a 32-byte digest newtype whose declaration is
//! its whole definition: the same derives and the same `from_bytes`/`bytes`
//! accessors. Restating those lines per artifact is what drifts — a member
//! that grows a second accessor or drops a derive keeps compiling while the
//! family no longer has one surface. The declarative row below is that
//! surface; a member writes its own name and field visibility, and nothing
//! else. `machine-code` artifacts declare theirs through the same row.
#[macro_export]
macro_rules! identity_newtype {
    ($(#[$meta:meta])* $vis:vis struct $name:ident($field_vis:vis [u8; 32]);) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis struct $name($field_vis [u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

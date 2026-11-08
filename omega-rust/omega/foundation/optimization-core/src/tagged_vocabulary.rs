//! Shared closed-vocabulary expansions: one row per variant states its wire
//! tag and canonical (declaration) order; a labelled vocabulary also states
//! the stable name reports print. The enum, its `ALL` row, and the tag
//! reading are generated from that single row list.

/// Expands one closed, wire-tagged vocabulary from its single row list.
///
/// `tagged_vocabulary!` keeps the variant rows: their declaration order is the
/// canonical order, so the generated `ALL` row cannot drop a variant the enum
/// declares while a hand-written array could. The labelled form adds the
/// stable name projection from the same rows.
///
/// A row list that needs a tag reading adds [`tag_from_all!`], which walks the
/// generated `ALL` and compares the same declared tags.
#[macro_export]
macro_rules! tagged_vocabulary {
    (
        $count:literal;
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident = $tag:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(u8)]
        $vis enum $name {
            $($variant = $tag),+
        }

        impl $name {
            /// Canonical declaration order.
            pub const ALL: [Self; $count] = [$(Self::$variant),+];
        }
    };
    (
        $count:literal;
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident = $tag:literal => $label:literal),+ $(,)?
        }
    ) => {
        $crate::tagged_vocabulary! {
            $count;
            $(#[$meta])*
            $vis enum $name {
                $($variant = $tag),+
            }
        }

        impl $name {
            /// The stable name reports and manifests print.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

/// Expands the tag reading of one vocabulary over its generated `ALL` row.
///
/// The reading walks `ALL` in canonical order and compares the declared tags,
/// so a tag the row list does not declare refuses through the caller's own
/// unknown-tag constructor — the same owner pattern the exact-name
/// optimization vocabulary uses.
#[macro_export]
macro_rules! tag_from_all {
    ($name:ty, $error:ty => $unknown:path) => {
        impl $name {
            fn from_tag(tag: u8) -> Result<Self, $error> {
                Self::ALL
                    .into_iter()
                    .find(|variant| *variant as u8 == tag)
                    .ok_or($unknown(tag))
            }
        }
    };
}

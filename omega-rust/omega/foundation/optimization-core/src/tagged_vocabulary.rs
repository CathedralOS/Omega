// Closed wire-tagged enum macros (`tagged_vocabulary!`, `tag_from_all!`).
//
// `tagged_vocabulary!` declares a `#[repr(u8)]` enum alongside its canonical
// declaration-order `ALL` array (and optional `name(self) -> &'static str`
// projection) from one variant list so `ALL` can never omit a declared variant.
// `tag_from_all!` generates `from_tag(u8)` by scanning `Self::ALL`.

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
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

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

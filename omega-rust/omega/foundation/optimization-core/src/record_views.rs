//! Shared record-view expansion for encapsulated records: one row per exported
//! field states the field and the type its accessor reads as, and the record's
//! views — plus, in the `new;` form, its `const` constructor — are generated
//! from that single row list.

/// Expands one encapsulated record's exported views from its field rows, then
/// leaves the rest of the record (validation in `new`, codecs) to its owner.
///
/// Rows are grouped by accessor form. `copied` rows take `self` by value on a
/// `Copy` record; `shared` rows take `&self`; `slice` rows name the element
/// type of the record's own `Vec` and read it as a slice. The leading `new;`
/// marker also generates the record's `const` constructor from the `copied`
/// rows — valid where construction is a pure field restatement, so the marker
/// stays off a record whose `new` validates.
///
/// Rejected alternatives: one accessor macro per form (three invocations per
/// record, each able to state a different field set), and generating the
/// struct declaration from these rows (a `slice` row names the element type,
/// not the record's own `Vec` carrier, so the declaration is not derivable
/// from them).
#[macro_export]
macro_rules! record_views {
    (
        record $name:ident; new;
        copied { $($cf:ident : $ct:ty),+ $(,)? }
        $(shared { $($sf:ident : $st:ty),+ $(,)? })?
        $(slice { $($lf:ident : $le:ty),+ $(,)? })?
    ) => {
        impl $name {
            pub const fn new($($cf: $ct),+) -> Self {
                Self { $($cf),+ }
            }

            $crate::record_views!(@copied $($cf : $ct),+);
            $($crate::record_views!(@shared $($sf : $st),+);)?
            $($crate::record_views!(@slice $($lf : $le),+);)?
        }
    };
    (
        record $name:ident;
        $(copied { $($cf:ident : $ct:ty),+ $(,)? })?
        $(shared { $($sf:ident : $st:ty),+ $(,)? })?
        $(slice { $($lf:ident : $le:ty),+ $(,)? })?
    ) => {
        impl $name {
            $($crate::record_views!(@copied $($cf : $ct),+);)?
            $($crate::record_views!(@shared $($sf : $st),+);)?
            $($crate::record_views!(@slice $($lf : $le),+);)?
        }
    };
    (@copied $($field:ident : $ty:ty),+ $(,)?) => {
        $(pub const fn $field(self) -> $ty {
            self.$field
        })+
    };
    (@shared $($field:ident : $ty:ty),+ $(,)?) => {
        $(pub const fn $field(&self) -> $ty {
            self.$field
        })+
    };
    (@slice $($field:ident : $element:ty),+ $(,)?) => {
        $(pub fn $field(&self) -> &[$element] {
            &self.$field
        })+
    };
}

// `record_views!`: generates read-only field accessors (and optional `const fn new`)
// for encapsulated record structs from a single grouped field list.
//
// Accessor groups:
// - `copied`: takes `self` by value on a `Copy` record and returns the field by value.
// - `shared`: takes `&self` and returns a `Copy` field by value.
// - `borrowed`: takes `&self` and returns `&T` to an owned field.
// - `slice`: takes `&self` and returns `&[Element]` over an owned `Vec<Element>`.
// Adding `new;` after `record $name;` also generates `pub const fn new(...)`
// from the `copied` group when construction requires no runtime validation.

#[macro_export]
macro_rules! record_views {
    (
        record $name:ident; new;
        copied { $($cf:ident : $ct:ty),+ $(,)? }
        $(shared { $($sf:ident : $st:ty),+ $(,)? })?
        $(borrowed { $($bf:ident : $bt:ty),+ $(,)? })?
        $(slice { $($lf:ident : $le:ty),+ $(,)? })?
    ) => {
        impl $name {
            pub const fn new($($cf: $ct),+) -> Self {
                Self { $($cf),+ }
            }

            $crate::record_views!(@copied $($cf : $ct),+);
            $($crate::record_views!(@shared $($sf : $st),+);)?
            $($crate::record_views!(@borrowed $($bf : $bt),+);)?
            $($crate::record_views!(@slice $($lf : $le),+);)?
        }
    };
    (
        record $name:ident;
        $(copied { $($cf:ident : $ct:ty),+ $(,)? })?
        $(shared { $($sf:ident : $st:ty),+ $(,)? })?
        $(borrowed { $($bf:ident : $bt:ty),+ $(,)? })?
        $(slice { $($lf:ident : $le:ty),+ $(,)? })?
    ) => {
        impl $name {
            $($crate::record_views!(@copied $($cf : $ct),+);)?
            $($crate::record_views!(@shared $($sf : $st),+);)?
            $($crate::record_views!(@borrowed $($bf : $bt),+);)?
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
    (@borrowed $($field:ident : $ty:ty),+ $(,)?) => {
        $(pub const fn $field(&self) -> &$ty {
            &self.$field
        })+
    };
    (@slice $($field:ident : $element:ty),+ $(,)?) => {
        $(pub fn $field(&self) -> &[$element] {
            &self.$field
        })+
    };
}

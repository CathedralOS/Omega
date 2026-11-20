macro_rules! typed_tree_table {
    ($( $field:ident => $get:ident ($ty:ty) $( [ $push:ident ] )? $(, $inline:ident)? ;)*) => {
        $(
            $( pub fn $push(&mut self, value: $ty) {
                self.tables
                    .$field
                    .append_to_span(&mut self.roots.$field, value);
            } )?

            $( #[ $inline ] )?
            pub fn $get(&self) -> arena::RootRows<'_, $ty> {
                self.tables
                    .$field
                    .span_or_empty(self.roots.$field)
            }
        )*
    };
}
pub(crate) use typed_tree_table;

macro_rules! declaration_columns {
    () => {};
    (@column $(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty)
        -> $rows:ty, $ty:ty = $column:ident.$span:ident $( [ $push:ident ] )? ) => {
        $(#[$attr])*
        pub fn $name(&self, $node: &$node_ty) -> $rows {
            self.$column.span_or_empty($node.$span)
        }
        $(
            pub fn $push(&mut self, $node: &mut $node_ty, value: $ty) {
                self.$column.append_to_span(&mut $node.$span, value);
            }
        )?
    };

    ($(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty) -> rows[$ty:ty]
        = $column:ident.$span:ident $( [ $push:ident ] )? ; $($rest:tt)*) => {
            $crate::source_state::declaration_columns!(@column $(#[$attr])* $name($node: $node_ty)
                -> arena::RootRows<'_, $ty>, $ty = $column.$span $( [ $push ] )?);
            $crate::source_state::declaration_columns!($($rest)*);
    };

    ($(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty) -> &[$ty:ty]
        = $column:ident.$span:ident $( [ $push:ident ] )? ; $($rest:tt)*) => {
            $crate::source_state::declaration_columns!(@column $(#[$attr])* $name($node: $node_ty)
                -> &[$ty], $ty = $column.$span $( [ $push ] )?);
            $crate::source_state::declaration_columns!($($rest)*);
    };

    ($( append $push:ident($node:ident: $node_ty:ty, $ty:ty)
        = $column:ident.$span:ident ; )*) => {
        $(
            pub fn $push(&mut self, $node: &mut $node_ty, value: $ty) {
                self.$column.append_to_span(&mut $node.$span, value);
            }
        )*
    };
}
pub(crate) use declaration_columns;

macro_rules! span_columns {
    ($( $arena:ident($ty:ty)
        reserve $reserve:ident($message:literal)
        set $set:ident
        $( push $push:ident )? ; )*) => {
        $(
            pub fn $reserve(&mut self, count: u32) -> HandleSpan<$ty> {
                self.$arena.insert_many(
                    std::iter::repeat_with(<$ty as Default>::default)
                        .take(usize::try_from(count).expect($message)),
                )
            }

            pub fn $set(&mut self, span: HandleSpan<$ty>, offset: u32, value: $ty) {
                *self.$arena.get_mut(span.handle_at(offset)) = value;
            }

            $(
                pub fn $push(&mut self, span: &mut HandleSpan<$ty>, value: $ty) {
                    self.$arena.append_to_span(span, value);
                }
            )?
        )*
    };
}
pub(crate) use span_columns;

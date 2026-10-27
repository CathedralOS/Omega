//! One owner for a syntax table's paged-storage roster.
//!
//! Every table keeps a fixed roster of paged arenas (and, for the composite
//! tables, nested rosters). The append checkpoint, its rollback, page
//! sharing, adoption between generations and page opening are the same walk
//! over that roster five times; `paged_roster!` writes the roster once and
//! derives all of them, so a table adds an arena by naming it.

/// Declare a paged-storage roster: the table struct, its append checkpoint and
/// the five roster walks. `arena` members are paged arenas; `roster` members
/// are nested rosters written the same way, whose walks this table delegates.
macro_rules! paged_roster {
    (
        $(#[$table_meta:meta])*
        $table_vis:vis struct $table:ident => $checkpoint_vis:vis $checkpoint:ident {
            $( arena $field:ident: $ty:ty, )*
            $( roster $nested:ident: $nested_ty:ty => $nested_checkpoint:ty, )*
        }
    ) => {
        $(#[$table_meta])*
        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
        $table_vis struct $table {
            $( $field: arena::PagedArenaSet<$ty>, )*
            $( $nested: $nested_ty, )*
        }

        $checkpoint_vis struct $checkpoint {
            $( $field: arena::PagedArenaFrontier<$ty>, )*
            $( $nested: $nested_checkpoint, )*
        }

        impl $table {
            pub(crate) fn append_checkpoint(&self) -> $checkpoint {
                $checkpoint {
                    $( $field: self.$field.frontier(), )*
                    $( $nested: self.$nested.append_checkpoint(), )*
                }
            }

            pub(crate) fn rewind_appended(&mut self, checkpoint: $checkpoint) {
                $( self.$field.rewind_appended(checkpoint.$field); )*
                $( self.$nested.rewind_appended(checkpoint.$nested); )*
            }

            pub(crate) fn share_pages(&mut self) {
                $( self.$field.share_pages(); )*
                $( self.$nested.share_pages(); )*
            }

            pub(crate) fn new_with_generation(generation: u32) -> Self {
                Self {
                    $( $field: arena::PagedArenaSet::with_generation(generation), )*
                    $( $nested: <$nested_ty>::new_with_generation(generation), )*
                }
            }

            pub(crate) fn adopt(&mut self, other: Self) {
                $( self.$field.adopt(other.$field); )*
                $( self.$nested.adopt(other.$nested); )*
            }

            pub(crate) fn begin_page(&mut self, generation: u32) {
                $( self.$field.begin_page(generation); )*
                $( self.$nested.begin_page(generation); )*
            }
        }
    };
}

pub(crate) use paged_roster;

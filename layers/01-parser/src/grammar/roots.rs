//! The places a parse can start: a whole file, or one of the fragment shapes the spec's examples use.

use crate::fragment_kind::FragmentKind;
use crate::grammar::statements::TailRule;
use crate::grammar::types::TypePosition;
use crate::node_kind::NodeKind;
use crate::parser::{Parsed, Parser};
use omega_lexer::Punctuation;

impl Parser<'_> {
    /// `file = item*`
    pub(crate) fn parse_file_root(&mut self) -> Parsed {
        let file = self.open();

        while !self.at_end() {
            self.parse_item()?;
        }

        self.close(file, NodeKind::File);

        Ok(())
    }

    /// A piece of source that is not a whole file. See `FragmentKind`.
    pub(crate) fn parse_fragment_root(
        &mut self,
        fragment_kind: FragmentKind,
    ) -> Parsed {
        let fragment = self.open();

        match fragment_kind {
            FragmentKind::Statements => {
                while !self.at_end() {
                    if self.at_item() {
                        self.parse_item()?;
                    } else {
                        self.parse_statement(TailRule::Anywhere)?;
                    }
                }
            }
            FragmentKind::Clauses => {
                self.parse_clauses(TypePosition::Plain)?;

                // The `;` that would end a bodyless declaration, when the example shows it.
                self.eat(Punctuation::Semicolon);
            }
            FragmentKind::GenericParam => self.parse_generic_param()?,
        }

        self.close(fragment, NodeKind::Fragment);

        Ok(())
    }
}

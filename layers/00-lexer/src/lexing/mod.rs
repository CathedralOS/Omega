//! The operation itself: the two entry points, the scanner that makes the one pass, the tables it reads, and the escape rules of a quoted literal.

pub mod byte_class;
pub mod lexer;
pub mod quoted_escape_error;
pub mod quoted_literal_decoder;
pub mod token_scanner;

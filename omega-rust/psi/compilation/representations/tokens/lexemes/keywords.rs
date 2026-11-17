//! One keyword vocabulary generates classification and diagnostic spellings.
//! A string match lets the compiler dispatch by length before comparing bytes.

macro_rules! define_keywords {
    ($($spelling:literal => $kind:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum KeywordKind {
            $($kind,)*
            #[default]
            Unknown,
        }
        impl KeywordKind {
            pub fn from_lexeme(lexeme: &str) -> Option<Self> {
                match lexeme {
                    $($spelling => Some(Self::$kind),)*
                    _ => None,
                }
            }
            pub fn lexeme(self) -> &'static str {
                match self {
                    $(Self::$kind => $spelling,)*
                    Self::Unknown => "<unknown>",
                }
            }
            pub fn is_strict_identifier_keyword(self) -> bool {
                matches!(self, Self::SelfValue)
            }
        }
    };
}

define_keywords! {
    "as" => As,
    "calling_convention" => CallingConvention,
    "capability" => Capability,
    "contains" => Contains,
    "data" => Data,
    "else" => Else,
    "false" => False,
    "foreign" => Foreign,
    "host" => Host,
    "if" => If,
    "let" => Let,
    "library" => Library,
    "loop" => Loop,
    "machine" => Machine,
    "match" => Match,
    "owns" => Owns,
    "platform" => Platform,
    "public" => Public,
    "return" => Return,
    "self" => SelfValue,
    "state" => State,
    "struct" => Struct,
    "target" => Target,
    "transition" => Transition,
    "true" => True,
    "use" => Use,
    "when" => When,
    "while" => While,
}

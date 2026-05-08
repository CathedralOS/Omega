use std::fmt;
use std::ops::Deref;

use omega_core::source::SourceSpan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identifier {
    text: String,
    source_span: SourceSpan,
}

impl Identifier {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: text.into(),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn into_string(self) -> String {
        self.text
    }
}

impl Deref for Identifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for Identifier {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for Identifier {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Identifier> for &str {
    fn eq(&self, other: &Identifier) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Identifier> for str {
    fn eq(&self, other: &Identifier) -> bool {
        self == other.as_str()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentifierPath {
    members: Vec<Identifier>,
}

impl IdentifierPath {
    pub fn new(members: Vec<Identifier>) -> Self {
        Self { members }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Identifier> {
        self.members.iter()
    }

    pub fn first(&self) -> Option<&Identifier> {
        self.members.first()
    }

    pub fn join(&self, separator: &str) -> String {
        self.members
            .iter()
            .map(Identifier::as_str)
            .collect::<Vec<_>>()
            .join(separator)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

impl From<Vec<Identifier>> for IdentifierPath {
    fn from(members: Vec<Identifier>) -> Self {
        Self::new(members)
    }
}

impl<'path> IntoIterator for &'path IdentifierPath {
    type Item = &'path Identifier;
    type IntoIter = std::slice::Iter<'path, Identifier>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
    }
}

use std::fmt;
use std::ops::Deref;

use omega_core::source::SourceSpan;

#[derive(Clone, Default, Eq)]
pub struct Identifier {
    text: IdentifierText,
    source_span: SourceSpan,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum IdentifierText {
    #[default]
    Missing,
    Owned(String),
}

impl Identifier {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: IdentifierText::Owned(text.into()),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: IdentifierText::Owned(text.into()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            IdentifierText::Missing => "",
            IdentifierText::Owned(text) => text.as_str(),
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn is_source_backed(&self) -> bool {
        self.source_span.span.start != self.source_span.span.end
    }

    pub fn into_string(self) -> String {
        self.as_str().to_owned()
    }
}

impl From<&str> for Identifier {
    fn from(text: &str) -> Self {
        Self::generated(text)
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

impl fmt::Debug for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Identifier")
            .field("text", &self.as_str())
            .field("source_span", &self.source_span)
            .finish()
    }
}

impl PartialEq for Identifier {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
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

    pub fn push(&mut self, member: Identifier) {
        self.members.push(member);
    }

    pub fn as_slice(&self) -> &[Identifier] {
        &self.members
    }

    pub fn join(&self, separator: &str) -> String {
        let byte_count = self
            .members
            .iter()
            .map(|member| member.as_str().len())
            .sum::<usize>()
            + separator
                .len()
                .saturating_mul(self.members.len().saturating_sub(1));
        let mut joined = String::with_capacity(byte_count);

        for (index, member) in self.members.iter().enumerate() {
            if index > 0 {
                joined.push_str(separator);
            }

            joined.push_str(member.as_str());
        }

        joined
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

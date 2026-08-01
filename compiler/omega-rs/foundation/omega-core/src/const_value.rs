//! Canonical, compiler-generated const-generic value atoms.
//!
//! Generic arguments historically carry integer values as unnameable decimal
//! `Named` leaves. Structured const values use the same erased carrier, but a
//! reserved length-delimited atom keeps them disjoint from source-spellable
//! type names. The canonical encoding, not the source expression or evaluation
//! trace, is semantic identity. `display` is canonical diagnostic text and is
//! included in the atom only so every downstream tree can render the value
//! without retaining the pre-resolution expression arena.

const PREFIX: &str = "#omega-const:";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalConstValue {
    pub type_name: String,
    pub encoding: String,
    pub display: String,
}

impl CanonicalConstValue {
    pub fn new(
        type_name: impl Into<String>,
        encoding: impl Into<String>,
        display: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            encoding: encoding.into(),
            display: display.into(),
        }
    }

    /// The reserved leaf stored in generic argument position. Source
    /// identifiers cannot contain `#`, so no authored type or parameter can
    /// collide with this namespace.
    pub fn atom(&self) -> String {
        format!(
            "{PREFIX}{}:{}{}:{}{}:{}",
            self.type_name.len(),
            self.type_name,
            self.encoding.len(),
            self.encoding,
            self.display.len(),
            self.display,
        )
    }

    pub fn from_atom(atom: &str) -> Option<Self> {
        let mut rest = atom.strip_prefix(PREFIX)?;
        let type_name = take_length_delimited(&mut rest)?;
        let encoding = take_length_delimited(&mut rest)?;
        let display = take_length_delimited(&mut rest)?;
        if !rest.is_empty() {
            return None;
        }
        Some(Self::new(type_name, encoding, display))
    }
}

fn take_length_delimited(rest: &mut &str) -> Option<String> {
    let separator = rest.find(':')?;
    let length = rest[..separator].parse::<usize>().ok()?;
    let payload = &rest[separator + 1..];
    if !payload.is_char_boundary(length) || payload.len() < length {
        return None;
    }
    let (value, tail) = payload.split_at(length);
    *rest = tail;
    Some(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::CanonicalConstValue;

    #[test]
    fn canonical_const_atom_round_trips_delimiters_and_unicode() {
        let value =
            CanonicalConstValue::new("pkg::Unit", "record(2:a=1:b)", "Unit { symbol: \"μ:m\" }");
        assert_eq!(CanonicalConstValue::from_atom(&value.atom()), Some(value));
    }

    #[test]
    fn ordinary_names_are_not_const_atoms() {
        assert!(CanonicalConstValue::from_atom("Unit").is_none());
        assert!(CanonicalConstValue::from_atom("42").is_none());
    }
}

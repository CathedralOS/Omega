/*
A primitive byte predicate (`valid_utf8`, `ascii_only`, `no_nul`, `non_empty`)
of a fixed byte array, read as the clause it means over the array's bytes.

`domain [u8; 3]::Utf8 requires valid_utf8(self);` puts a domain on the same
3-byte fixed array as `[u8; 3]` (domains.md: "`[u8; N] in D` is the same exact
`N`-byte fixed-array carrier as `[u8; N]`"). Its length is in the type, so the
predicate is a statement about `self[0]` .. `self[N - 1]`, and this reader
states exactly that statement. The checker then proves and assumes it like any
other data clause: a cast proves it from what is known about each byte, and a
value that holds the domain supplies it to its readers.

The statement is equivalent to the predicate, never stronger or weaker: a
reader that assumes it learns no more than the predicate says, and a cast that
proves it establishes nothing the bytes do not show.
*/

use semantic_vocabulary::{
    ByteSequencePredicate, CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue,
    MachineId, PlaceId, Proposition, ScalarTerm, ScalarType,
};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use super::authority::Access;
use crate::body::ValueType;
use crate::canonical::{self, Repeated};
use crate::{Unserved, refuse};

/// The bytes of one fixed array, by position, and how to write a statement
/// about them in canonical form.
struct Bytes {
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    length: u64,
    id: MachineId,
}

fn byte() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is an integer type")
}

impl Bytes {
    fn at(&self, position: u64) -> ScalarTerm {
        let mut path = self.path.clone();
        path.push(CanonicalStructuralPathSegment::FixedIndex(position));
        ScalarTerm::IntegerField {
            version: semantic_vocabulary::ContentPlaceVersion::Current,
            root: self.root,
            path,
            scalar_type: byte(),
        }
    }

    fn literal(value: u8) -> ScalarTerm {
        ScalarTerm::Integer {
            scalar_type: byte(),
            value: IntegerValue::Unsigned(u128::from(value)),
        }
    }

    /// `low <= self[position] <= high`, leaving out a bound every byte meets.
    /// A position past the end is no byte, so nothing is in any range there.
    fn within(&self, position: u64, low: u8, high: u8) -> Checked<Proposition> {
        if position >= self.length {
            return Ok(Proposition::Falsehood);
        }
        let byte = self.at(position);
        if low == high {
            return canonical::equal(byte, Self::literal(low), self.id);
        }
        let mut bounds = Vec::with_capacity(2);
        if low > u8::MIN {
            bounds.push(Proposition::LessOrEqual(Self::literal(low), byte.clone()));
        }
        if high < u8::MAX {
            bounds.push(Proposition::LessOrEqual(byte, Self::literal(high)));
        }
        canonical::conjunction(bounds, self.id)
    }

    fn all(&self, conjuncts: Vec<Proposition>) -> Checked<Proposition> {
        if conjuncts.contains(&Proposition::Falsehood) {
            return Ok(Proposition::Falsehood);
        }
        canonical::conjunction(
            conjuncts
                .into_iter()
                .filter(|conjunct| *conjunct != Proposition::Truth)
                .collect(),
            self.id,
        )
    }

    fn any(&self, disjuncts: Vec<Proposition>) -> Checked<Proposition> {
        if disjuncts.contains(&Proposition::Truth) {
            return Ok(Proposition::Truth);
        }
        let mut open: Vec<_> = disjuncts
            .into_iter()
            .filter(|disjunct| *disjunct != Proposition::Falsehood)
            .collect();
        match open.len() {
            0 => Ok(Proposition::Falsehood),
            1 => Ok(open.remove(0)),
            _ => canonical::clauses(open, Repeated::Merge, self.id).map(Proposition::Disjunction),
        }
    }

    fn implies(premise: Proposition, conclusion: Proposition) -> Proposition {
        if premise == Proposition::Falsehood || conclusion == Proposition::Truth {
            return Proposition::Truth;
        }
        Proposition::Implication {
            premise: Box::new(premise),
            conclusion: Box::new(conclusion),
        }
    }

    fn continuation(&self, position: u64) -> Checked<Proposition> {
        self.within(position, 0x80, 0xBF)
    }

    /// Every byte at `start .. start + count` is a continuation byte.
    fn continuations(&self, start: u64, count: u64) -> Checked<Proposition> {
        let mut bytes = Vec::with_capacity(count as usize);
        for position in start..start + count {
            bytes.push(self.continuation(position)?);
        }
        self.all(bytes)
    }

    /// The bytes are well-formed UTF-8 (the Unicode Standard, table 3-7).
    /// Each byte is held to a rule that reads at most three bytes on either
    /// side of it, and together the rules say exactly what the table says:
    /// - a byte is ASCII, a continuation, or a lead in C2..F4;
    /// - a lead is followed by the continuations its sequence needs, with the
    ///   narrower second byte after E0, ED, F0 and F4;
    /// - a continuation lies within the span of the lead before it.
    ///
    /// A lead's span holds only continuations, so no two spans overlap, and
    /// every continuation lies in one: the bytes split into whole sequences.
    fn valid_utf8(&self) -> Checked<Proposition> {
        let mut rules = Vec::new();
        for i in 0..self.length {
            rules.push(self.any(vec![
                self.within(i, 0x00, 0xBF)?,
                self.within(i, 0xC2, 0xF4)?,
            ])?);
            // Two-byte sequences.
            rules.push(Self::implies(
                self.within(i, 0xC2, 0xDF)?,
                self.continuation(i + 1)?,
            ));
            // Three-byte sequences, without overlong forms or surrogates.
            rules.push(Self::implies(
                self.within(i, 0xE0, 0xEF)?,
                self.continuations(i + 1, 2)?,
            ));
            rules.push(Self::implies(
                self.within(i, 0xE0, 0xE0)?,
                self.within(i + 1, 0xA0, 0xBF)?,
            ));
            rules.push(Self::implies(
                self.within(i, 0xED, 0xED)?,
                self.within(i + 1, 0x80, 0x9F)?,
            ));
            // Four-byte sequences, without overlong forms or values past
            // U+10FFFF.
            rules.push(Self::implies(
                self.within(i, 0xF0, 0xF4)?,
                self.continuations(i + 1, 3)?,
            ));
            rules.push(Self::implies(
                self.within(i, 0xF0, 0xF0)?,
                self.within(i + 1, 0x90, 0xBF)?,
            ));
            rules.push(Self::implies(
                self.within(i, 0xF4, 0xF4)?,
                self.within(i + 1, 0x80, 0x8F)?,
            ));
            // A continuation follows a lead at most three bytes back, with
            // only continuations between.
            let mut claimed = Vec::with_capacity(3);
            if i >= 1 {
                claimed.push(self.within(i - 1, 0xC2, 0xF4)?);
            }
            if i >= 2 {
                claimed.push(self.all(vec![
                    self.within(i - 2, 0xE0, 0xF4)?,
                    self.continuation(i - 1)?,
                ])?);
            }
            if i >= 3 {
                claimed.push(self.all(vec![
                    self.within(i - 3, 0xF0, 0xF4)?,
                    self.continuations(i - 2, 2)?,
                ])?);
            }
            rules.push(Self::implies(self.continuation(i)?, self.any(claimed)?));
        }
        self.all(rules)
    }

    fn proposition(&self, predicate: ByteSequencePredicate) -> Checked<Proposition> {
        match predicate {
            ByteSequencePredicate::AsciiOnly => self.all(
                (0..self.length)
                    .map(|i| self.within(i, 0x00, 0x7F))
                    .collect::<Checked<_>>()?,
            ),
            ByteSequencePredicate::NoNul => self.all(
                (0..self.length)
                    .map(|i| self.within(i, 0x01, 0xFF))
                    .collect::<Checked<_>>()?,
            ),
            ByteSequencePredicate::NonEmpty => Ok(if self.length > 0 {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }),
            ByteSequencePredicate::ValidUtf8 => self.valid_utf8(),
        }
    }
}

impl Program<'_> {
    /// `valid_utf8(self)` and its kin of a fixed array at `root`/`path`:
    /// the clause over that array's bytes. An array whose elements are not
    /// `u8` is refused.
    pub(super) fn fixed_byte_predicate_clause(
        &self,
        id: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        value_type: ValueType,
        predicate: ByteSequencePredicate,
    ) -> Checked<Proposition> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        let Some(array) = self.opened(id, Some(value_type), Access::Read)? else {
            return Err(unserved(
                "byte predicate of something that is not a byte array",
            ));
        };
        let (element, qualification, terminal_semantics::CollectionExtent::Fixed(length)) =
            self.array_element(id, array)?
        else {
            return Err(unserved(
                "byte predicate of an array without a fixed extent",
            ));
        };
        if !matches!(
            self.storage_value_type(id, element, qualification)?,
            ValueType::Scalar(ScalarType::Integer(element), _) if element == byte()
        ) {
            return Err(unserved(
                "byte predicate of an array whose elements are not u8",
            ));
        }
        Bytes {
            root,
            path,
            length,
            id,
        }
        .proposition(predicate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a statement says of `bytes`, deciding each comparison of a byte
    /// against a literal.
    fn holds(statement: &Proposition, bytes: &[u8]) -> bool {
        let value = |term: &ScalarTerm| match term {
            ScalarTerm::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => *value,
            ScalarTerm::IntegerField { path, .. } => match path.last() {
                Some(CanonicalStructuralPathSegment::FixedIndex(position)) => {
                    u128::from(bytes[*position as usize])
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        match statement {
            Proposition::Truth => true,
            Proposition::Falsehood => false,
            Proposition::Equal(left, right) => value(left) == value(right),
            Proposition::LessOrEqual(left, right) => value(left) <= value(right),
            Proposition::Conjunction(parts) => parts.iter().all(|part| holds(part, bytes)),
            Proposition::Disjunction(parts) => parts.iter().any(|part| holds(part, bytes)),
            Proposition::Implication {
                premise,
                conclusion,
            } => !holds(premise, bytes) || holds(conclusion, bytes),
            other => panic!("{other:?}"),
        }
    }

    fn statement(predicate: ByteSequencePredicate, length: u64) -> Proposition {
        Bytes {
            root: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            length,
            id: MachineId::new(1).unwrap(),
        }
        .proposition(predicate)
        .unwrap()
    }

    /// The statement agrees with the predicate on every 1- and 2-byte array
    /// and on every 3-byte array built from bytes at the class edges, so it
    /// is neither stronger nor weaker than what it states.
    #[test]
    fn each_statement_says_exactly_what_its_predicate_says() {
        let edges = [
            0x00, 0x01, 0x41, 0x7F, 0x80, 0x8F, 0x90, 0x9F, 0xA0, 0xBF, 0xC0, 0xC1, 0xC2, 0xDF,
            0xE0, 0xE1, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF3, 0xF4, 0xF5, 0xFF,
        ];
        for predicate in ByteSequencePredicate::ALL {
            assert!(holds(&statement(predicate, 0), &[]) == predicate.holds_for(&[]));
            let one = statement(predicate, 1);
            let two = statement(predicate, 2);
            for first in 0..=u8::MAX {
                assert_eq!(holds(&one, &[first]), predicate.holds_for(&[first]));
                for second in 0..=u8::MAX {
                    let bytes = [first, second];
                    assert_eq!(
                        holds(&two, &bytes),
                        predicate.holds_for(&bytes),
                        "{bytes:x?}"
                    );
                }
            }
            for length in 3..=4 {
                let long = statement(predicate, length);
                let mut bytes = vec![0u8; length as usize];
                let count = edges.len().pow(length as u32);
                for mut code in 0..count {
                    for byte in bytes.iter_mut() {
                        *byte = edges[code % edges.len()];
                        code /= edges.len();
                    }
                    assert_eq!(
                        holds(&long, &bytes),
                        predicate.holds_for(&bytes),
                        "{bytes:x?}"
                    );
                }
            }
        }
    }
}

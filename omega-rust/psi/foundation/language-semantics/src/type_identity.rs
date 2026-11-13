/*
We canonicalize structural and nominal type identities (`TypeIdentity`),
interned type identity tables (`index`), binary framing (`framing`), grammar
validation (`grammar`), callable display formatting (`callable_display`),
lifetime partitions (`lifetime_partition`), and type vocabulary descriptors
(`vocabulary`).
*/

mod callable_display {
    use std::borrow::Cow;

    pub fn callable_display_path(identity: &str) -> Cow<'_, str> {
        const HEAD: &str = "named-callable(path(";
        let Some(start) = identity.find(HEAD).map(|index| index + HEAD.len()) else {
            return Cow::Borrowed(identity);
        };
        let Some((path, _)) = atom_payload(&identity[start..]) else {
            return Cow::Borrowed(identity);
        };
        let authored = path
            .strip_prefix("nominal(")
            .and_then(|qualified| qualified.split_once(",path("))
            .and_then(|(_, authored)| atom_payload(authored))
            .map(|(authored, _)| authored.into_owned());
        authored.map_or(path, Cow::Owned)
    }

    fn atom_payload(text: &str) -> Option<(Cow<'_, str>, usize)> {
        let mut characters = text.char_indices();
        while let Some((offset, character)) = characters.next() {
            match character {
                '\\' => {
                    characters.next()?;
                }
                ')' => {
                    let raw = &text[..offset];
                    let payload = if raw.contains('\\') {
                        let mut unescaped = String::with_capacity(raw.len());
                        let mut raw_characters = raw.chars();
                        while let Some(character) = raw_characters.next() {
                            unescaped.push(if character == '\\' {
                                raw_characters.next().unwrap_or(character)
                            } else {
                                character
                            });
                        }
                        Cow::Owned(unescaped)
                    } else {
                        Cow::Borrowed(raw)
                    };
                    return Some((payload, offset + 1));
                }
                _ => {}
            }
        }
        None
    }

    #[cfg(test)]
    mod tests {
        use super::callable_display_path as requirement_path;

        const OWNER: &str = "package-owner\\(32:31ed5a8cdac79074acce8823cec1b3e85e1a2a4d42afa87343d6efc3220c7a2b\\)";

        #[test]
        fn requirement_path_names_the_authored_callable() {
            assert_eq!(
                requirement_path(
                    "named-callable(path(Acknowledgement::ack),parameters(parameter\\(x\\)),result-dispatch())"
                ),
                "Acknowledgement::ack"
            );
            assert_eq!(
                requirement_path(
                    "closed-requirement(template(named-callable(path(Parent::read),parameters())))"
                ),
                "Parent::read"
            );
            assert_eq!(requirement_path("opaque"), "opaque");
        }

        #[test]
        fn requirement_path_looks_through_the_nominal_owner() {
            let qualified = format!(
                "named-callable(path(nominal\\({OWNER}\\,path\\(CText::length\\)\\)),parameters(parameter\\(x\\)),result-dispatch())"
            );
            assert_eq!(requirement_path(&qualified), "CText::length");
            for owner in ["toolchain-owner", "unresolved-owner"] {
                let qualified = format!(
                    "named-callable(path(nominal\\({owner}\\,path\\(emit\\)\\)),parameters(),result-dispatch())"
                );
                assert_eq!(requirement_path(&qualified), "emit");
            }
        }

        #[test]
        fn requirement_path_renders_what_it_cannot_unwrap_as_written() {
            assert_eq!(
                requirement_path("named-callable(path(Gui::window_create"),
                "named-callable(path(Gui::window_create"
            );
            assert_eq!(
                requirement_path("named-callable(path(nominal\\(toolchain-owner\\)),parameters())"),
                "nominal(toolchain-owner)"
            );
        }
    }
}
mod framing {
    use super::vocabulary::{Spelled, Vocabulary, is_vocabulary_spelling};
    use super::{Error, Result, TypeIdentityPackageOwnerVisitor};
    use std::borrow::Cow;

    pub(super) struct Reader<'a> {
        text: &'a str,
        position: usize,
        pub(super) callable_regions: bool,
    }

    impl<'a> Reader<'a> {
        pub(super) fn new(text: &'a str) -> Self {
            Self {
                text,
                position: 0,
                callable_regions: false,
            }
        }

        pub(super) fn position(&self) -> usize {
            self.position
        }

        pub(super) fn finish(&self) -> Result {
            if self.position == self.text.len() {
                Ok(())
            } else {
                Err(Error::MalformedIdentity)
            }
        }

        pub(super) fn next(&self, byte: u8) -> bool {
            self.text.as_bytes().get(self.position) == Some(&byte)
        }

        pub(super) fn expect(&mut self, byte: u8) -> Result {
            if !self.next(byte) {
                return Err(Error::MalformedIdentity);
            }
            self.position += 1;
            Ok(())
        }

        fn peek_token(&self) -> Option<&'a str> {
            let rest = &self.text[self.position..];
            let length = rest
                .bytes()
                .take_while(|byte| byte.is_ascii_lowercase() || matches!(byte, b'-' | b'_'))
                .count();
            (length != 0).then(|| &rest[..length])
        }

        pub(super) fn production<F: Vocabulary>(&mut self) -> Result<F> {
            let token = self.peek_token().ok_or(Error::MalformedIdentity)?;
            let form = F::parse(token).ok_or(Error::MalformedIdentity)?;
            self.position += token.len();
            Ok(form)
        }

        pub(super) fn peek<F: Vocabulary>(&self) -> Option<F> {
            self.peek_token().and_then(F::parse)
        }

        pub(super) fn peek_after_comma<F: Vocabulary>(&self) -> Option<F> {
            self.next(b',').then_some(())?;
            Self {
                text: self.text,
                position: self.position + 1,
                callable_regions: self.callable_regions,
            }
            .peek()
        }

        pub(super) fn peek_compound<F: Vocabulary>(&self) -> Option<F> {
            let token = self.peek_token()?;
            self.opens(token).then(|| F::parse(token)).flatten()
        }

        pub(super) fn opens_vocabulary_compound(&self) -> bool {
            self.peek_token()
                .is_some_and(|token| self.opens(token) && is_vocabulary_spelling(token))
        }

        fn opens(&self, token: &str) -> bool {
            self.text.as_bytes().get(self.position + token.len()) == Some(&b'(')
        }

        pub(super) fn expect_tag(&mut self, tag: impl Spelled) -> Result {
            match self.peek_token() {
                Some(token) if token == tag.spelling() => {
                    self.position += token.len();
                    Ok(())
                }
                _ => Err(Error::MalformedIdentity),
            }
        }

        fn raw_value(&mut self) -> Result<(&'a str, usize)> {
            self.expect(b'(')?;
            let start = self.position;
            let mut decoded = 0usize;
            loop {
                match self.text.as_bytes().get(self.position).copied() {
                    Some(b')') => {
                        let value = &self.text[start..self.position];
                        self.position += 1;
                        return Ok((value, decoded));
                    }
                    Some(b'\\') => {
                        self.position += 1;
                        if !matches!(
                            self.text.as_bytes().get(self.position),
                            Some(b'\\' | b'(' | b')' | b',')
                        ) {
                            return Err(Error::MalformedIdentity);
                        }
                    }
                    Some(b'(' | b',') | None => return Err(Error::MalformedIdentity),
                    Some(_) => {}
                }
                self.position += 1;
                decoded += 1;
            }
        }

        pub(super) fn opaque(
            &mut self,
            tag: impl Spelled,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result {
            visitor.enter()?;
            let result = self
                .expect_tag(tag)
                .and_then(|()| self.raw_value())
                .map(|_| ());
            visitor.leave();
            result
        }

        pub(super) fn leaf_opaque(
            &mut self,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result {
            visitor.enter()?;
            let result = self.raw_value().map(|_| ());
            visitor.leave();
            result
        }

        pub(super) fn atom(
            &mut self,
            tag: impl Spelled,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result<Cow<'a, str>> {
            visitor.enter()?;
            let result = self
                .expect_tag(tag)
                .and_then(|()| self.unescaped_value(visitor));
            visitor.leave();
            result
        }

        pub(super) fn leaf_atom(
            &mut self,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result<Cow<'a, str>> {
            visitor.enter()?;
            let result = self.unescaped_value(visitor);
            visitor.leave();
            result
        }

        fn unescaped_value(
            &mut self,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result<Cow<'a, str>> {
            let (raw, decoded) = self.raw_value()?;
            if raw.len() == decoded {
                return Ok(Cow::Borrowed(raw));
            }
            visitor.reserve(decoded)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(decoded)
                .map_err(|_| Error::AllocationFailed)?;
            let mut source = raw.bytes();
            while let Some(byte) = source.next() {
                bytes.push(if byte == b'\\' {
                    source.next().expect("prevalidated escape")
                } else {
                    byte
                });
            }
            Ok(Cow::Owned(
                String::from_utf8(bytes).map_err(|_| Error::MalformedIdentity)?,
            ))
        }

        pub(super) fn leaf_byte_atom(
            &mut self,
            visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        ) -> Result<&'a str> {
            visitor.enter()?;
            let result = self.hexadecimal_value();
            visitor.leave();
            result
        }

        fn hexadecimal_value(&mut self) -> Result<&'a str> {
            let (raw, _) = self.raw_value()?;
            let (length, hexadecimal) = raw.split_once(':').ok_or(Error::MalformedIdentity)?;
            if length.is_empty() || (length.len() > 1 && length.starts_with('0')) {
                return Err(Error::MalformedIdentity);
            }
            let length = length
                .bytes()
                .try_fold(0usize, |value, byte| {
                    if !byte.is_ascii_digit() {
                        return None;
                    }
                    value.checked_mul(10)?.checked_add(usize::from(byte - b'0'))
                })
                .ok_or(Error::MalformedIdentity)?;
            if length.checked_mul(2) != Some(hexadecimal.len())
                || !hexadecimal
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(Error::MalformedIdentity);
            }
            Ok(hexadecimal)
        }
    }
}
mod grammar {
    use super::vocabulary::{
        CallableForm, ConstraintForm, DomainFamily, IndexForm, Label, NameForm, OwnerForm,
        RegionForm, Spelled, TypeForm, Unadmitted, Vocabulary,
    };
    use super::{
        Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, index, scoped, unadmitted,
    };

    pub(super) fn compound<T>(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        children: impl FnOnce(&mut Reader<'_>, &mut dyn TypeIdentityPackageOwnerVisitor) -> Result<T>,
    ) -> Result<T> {
        scoped(visitor, |visitor| {
            reader.expect(b'(')?;
            let value = children(reader, visitor)?;
            reader.expect(b')')?;
            Ok(value)
        })
    }

    pub(super) fn type_identity(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.production::<TypeForm>()? {
            TypeForm::Unit => scoped(visitor, |_| Ok(())),
            TypeForm::Nominal => compound(reader, visitor, nominal),
            TypeForm::CompilerType => compound(reader, visitor, compiler_type),
            TypeForm::Reference | TypeForm::MutableReference | TypeForm::WriteOnlyReference => {
                compound(reader, visitor, |reader, visitor| {
                    type_identity(reader, visitor)?;
                    if reader.next(b',') {
                        if !reader.callable_regions {
                            return Err(region_operand(reader));
                        }
                        reader.expect(b',')?;
                        region(reader, visitor)?;
                    }
                    Ok(())
                })
            }
            TypeForm::Slice => compound(reader, visitor, type_identity),
            TypeForm::Array => compound(reader, visitor, |reader, visitor| {
                type_identity(reader, visitor)?;
                reader.expect(b',')?;
                index::array_length(reader, visitor)
            }),
            TypeForm::Constrained => compound(reader, visitor, |reader, visitor| {
                type_identity(reader, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    constraint(reader, visitor)?;
                }
                Ok(())
            }),
            TypeForm::Generic | TypeForm::DynamicTrait => {
                compound(reader, visitor, |reader, visitor| {
                    name_atom(reader, Label::Name, visitor)?;
                    while reader.next(b',') {
                        reader.expect(b',')?;
                        index::static_argument(reader, visitor)?;
                    }
                    Ok(())
                })
            }
            TypeForm::Named => compound(reader, visitor, |reader, visitor| {
                match reader.peek::<IndexForm>() {
                    Some(IndexForm::IntegerConst) => {
                        reader.production::<IndexForm>()?;
                        reader.leaf_opaque(visitor)
                    }
                    Some(IndexForm::CanonicalConst) => {
                        reader.production::<IndexForm>()?;
                        canonical_const(reader, visitor)
                    }
                    _ => name_atom(reader, Label::Name, visitor),
                }
            }),
            TypeForm::IndexExpression => compound(reader, visitor, index::expression),
            TypeForm::Callable => compound(reader, visitor, |reader, visitor| {
                let previous = reader.callable_regions;
                reader.callable_regions = true;
                let result = callable_child(reader, visitor);
                reader.callable_regions = previous;
                result
            }),
        }
    }

    pub(super) fn callable_child(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let form = reader.production::<CallableForm>()?;
        compound(reader, visitor, |reader, visitor| {
            reader.opaque(Label::Kind, visitor)?;
            match form {
                CallableForm::Record => {
                    while reader.next(b',') {
                        reader.expect(b',')?;
                        callable_child(reader, visitor)?;
                    }
                    Ok(())
                }
                CallableForm::Atom => {
                    reader.expect(b',')?;
                    reader.opaque(Label::Encoding, visitor)
                }
                CallableForm::Name => {
                    reader.expect(b',')?;
                    name_atom(reader, Label::Symbol, visitor)
                }
                CallableForm::Type => {
                    reader.expect(b',')?;
                    type_identity(reader, visitor)
                }
                CallableForm::Index => {
                    reader.expect(b',')?;
                    index::static_argument(reader, visitor)
                }
            }
        })
    }

    pub(super) fn canonical_const(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        compound(reader, visitor, |reader, visitor| {
            if reader.callable_regions {
                let encoded = reader.atom(Label::Type, visitor)?;
                let mut nested = Reader::new(&encoded);
                nested.callable_regions = true;
                type_identity(&mut nested, visitor)?;
                nested.finish()?;
            } else {
                reader.opaque(Label::Type, visitor)?;
            }
            reader.expect(b',')?;
            reader.opaque(Label::Encoding, visitor)
        })
    }

    pub(super) fn region(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.production::<RegionForm>()? {
            RegionForm::Region if reader.callable_regions => {
                compound(reader, visitor, |reader, visitor| {
                    if reader.peek::<Label>() == Some(Label::Owner) {
                        if reader.atom(Label::Owner, visitor)?.as_ref() != "schema" {
                            return Err(unadmitted(Unadmitted::ApplicationScope));
                        }
                    } else {
                        reader.opaque(Label::Kind, visitor)?;
                        reader.expect(b',')?;
                        name_atom(reader, Label::Declaration, visitor)?;
                    }
                    reader.expect(b',')?;
                    let ordinal = reader.atom(Label::Ordinal, visitor)?;
                    if ordinal.is_empty() || !ordinal.bytes().all(|byte| byte.is_ascii_digit()) {
                        return Err(Error::MalformedIdentity);
                    }
                    Ok(())
                })
            }
            RegionForm::Region | RegionForm::Elided | RegionForm::Unresolved => {
                Err(unadmitted(Unadmitted::ApplicationScope))
            }
        }
    }

    fn region_operand(reader: &Reader<'_>) -> Error {
        match reader.peek_after_comma::<RegionForm>() {
            Some(RegionForm::Region | RegionForm::Elided | RegionForm::Unresolved) => {
                unadmitted(Unadmitted::ApplicationScope)
            }
            None => Error::MalformedIdentity,
        }
    }

    pub(super) fn name_atom(
        reader: &mut Reader<'_>,
        tag: impl Spelled,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let name = reader.atom(tag, visitor)?;
        semantic_name(&name, visitor, reader.callable_regions)
    }

    pub(super) fn name_value(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let name = reader.leaf_atom(visitor)?;
        semantic_name(&name, visitor, reader.callable_regions)
    }

    fn semantic_name(
        name: &str,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        callable_regions: bool,
    ) -> Result {
        scoped(visitor, |visitor| {
            let mut reader = Reader::new(name);
            reader.callable_regions = callable_regions;
            let Some(form) = reader.peek_compound::<NameForm>() else {
                if reader.opens_vocabulary_compound() {
                    return Err(Error::MalformedIdentity);
                }
                return visitor.embedded_name(name);
            };
            reader.production::<NameForm>()?;
            match form {
                NameForm::Nominal => compound(&mut reader, visitor, nominal)?,
                NameForm::CompilerType => compound(&mut reader, visitor, compiler_type)?,
                NameForm::DerivedApplication => compound(&mut reader, visitor, type_identity)?,
                NameForm::PlacedField => compound(&mut reader, visitor, placed_field)?,
                NameForm::PlacedOperation => compound(&mut reader, visitor, |reader, visitor| {
                    if reader.production::<NameForm>()? != NameForm::PlacedField {
                        return Err(Error::MalformedIdentity);
                    }
                    compound(reader, visitor, placed_field)?;
                    reader.expect(b',')?;
                    reader.opaque(Label::Operation, visitor)?;
                    reader.expect(b',')?;
                    reader.opaque(Label::Role, visitor)
                })?,
                NameForm::InvalidDerivedOrigin => {
                    return Err(unadmitted(Unadmitted::InvalidIdentity));
                }
            }
            reader.finish()
        })
    }

    fn nominal(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        owner(reader, visitor)?;
        reader.expect(b',')?;
        reader.opaque(Label::Path, visitor)
    }

    fn compiler_type(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        reader.opaque(Label::Atom, visitor)
    }

    fn placed_field(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        type_identity(reader, visitor)?;
        reader.expect(b',')?;
        match reader.peek::<Label>() {
            Some(Label::Member) => reader.opaque(Label::Member, visitor),
            _ => reader.opaque(Label::Field, visitor),
        }
    }

    fn owner(reader: &mut Reader<'_>, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
        match reader.production::<OwnerForm>()? {
            OwnerForm::Package => {
                let hexadecimal = reader.leaf_byte_atom(visitor)?;
                if hexadecimal.len() != 64 {
                    return Err(Error::MalformedIdentity);
                }
                let mut digest = [0u8; 32];
                for (output, pair) in digest
                    .iter_mut()
                    .zip(hexadecimal.as_bytes().as_chunks::<2>().0)
                {
                    let value = |byte| {
                        if byte <= b'9' {
                            byte - b'0'
                        } else {
                            byte - b'a' + 10
                        }
                    };
                    *output = (value(pair[0]) << 4) | value(pair[1]);
                }
                if digest == [0; 32] {
                    return Err(Error::MalformedIdentity);
                }
                visitor.package_owner(digest)
            }
            OwnerForm::ToolchainSource => {
                if reader.leaf_byte_atom(visitor)?.len() != 64 {
                    return Err(Error::MalformedIdentity);
                }
                Ok(())
            }
            OwnerForm::Toolchain | OwnerForm::Unresolved => {
                Err(unadmitted(Unadmitted::InvalidIdentity))
            }
        }
    }

    fn constraint(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.production::<ConstraintForm>()? {
            ConstraintForm::ArithmeticDomain | ConstraintForm::NamedConstraint => {
                compound(reader, visitor, |reader, visitor| {
                    reader.opaque(Label::Name, visitor)
                })
            }
            ConstraintForm::DeclaredDomain => compound(reader, visitor, declared_domain),
            ConstraintForm::CompilerDomain => compound(reader, visitor, compiler_domain),
        }
    }

    fn declared_domain(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let name = reader.atom(Label::Name, visitor)?;
        let mut nested = Reader::new(&name);
        if nested.peek_compound::<ConstraintForm>() == Some(ConstraintForm::DeclaredDomain) {
            constraint(&mut nested, visitor)?;
            nested.finish()?;
        } else {
            semantic_name(&name, visitor, reader.callable_regions)?;
        }
        while reader.next(b',') {
            reader.expect(b',')?;
            type_identity(reader, visitor)?;
        }
        Ok(())
    }

    fn compiler_domain(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let family = reader.atom(Label::Family, visitor)?;
        let family = DomainFamily::parse(&family).ok_or(Error::MalformedIdentity)?;
        reader.expect(b',')?;
        match family {
            DomainFamily::Carry => reader.opaque(Label::Permission, visitor),
            DomainFamily::Value => reader.opaque(Label::Domain, visitor),
            DomainFamily::OmegaLayout => {
                reader.opaque(Label::Grammar, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    reader.expect_tag(Label::Schema)?;
                    compound(reader, visitor, type_identity)?;
                }
                Ok(())
            }
        }
    }
}
mod index {

    use super::grammar::{self, compound};
    use super::vocabulary::{
        ArrayLengthForm, FieldActionForm, IndexForm, Label, PatternForm, RegionForm, TypeForm,
        Unadmitted,
    };
    use super::{
        Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, scoped, unadmitted,
    };
    use numerics::arithmetic::ArithmeticDomain;

    pub(super) fn array_length(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.production::<ArrayLengthForm>()? {
            ArrayLengthForm::Literal => reader.leaf_opaque(visitor),
            ArrayLengthForm::ConstParameter | ArrayLengthForm::ConstCall => {
                grammar::name_value(reader, visitor)
            }
            ArrayLengthForm::ConstExpression => compound(reader, visitor, expression),
            ArrayLengthForm::InvalidExpression => Err(unadmitted(Unadmitted::InvalidIdentity)),
        }
    }

    pub(super) fn expression(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.production::<IndexForm>()? {
            IndexForm::ConstName => grammar::name_value(reader, visitor),
            IndexForm::CanonicalConst => grammar::canonical_const(reader, visitor),
            IndexForm::IntegerConst
            | IndexForm::Integer
            | IndexForm::Boolean
            | IndexForm::Float
            | IndexForm::UndeterminedNan => reader.leaf_opaque(visitor),
            IndexForm::String => reader.leaf_byte_atom(visitor).map(|_| ()),
            IndexForm::BitwiseNot
            | IndexForm::LogicalNot
            | IndexForm::Borrow
            | IndexForm::CaseComponent
            | IndexForm::MutableBorrow
            | IndexForm::WriteOnlyBorrow => compound(reader, visitor, expression),
            IndexForm::Add
            | IndexForm::And
            | IndexForm::BitwiseAnd
            | IndexForm::BitwiseOr
            | IndexForm::BitwiseXor
            | IndexForm::Divide
            | IndexForm::Equal
            | IndexForm::Greater
            | IndexForm::GreaterOrEqual
            | IndexForm::Less
            | IndexForm::LessOrEqual
            | IndexForm::Modulo
            | IndexForm::Multiply
            | IndexForm::NotEqual
            | IndexForm::Or
            | IndexForm::ShiftLeft
            | IndexForm::ShiftRight
            | IndexForm::Subtract
            | IndexForm::CaseMembership => compound(reader, visitor, |reader, visitor| {
                let licensed = reader.peek::<Label>() == Some(Label::Operation);
                if licensed {
                    embedded_operation(reader, visitor)?;
                    reader.expect(b',')?;
                    embedded_algebra(reader, visitor)?;
                    reader.expect(b',')?;
                }
                expression(reader, visitor)?;
                reader.expect(b',')?;
                expression(reader, visitor)?;
                if licensed {
                    while reader.next(b',') {
                        reader.expect(b',')?;
                        expression(reader, visitor)?;
                    }
                }
                Ok(())
            }),
            IndexForm::RuntimeCall => compound(reader, visitor, |reader, visitor| {
                let previous = reader.callable_regions;
                reader.callable_regions = true;
                let result = (|| {
                    expression(reader, visitor)?;
                    reader.expect(b',')?;
                    grammar::type_identity(reader, visitor)?;
                    while reader.next(b',') {
                        reader.expect(b',')?;
                        if reader.peek::<Label>() == Some(Label::StaticArguments) {
                            reader.expect_tag(Label::StaticArguments)?;
                            compound(reader, visitor, |reader, visitor| {
                                static_argument(reader, visitor)?;
                                while reader.next(b',') {
                                    reader.expect(b',')?;
                                    static_argument(reader, visitor)?;
                                }
                                Ok(())
                            })?;
                        } else {
                            expression(reader, visitor)?;
                        }
                    }
                    Ok(())
                })();
                reader.callable_regions = previous;
                result
            }),
            IndexForm::Call => compound(reader, visitor, |reader, visitor| {
                grammar::name_atom(reader, Label::Target, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    if reader.peek::<Label>() == Some(Label::StaticArguments) {
                        reader.expect_tag(Label::StaticArguments)?;
                        compound(reader, visitor, |reader, visitor| {
                            static_argument(reader, visitor)?;
                            while reader.next(b',') {
                                reader.expect(b',')?;
                                static_argument(reader, visitor)?;
                            }
                            Ok(())
                        })?;
                    } else {
                        expression(reader, visitor)?;
                    }
                }
                Ok(())
            }),
            IndexForm::Cast => compound(reader, visitor, |reader, visitor| {
                expression(reader, visitor)?;
                reader.expect(b',')?;
                grammar::type_identity(reader, visitor)?;
                reader.expect(b',')?;
                let domain = reader.atom(Label::Domain, visitor)?;
                if ArithmeticDomain::from_name(&domain).is_none()
                    && domain != ArithmeticDomain::Exact.name()
                {
                    return Err(Error::MalformedIdentity);
                }
                Ok(())
            }),
            IndexForm::ArrayLiteral => compound(reader, visitor, |reader, visitor| {
                if !reader.next(b')') {
                    expression(reader, visitor)?;
                    while reader.next(b',') {
                        reader.expect(b',')?;
                        expression(reader, visitor)?;
                    }
                }
                Ok(())
            }),
            IndexForm::Indexed | IndexForm::InclusiveRange | IndexForm::ExclusiveRange => {
                compound(reader, visitor, |reader, visitor| {
                    expression(reader, visitor)?;
                    reader.expect(b',')?;
                    expression(reader, visitor)
                })
            }
            IndexForm::Member => compound(reader, visitor, |reader, visitor| {
                expression(reader, visitor)?;
                reader.expect(b',')?;
                reader.opaque(Label::Member, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    reader.opaque(Label::Case, visitor)?;
                }
                Ok(())
            }),
            IndexForm::StructLiteral => compound(reader, visitor, |reader, visitor| {
                grammar::name_atom(reader, Label::Type, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    if reader.peek::<Label>() == Some(Label::Case) {
                        reader.opaque(Label::Case, visitor)?;
                    } else {
                        reader.expect_tag(Label::Field)?;
                        compound(reader, visitor, |reader, visitor| {
                            reader.opaque(Label::Name, visitor)?;
                            reader.expect(b',')?;
                            expression(reader, visitor)
                        })?;
                    }
                }
                Ok(())
            }),
            IndexForm::TypeExpression | IndexForm::ZeroValue => {
                compound(reader, visitor, grammar::type_identity)
            }
            IndexForm::MachineValue => compound(reader, visitor, |reader, visitor| {
                let previous = reader.callable_regions;
                reader.callable_regions = true;
                let result = grammar::callable_child(reader, visitor);
                reader.callable_regions = previous;
                result
            }),
            IndexForm::Atomic => compound(reader, visitor, |reader, visitor| {
                expression(reader, visitor)?;
                reader.expect(b',')?;
                expression(reader, visitor)?;
                reader.expect(b',')?;
                reader.opaque(Label::Ordering, visitor)?;
                reader.expect(b',')?;
                reader.opaque(Label::ResultCustody, visitor)
            }),
            IndexForm::Match => compound(reader, visitor, |reader, visitor| {
                expression(reader, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    reader.expect_tag(Label::Arm)?;
                    compound(reader, visitor, |reader, visitor| {
                        pattern(reader, visitor)?;
                        reader.expect(b',')?;
                        expression(reader, visitor)
                    })?;
                }
                Ok(())
            }),
            IndexForm::SavedSubject => compound(reader, visitor, |reader, visitor| {
                reader.opaque(Label::Depth, visitor)?;
                reader.expect(b',')?;
                expression(reader, visitor)
            }),
            IndexForm::StaticApplication => Err(Error::MalformedIdentity),
            IndexForm::UnsupportedConstSubstitution | IndexForm::UnboundMatchSubject => {
                Err(unadmitted(Unadmitted::InvalidIdentity))
            }
        }
    }

    fn pattern(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        match reader.peek::<PatternForm>() {
            None => expression(reader, visitor),
            Some(PatternForm::Wildcard) => {
                reader.production::<PatternForm>()?;
                scoped(visitor, |_| Ok(()))
            }
            Some(PatternForm::Destructure) => {
                reader.production::<PatternForm>()?;
                compound(reader, visitor, |reader, visitor| {
                    expression(reader, visitor)?;
                    reader.expect(b',')?;
                    reader.expect_tag(Label::Fields)?;
                    compound(reader, visitor, |reader, visitor| {
                        group(reader, visitor, destructured_field)
                    })?;
                    reader.expect(b',')?;
                    reader.opaque(Label::Rest, visitor)?;
                    reader.expect(b',')?;
                    if reader.peek::<Label>() == Some(Label::NoGuard) {
                        reader.expect_tag(Label::NoGuard)?;
                        scoped(visitor, |_| Ok(()))?;
                    } else {
                        expression(reader, visitor)?;
                    }
                    reader.expect(b',')?;
                    reader.expect_tag(Label::ProofSelectors)?;
                    compound(reader, visitor, |reader, visitor| {
                        group(reader, visitor, proof_selector)
                    })
                })
            }
        }
    }

    fn group(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
        member: fn(&mut Reader<'_>, &mut dyn TypeIdentityPackageOwnerVisitor) -> Result,
    ) -> Result {
        if reader.next(b')') {
            return Ok(());
        }
        member(reader, visitor)?;
        while reader.next(b',') {
            reader.expect(b',')?;
            member(reader, visitor)?;
        }
        Ok(())
    }

    fn destructured_field(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        reader.expect_tag(Label::Field)?;
        compound(reader, visitor, |reader, visitor| {
            grammar::name_atom(reader, Label::Name, visitor)?;
            reader.expect(b',')?;
            match reader.production::<FieldActionForm>()? {
                FieldActionForm::Bind
                | FieldActionForm::Waive
                | FieldActionForm::BindCase
                | FieldActionForm::WaiveCase => scoped(visitor, |_| Ok(())),
                FieldActionForm::Equal => compound(reader, visitor, expression),
            }
        })
    }

    fn proof_selector(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        reader.expect_tag(Label::ProofSelector)?;
        compound(reader, visitor, |reader, visitor| {
            reader.opaque(Label::Field, visitor)?;
            reader.expect(b',')?;
            reader.opaque(Label::Binding, visitor)
        })
    }

    pub(super) fn static_argument(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        if reader.peek::<RegionForm>().is_some() {
            return grammar::region(reader, visitor);
        }
        if reader.peek::<IndexForm>() == Some(IndexForm::StaticApplication) {
            reader.production::<IndexForm>()?;
            return compound(reader, visitor, |reader, visitor| {
                expression(reader, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    if reader.peek::<Label>() == Some(Label::Lifetime) {
                        reader.opaque(Label::Lifetime, visitor)?;
                    } else {
                        static_argument(reader, visitor)?;
                    }
                }
                Ok(())
            });
        }
        if reader.peek::<TypeForm>().is_some() {
            grammar::type_identity(reader, visitor)
        } else {
            expression(reader, visitor)
        }
    }

    fn embedded_operation(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let value = reader.atom(Label::Operation, visitor)?;
        let mut nested = Reader::new(&value);
        nested.expect_tag(Label::OpenIndexOperation)?;
        compound(&mut nested, visitor, |nested, visitor| {
            grammar::name_atom(nested, Label::Symbol, visitor)?;
            nested.expect(b',')?;
            nested.opaque(Label::Contract, visitor)
        })?;
        nested.finish()
    }

    fn embedded_algebra(
        reader: &mut Reader<'_>,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        let value = reader.atom(Label::Algebra, visitor)?;
        let mut nested = Reader::new(&value);
        nested.expect_tag(Label::OpenIndexAlgebra)?;
        compound(&mut nested, visitor, |nested, visitor| {
            let provider = nested.atom(Label::Provider, visitor)?;
            let mut operand = Reader::new(&provider);
            static_argument(&mut operand, visitor)?;
            operand.finish()?;
            nested.expect(b',')?;
            grammar::name_atom(nested, Label::Trait, visitor)?;
            nested.expect(b',')?;
            nested.opaque(Label::Requirement, visitor)
        })?;
        nested.finish()
    }
}
mod lifetime_partition {
    pub fn normalize_requirement_lifetime_partition(raw: &[u32]) -> Vec<u32> {
        let mut representatives = Vec::new();
        raw.iter()
            .map(|ordinal| {
                if let Some(index) = representatives
                    .iter()
                    .position(|representative| representative == ordinal)
                {
                    u32::try_from(index).expect("lifetime partition index exceeds u32")
                } else {
                    representatives.push(*ordinal);
                    u32::try_from(representatives.len() - 1)
                        .expect("lifetime partition index exceeds u32")
                }
            })
            .collect()
    }
}
pub mod vocabulary {

    pub trait Spelled: Copy {
        fn spelling(self) -> &'static str;
    }

    pub trait Vocabulary: Spelled + 'static {
        const ALL: &'static [Self];

        fn parse(spelling: &str) -> Option<Self>;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Unadmitted {
        InvalidIdentity,
        ApplicationScope,
    }

    macro_rules! vocabulary {
        ($($(#[$enum_meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $spelling:literal,)+ })+) => {
            $(
                $(#[$enum_meta])*
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                pub enum $name {
                    $($(#[$variant_meta])* $variant,)+
                }

                impl Spelled for $name {
                    fn spelling(self) -> &'static str {
                        match self {
                            $(Self::$variant => $spelling,)+
                        }
                    }
                }

                impl Vocabulary for $name {
                    const ALL: &'static [Self] = &[$(Self::$variant,)+];

                    fn parse(spelling: &str) -> Option<Self> {
                        match spelling {
                            $($spelling => Some(Self::$variant),)+
                            _ => None,
                        }
                    }
                }
            )+

            pub fn is_vocabulary_spelling(spelling: &str) -> bool {
                false $(|| $name::parse(spelling).is_some())+
            }

            #[cfg(test)]
            pub(crate) fn audit() {
                $(audit_enum::<$name>(stringify!($name));)+
            }
        };
    }

    #[cfg(test)]
    fn audit_enum<F: Vocabulary + PartialEq + std::fmt::Debug>(name: &str) {
        let mut seen = Vec::new();
        for form in F::ALL {
            let spelling = form.spelling();
            assert!(!seen.contains(&spelling), "{name} repeats `{spelling}`");
            seen.push(spelling);
            assert_eq!(
                F::parse(spelling).as_ref(),
                Some(form),
                "{name} `{spelling}`"
            );
            let mut reader = super::framing::Reader::new(spelling);
            assert!(
                reader.production::<F>().as_ref() == Ok(form) && reader.finish().is_ok(),
                "{name} `{spelling}` must lex as one tag"
            );
        }
    }

    vocabulary! {
        TypeForm {
            Unit = "unit",
            Nominal = "nominal",
            CompilerType = "compiler-type",
            Reference = "ref",
            MutableReference = "ref-mut",
            WriteOnlyReference = "ref-write",
            Slice = "slice",
            Array = "array",
            Constrained = "constrained",
            Generic = "generic",
            Named = "named",
            DynamicTrait = "dynamic-trait",
            IndexExpression = "index-expression",
            Callable = "callable",
        }

        CallableForm {
            Record = "callable-record",
            Atom = "callable-atom",
            Name = "callable-name",
            Type = "callable-type",
            Index = "callable-index",
        }

        NameForm {
            Nominal = "nominal",
            CompilerType = "compiler-type",
            DerivedApplication = "derived-application",
            PlacedField = "placed-field",
            PlacedOperation = "placed-operation",
            InvalidDerivedOrigin = "invalid-derived-origin",
        }

        OwnerForm {
            Package = "package-owner",
            ToolchainSource = "toolchain-source-owner",
            Toolchain = "toolchain-owner",
            Unresolved = "unresolved-owner",
        }

        ConstraintForm {
            ArithmeticDomain = "arithmetic-domain",
            NamedConstraint = "named-constraint",
            DeclaredDomain = "declared-domain",
            CompilerDomain = "compiler-domain",
        }

        DomainFamily {
            Carry = "carry",
            Value = "value",
            OmegaLayout = "omega-layout",
        }

        ArrayLengthForm {
            Literal = "literal",
            ConstParameter = "const-parameter",
            ConstCall = "const-call",
            ConstExpression = "const-expression",
            InvalidExpression = "invalid-array-expression",
        }

        IndexForm {
            ConstName = "const-name",
            CanonicalConst = "canonical-const",
            IntegerConst = "integer-const",
            Integer = "integer",
            Boolean = "boolean",
            Float = "float",
            String = "string",
            UndeterminedNan = "undetermined-nan",
            BitwiseNot = "bitwise-not",
            LogicalNot = "logical-not",
            Add = "add",
            And = "and",
            BitwiseAnd = "bitwise-and",
            BitwiseOr = "bitwise-or",
            BitwiseXor = "bitwise-xor",
            Divide = "divide",
            Equal = "equal",
            Greater = "greater",
            GreaterOrEqual = "greater-or-equal",
            Less = "less",
            LessOrEqual = "less-or-equal",
            Modulo = "modulo",
            Multiply = "multiply",
            NotEqual = "not-equal",
            Or = "or",
            ShiftLeft = "shift-left",
            ShiftRight = "shift-right",
            Subtract = "subtract",
            CaseMembership = "case_membership",
            Call = "call",
            RuntimeCall = "runtime-call",
            Cast = "cast",
            ArrayLiteral = "array-literal",
            Indexed = "indexed",
            Member = "member",
            Borrow = "borrow",
            MutableBorrow = "borrow-mut",
            WriteOnlyBorrow = "borrow-write",
            InclusiveRange = "range-inclusive",
            ExclusiveRange = "range-exclusive",
            StructLiteral = "struct-literal",
            TypeExpression = "type-expression",
            ZeroValue = "zero-value",
            Atomic = "atomic",
            Match = "match",
            SavedSubject = "saved-subject",
            /// `case-component(<index>)`: the case view of the same saved value.
            CaseComponent = "case-component",
            /// `static-application(<index>,<argument or lifetime(<text>)>*)`: a
            /// selected declaration with its static operands.
            StaticApplication = "static-application",
            MachineValue = "machine-value",
            UnsupportedConstSubstitution = "unsupported-const-substitution",
            UnboundMatchSubject = "unbound-match-subject",
        }

        RegionForm {
            Region = "region",
            Elided = "elided-region",
            Unresolved = "unresolved-region",
        }

        PatternForm {
            Wildcard = "wildcard",
            Destructure = "destructure",
        }

        FieldActionForm {
            Bind = "bind",
            BindCase = "bind-case",
            /// `equal(<index>)`
            Equal = "equal",
            Waive = "waive",
            WaiveCase = "waive-case",
        }

        Label {
            Name = "name",
            Path = "path",
            Atom = "atom",
            Type = "type",
            Encoding = "encoding",
            Family = "family",
            Permission = "permission",
            Domain = "domain",
            Grammar = "grammar",
            Schema = "schema",
            Target = "target",
            StaticArguments = "static-arguments",
            Operation = "operation",
            Algebra = "algebra",
            OpenIndexOperation = "open-index-operation",
            OpenIndexAlgebra = "open-index-algebra",
            Symbol = "symbol",
            Contract = "contract",
            Provider = "provider",
            Trait = "trait",
            Requirement = "requirement",
            Lifetime = "lifetime",
            Ordering = "ordering",
            ResultCustody = "result-custody",
            Member = "member",
            Case = "case",
            Field = "field",
            Fields = "fields",
            Arm = "arm",
            NoGuard = "no-guard",
            Rest = "rest",
            ProofSelectors = "proof-selectors",
            ProofSelector = "proof-selector",
            Binding = "binding",
            Depth = "depth",
            Role = "role",
            Owner = "owner",
            Ordinal = "ordinal",
            Kind = "kind",
            Declaration = "declaration",
        }
    }
}
pub use callable_display::callable_display_path;
pub use lifetime_partition::normalize_requirement_lifetime_partition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIdentityVisitError {
    MalformedIdentity,
    ResourceLimitExceeded,
    AllocationFailed,
    UnknownPackage,
    UnsupportedEmbeddedName,
    Unadmitted(vocabulary::Unadmitted),
}

impl std::fmt::Display for TypeIdentityVisitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MalformedIdentity => "malformed canonical type identity",
            Self::ResourceLimitExceeded => "canonical identity resource limit exceeded",
            Self::AllocationFailed => "canonical identity allocation failed",
            Self::UnknownPackage => "canonical identity refers to an unknown package",
            Self::UnsupportedEmbeddedName => "unsupported embedded canonical name",
            Self::Unadmitted(vocabulary::Unadmitted::InvalidIdentity) => {
                "canonical identity carries a marker of an invalid identity"
            }
            Self::Unadmitted(vocabulary::Unadmitted::ApplicationScope) => {
                "canonical identity carries a region of an application scope"
            }
        })
    }
}

impl std::error::Error for TypeIdentityVisitError {}

pub trait TypeIdentityPackageOwnerVisitor {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError>;
    fn leave(&mut self);
    fn reserve(&mut self, owned_bytes: usize) -> Result<(), TypeIdentityVisitError>;
    fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError>;
    fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError>;
}

pub fn visit_type_identity_package_owners(
    identity: &str,
    visitor: &mut impl TypeIdentityPackageOwnerVisitor,
) -> Result<(), TypeIdentityVisitError> {
    let mut reader = framing::Reader::new(identity);
    grammar::type_identity(&mut reader, visitor)?;
    reader.finish()
}

pub fn malformed_identity_offset(identity: &str) -> Option<usize> {
    struct Unbudgeted;
    impl TypeIdentityPackageOwnerVisitor for Unbudgeted {
        fn enter(&mut self) -> Result {
            Ok(())
        }
        fn leave(&mut self) {}
        fn reserve(&mut self, _: usize) -> Result {
            Ok(())
        }
        fn package_owner(&mut self, _: [u8; 32]) -> Result {
            Ok(())
        }
        fn embedded_name(&mut self, _: &str) -> Result {
            Ok(())
        }
    }
    let mut reader = framing::Reader::new(identity);
    match grammar::type_identity(&mut reader, &mut Unbudgeted).and_then(|()| reader.finish()) {
        Err(Error::MalformedIdentity | Error::Unadmitted(_)) => Some(reader.position()),
        _ => None,
    }
}

type Error = TypeIdentityVisitError;
type Result<T = (), E = Error> = std::result::Result<T, E>;

fn unadmitted(reason: vocabulary::Unadmitted) -> Error {
    Error::Unadmitted(reason)
}

fn scoped<T>(
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    action: impl FnOnce(&mut dyn TypeIdentityPackageOwnerVisitor) -> Result<T>,
) -> Result<T> {
    visitor.enter()?;
    let result = action(visitor);
    visitor.leave();
    result
}

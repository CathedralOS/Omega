//! Type, nominal, and domain productions of the canonical renderer.

use super::vocabulary::{
    ConstraintForm, DomainFamily, IndexForm, Label, NameForm, OwnerForm, RegionForm, Spelled,
    TypeForm, Unadmitted, Vocabulary,
};
use super::{
    Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, index, scoped, unadmitted,
};

/// The children between a production's parentheses, charged as one node.
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
                    return Err(region_operand(reader));
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
        // Generic applications retain both type and const operands in
        // declaration order. Each operand keeps its own owner grammar.
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
    }
}

/// The type and encoding of an evaluator-created constant are literal text, not
/// a recursively encoded package-qualified type reference.
pub(super) fn canonical_const(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    compound(reader, visitor, |reader, visitor| {
        reader.opaque(Label::Type, visitor)?;
        reader.expect(b',')?;
        reader.opaque(Label::Encoding, visitor)
    })
}

/// A region exists only while resolving under application scopes. A retained
/// policy holds exact-owner identities, which resolve none.
pub(super) fn region(reader: &mut Reader<'_>) -> Result {
    match reader.production::<RegionForm>()? {
        RegionForm::Region | RegionForm::Elided | RegionForm::Unresolved => {
            Err(unadmitted(Unadmitted::ApplicationScope))
        }
    }
}

/// A second operand of a reference is its region. The refusal stands at the
/// comma that opens it, where the grammar stopped expecting more.
fn region_operand(reader: &Reader<'_>) -> Error {
    match reader.peek_after_comma::<RegionForm>() {
        Some(RegionForm::Region | RegionForm::Elided | RegionForm::Unresolved) => {
            unadmitted(Unadmitted::ApplicationScope)
        }
        None => Error::MalformedIdentity,
    }
}

/// A name atom named `tag`, read as a qualified name.
pub(super) fn name_atom(
    reader: &mut Reader<'_>,
    tag: impl Spelled,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let name = reader.atom(tag, visitor)?;
    semantic_name(&name, visitor)
}

/// The same, for an atom whose tag was already consumed as a production.
pub(super) fn name_value(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let name = reader.leaf_atom(visitor)?;
    semantic_name(&name, visitor)
}

/// Qualified names are the vocabulary's `NameForm`s. A vocabulary tag that is
/// not one of them is a production in the wrong place. Anything else is a
/// binder replacement, which only the consumer can read.
fn semantic_name(name: &str, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
    scoped(visitor, |visitor| {
        let mut reader = Reader::new(name);
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
            NameForm::InvalidDerivedOrigin => return Err(unadmitted(Unadmitted::InvalidIdentity)),
        }
        reader.finish()
    })
}

fn nominal(reader: &mut Reader<'_>, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
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

/// The generic application that produced a derived declaration, and the member
/// of it that this name selects by identity or by field name.
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
        // Exported policy requires an exact package or toolchain source owner.
        // The renderer writes these beside the flag that fails the request.
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
        // NormalizedConstraint wraps the already-qualified domain application
        // in another name atom. Only this designated position is recursive.
        constraint(&mut nested, visitor)?;
        nested.finish()?;
    } else {
        semantic_name(&name, visitor)?;
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

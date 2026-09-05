//! Type, nominal, and domain productions from the existing canonical renderer.

use super::{Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, index, scoped};

pub(super) fn type_identity(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    scoped(visitor, |visitor| {
        let tag = reader.tag()?;
        if tag == "unit" {
            return Ok(());
        }
        reader.expect(b'(')?;
        match tag {
            "nominal" => {
                owner(reader, visitor)?;
                reader.expect(b',')?;
                reader.opaque("path", visitor)?;
            }
            "compiler-type" => reader.opaque("atom", visitor)?,
            "ref" | "ref-mut" | "ref-write" | "slice" => type_identity(reader, visitor)?,
            "array" => {
                type_identity(reader, visitor)?;
                reader.expect(b',')?;
                index::array_length(reader, visitor)?;
            }
            "constrained" => {
                type_identity(reader, visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    constraint(reader, visitor)?;
                }
            }
            "generic" => {
                name_atom(reader, "name", visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    type_identity(reader, visitor)?;
                }
            }
            "named" => const_or_name(reader, "name", visitor)?,
            "dynamic-trait" => {
                name_atom(reader, "name", visitor)?;
                if reader.next(b',') {
                    reader.expect(b',')?;
                    name_atom(reader, "conformance", visitor)?;
                }
            }
            "index-expression" => index::expression(reader, visitor)?,
            _ => return Err(Error::MalformedIdentity),
        }
        reader.expect(b')')
    })
}

pub(super) fn const_or_name(
    reader: &mut Reader<'_>,
    tag: &str,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    if reader.starts("integer-const(") {
        reader.opaque("integer-const", visitor)
    } else if reader.starts("canonical-const(") {
        scoped(visitor, |visitor| {
            reader.tag()?;
            reader.expect(b'(')?;
            // Canonical const evaluator metadata is literal value/type text,
            // not a recursively encoded package-qualified type reference.
            reader.opaque("type", visitor)?;
            reader.expect(b',')?;
            reader.opaque("encoding", visitor)?;
            reader.expect(b')')
        })
    } else {
        name_atom(reader, tag, visitor)
    }
}

pub(super) fn name_atom(
    reader: &mut Reader<'_>,
    tag: &str,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let name = reader.atom(tag, visitor)?;
    semantic_name(&name, visitor)
}

fn semantic_name(name: &str, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
    scoped(visitor, |visitor| {
        if name.starts_with("nominal(") {
            let mut reader = Reader::new(name);
            reader.tag()?;
            reader.expect(b'(')?;
            owner(&mut reader, visitor)?;
            reader.expect(b',')?;
            reader.opaque("path", visitor)?;
            reader.expect(b')')?;
            reader.finish()
        } else if name.starts_with("compiler-type(") {
            let mut reader = Reader::new(name);
            reader.tag()?;
            reader.expect(b'(')?;
            reader.opaque("atom", visitor)?;
            reader.expect(b')')?;
            reader.finish()
        } else {
            visitor.embedded_name(name)
        }
    })
}

fn owner(reader: &mut Reader<'_>, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
    if reader.starts("package-owner(") {
        let hexadecimal = reader.byte_atom("package-owner", visitor)?;
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
    } else if reader.starts("toolchain-source-owner(") {
        if reader.byte_atom("toolchain-source-owner", visitor)?.len() != 64 {
            return Err(Error::MalformedIdentity);
        }
        Ok(())
    } else {
        // Exported policy requires an exact package or toolchain source owner.
        // The renderer's diagnostic unresolved/non-source forms are not policy.
        Err(Error::MalformedIdentity)
    }
}

fn constraint(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    scoped(visitor, |visitor| {
        let tag = reader.tag()?;
        reader.expect(b'(')?;
        match tag {
            "arithmetic-domain" | "named-constraint" => reader.opaque("name", visitor)?,
            "declared-domain" => declared_domain(reader, visitor)?,
            "compiler-domain" => compiler_domain(reader, visitor)?,
            "range" => {
                for (index, tag) in ["minimum", "maximum"].into_iter().enumerate() {
                    if index != 0 {
                        reader.expect(b',')?;
                    }
                    let encoded = reader.atom(tag, visitor)?;
                    let mut nested = Reader::new(&encoded);
                    index::expression(&mut nested, visitor)?;
                    nested.finish()?;
                }
            }
            _ => return Err(Error::MalformedIdentity),
        }
        reader.expect(b')')
    })
}

fn declared_domain(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let name = reader.atom("name", visitor)?;
    if name.starts_with("declared-domain(") {
        // NormalizedConstraint wraps the already-qualified domain application
        // in another name atom. Only this designated position is recursive.
        let mut nested = Reader::new(&name);
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
    let family = reader.atom("family", visitor)?;
    reader.expect(b',')?;
    match family.as_ref() {
        "carry" => reader.opaque("permission", visitor),
        "value" => reader.opaque("domain", visitor),
        "omega-layout" => {
            reader.opaque("grammar", visitor)?;
            while reader.next(b',') {
                reader.expect(b',')?;
                scoped(visitor, |visitor| {
                    if reader.tag()? != "schema" {
                        return Err(Error::MalformedIdentity);
                    }
                    reader.expect(b'(')?;
                    type_identity(reader, visitor)?;
                    reader.expect(b')')
                })?;
            }
            Ok(())
        }
        _ => Err(Error::MalformedIdentity),
    }
}

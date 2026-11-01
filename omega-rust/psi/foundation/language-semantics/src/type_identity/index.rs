//! Structural const/index forms, including qualified algebra and operation names.

use super::{Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, grammar, scoped};

pub(super) fn array_length(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    if reader.starts("literal(") {
        reader.opaque("literal", visitor)
    } else if reader.starts("const-parameter(") {
        grammar::name_atom(reader, "const-parameter", visitor)
    } else if reader.starts("const-call(") {
        reader.opaque("const-call", visitor)
    } else if reader.starts("const-expression(") {
        scoped(visitor, |visitor| {
            reader.tag()?;
            reader.expect(b'(')?;
            expression(reader, visitor)?;
            reader.expect(b')')
        })
    } else {
        Err(Error::MalformedIdentity)
    }
}

pub(super) fn expression(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    if reader.starts("const-name(")
        || reader.starts("canonical-const(")
        || reader.starts("integer-const(")
    {
        return grammar::const_or_name(reader, "const-name", visitor);
    }
    if reader.starts("string(") {
        return reader.byte_atom("string", visitor).map(|_| ());
    }
    for tag in ["integer", "boolean", "float"] {
        if reader.starts(tag) {
            return reader.opaque(tag, visitor);
        }
    }
    scoped(visitor, |visitor| {
        let tag = reader.tag()?;
        reader.expect(b'(')?;
        match tag {
            "bitwise-not" | "logical-not" => expression(reader, visitor)?,
            "add" | "and" | "bitwise-and" | "bitwise-or" | "bitwise-xor" | "divide" | "equal"
            | "greater" | "greater-or-equal" | "less" | "less-or-equal" | "modulo" | "multiply"
            | "not-equal" | "or" | "shift-left" | "shift-right" | "subtract" => {
                let licensed = reader.starts("operation(");
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
            }
            "call" => {
                grammar::name_atom(reader, "target", visitor)?;
                while reader.next(b',') {
                    reader.expect(b',')?;
                    if reader.starts("static-arguments(") {
                        scoped(visitor, |visitor| {
                            reader.tag()?;
                            reader.expect(b'(')?;
                            static_argument(reader, visitor)?;
                            while reader.next(b',') {
                                reader.expect(b',')?;
                                static_argument(reader, visitor)?;
                            }
                            reader.expect(b')')
                        })?;
                    } else {
                        expression(reader, visitor)?;
                    }
                }
            }
            "cast" => {
                expression(reader, visitor)?;
                reader.expect(b',')?;
                grammar::type_identity(reader, visitor)?;
                reader.expect(b',')?;
                let domain = reader.atom("domain", visitor)?;
                if !matches!(
                    domain.as_ref(),
                    "Exact" | "Wrapping" | "Saturating" | "Trapping"
                ) {
                    return Err(Error::MalformedIdentity);
                }
            }
            _ => return Err(Error::MalformedIdentity),
        }
        reader.expect(b')')
    })
}

/// Static call and generic operands use the existing type/index grammars.
/// Visit nested selected declarations too: opaque payload text would hide
/// package owners.
pub(super) fn static_argument(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    if reader.starts("static-application(") {
        return scoped(visitor, |visitor| {
            reader.tag()?;
            reader.expect(b'(')?;
            expression(reader, visitor)?;
            while reader.next(b',') {
                reader.expect(b',')?;
                if reader.starts("lifetime(") {
                    reader.opaque("lifetime", visitor)?;
                } else {
                    static_argument(reader, visitor)?;
                }
            }
            reader.expect(b')')
        });
    }
    if [
        "unit",
        "nominal(",
        "compiler-type(",
        "ref(",
        "ref-mut(",
        "ref-write(",
        "slice(",
        "array(",
        "constrained(",
        "generic(",
        "named(",
        "dynamic-trait(",
        "index-expression(",
    ]
    .iter()
    .any(|tag| reader.starts(tag))
    {
        grammar::type_identity(reader, visitor)
    } else {
        expression(reader, visitor)
    }
}

fn embedded_operation(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let value = reader.atom("operation", visitor)?;
    let mut nested = Reader::new(&value);
    scoped(visitor, |visitor| {
        if nested.tag()? != "open-index-operation" {
            return Err(Error::MalformedIdentity);
        }
        nested.expect(b'(')?;
        grammar::name_atom(&mut nested, "symbol", visitor)?;
        nested.expect(b',')?;
        nested.opaque("contract", visitor)?;
        nested.expect(b')')?;
        nested.finish()
    })
}

fn embedded_algebra(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    let value = reader.atom("algebra", visitor)?;
    let mut nested = Reader::new(&value);
    scoped(visitor, |visitor| {
        if nested.tag()? != "open-index-algebra" {
            return Err(Error::MalformedIdentity);
        }
        nested.expect(b'(')?;
        grammar::name_atom(&mut nested, "provider", visitor)?;
        nested.expect(b',')?;
        grammar::name_atom(&mut nested, "trait", visitor)?;
        nested.expect(b',')?;
        nested.opaque("requirement", visitor)?;
        nested.expect(b',')?;
        nested.opaque("alias", visitor)?;
        nested.expect(b')')?;
        nested.finish()
    })
}

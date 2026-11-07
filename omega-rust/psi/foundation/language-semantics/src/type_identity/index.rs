//! Structural const/index forms, including qualified algebra and operation names.

use super::grammar::{self, compound};
use super::vocabulary::{
    ArrayLengthForm, FieldActionForm, IndexForm, Label, PatternForm, RegionForm, TypeForm,
    Unadmitted,
};
use super::{Error, Result, TypeIdentityPackageOwnerVisitor, framing::Reader, scoped, unadmitted};
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
        // Static operands belong only to their designated application position.
        IndexForm::StaticApplication => Err(Error::MalformedIdentity),
        IndexForm::UnsupportedConstSubstitution | IndexForm::UnboundMatchSubject => {
            Err(unadmitted(Unadmitted::InvalidIdentity))
        }
    }
}

fn pattern(reader: &mut Reader<'_>, visitor: &mut dyn TypeIdentityPackageOwnerVisitor) -> Result {
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

/// Comma-separated members of a group, which may be empty.
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
            FieldActionForm::Bind | FieldActionForm::Waive => scoped(visitor, |_| Ok(())),
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

/// Generic and static-call operands use the same type/index grammars. Visit
/// nested selected declarations too: opaque text would hide package owners.
pub(super) fn static_argument(
    reader: &mut Reader<'_>,
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
) -> Result {
    if reader.peek::<RegionForm>().is_some() {
        return grammar::region(reader);
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

/// The selected algebra names its provider as a static operand, then its trait
/// and the requirement the operation implements.
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

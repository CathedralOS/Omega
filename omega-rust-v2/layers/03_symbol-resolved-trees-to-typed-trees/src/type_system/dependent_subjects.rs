use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{Site, TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;
use omega_typed::site_at;

impl ResolvedSourceFile<'_> {
    pub(crate) fn binder_site(
        &self,
        span: SourceSpan,
    ) -> Site {
        Site {
            package: self.file.package.into(),
            file: self.file.path.into(),
            span,
        }
    }
    pub(crate) fn mathematical_subject(
        &self,
        n: usize,
        expected: &ValueType,
        types: &TypeCatalog<'_>,
        version: Option<&Site>,
    ) -> Result<ValueType, TypeError> {
        let f = self.file;
        // A group is its operand; groups nest as deeply as L1 admits.
        let mut n = n;
        while f.nodes[n].kind == NodeKind::Paren {
            n = self.single_child(n)?;
        }
        if let Some(Symbol::Binding {
            binder_node,
            kind: NodeKind::Param,
            ..
        }) = self.symbol(n)
        {
            let annotation = self
                .parameter_annotation(*binder_node)
                .ok_or_else(|| TypeError::unsupported(f, n))?;
            if self.raw_type(annotation, types)? != *expected {
                return Err(TypeError::at(f, n, "type-family subject has the wrong type"));
            }
        } else if f.nodes[n].kind == NodeKind::Literal {
            let (actual, suffix) = numeric_literals::literal(f, n)?;
            if suffix.as_ref().is_some_and(|t| t != expected) || !actual.fits(expected) {
                return Err(TypeError::at(f, n, "type-family literal has the wrong type"));
            }
        } else if f.nodes[n].kind == NodeKind::Call
            && let [callee, args] = *f.nodes[n].children.as_slice()
            && let Some(Symbol::Declaration(id)) = self.symbol(callee)
            && id.kind == NodeKind::MathLet
            && f.nodes[callee].children.is_empty()
        {
            let mut signature = types
                .mathematical
                .get(id)
                .cloned()
                .ok_or_else(|| TypeError::at(f, n, "mathematical subject has an unresolved interface"))?;
            let mut subjects = vec![];
            for &arg in &f.nodes[args].children {
                let ValueType::Function(input, out) = signature else {
                    return Err(TypeError::at(f, n, "excess mathematical arguments"));
                };
                let value = self.mathematical_subject(arg, &input, types, version)?;
                signature = out.apply_subject(&value);
                subjects.push(value);
            }
            if &signature != expected {
                return Err(TypeError::at(f, n, "type-family subject has the wrong type"));
            }
            return Ok(ValueType::MathematicalValue {
                definition: Box::new(id.clone()),
                subjects,
            });
        } else {
            return Err(TypeError::at(f, n, "type-family argument requires a typed mathematical subject"));
        }
        Ok(self.term_subject(n, expected, version))
    }
    pub(crate) fn term_subject(
        &self,
        n: usize,
        ty: &ValueType,
        version: Option<&Site>,
    ) -> ValueType {
        let mut n = n;
        loop {
            if let Some(Symbol::Binding { span, .. }) = self.symbol(n) {
                return ValueType::Subject {
                    declaration: self.binder_site(*span),
                    version: version.cloned(),
                };
            }
            // A group is its operand; groups nest as deeply as L1 admits.
            if self.file.nodes[n].kind == NodeKind::Paren
                && let Ok(child) = self.single_child(n)
            {
                n = child;
                continue;
            }
            break;
        }
        if self.file.nodes[n].kind == NodeKind::Literal
            && let Ok((
                ValueType::Exact {
                    value,
                    fractional_origin: None,
                },
                _,
            )) = numeric_literals::literal(self.file, n)
            && value.is_integer()
            && ty.contains_integer(value.numer())
        {
            return ValueType::StaticInteger {
                carrier: Box::new(ty.clone()),
                value: value.to_integer(),
            };
        }
        ValueType::Subject {
            declaration: site_at(self.file, n),
            version: version.cloned(),
        }
    }
}
pub(crate) trait DependentSubjects: Sized {
    fn instantiate_subjects(
        &self,
        signature: &crate::TypedSignature,
        operands: &[ValueType],
    ) -> Self;
    fn bind_subject(
        &self,
        site: &Site,
    ) -> Self;
    fn apply_subject(
        &self,
        subject: &Self,
    ) -> Self;
}

impl DependentSubjects for ValueType {
    fn instantiate_subjects(
        &self,
        signature: &crate::TypedSignature,
        operands: &[ValueType],
    ) -> Self {
        self.replace_subjects(|declaration| {
            if declaration.package != signature.site.package || declaration.file != signature.site.file {
                return None;
            }
            signature
                .parameters
                .iter()
                .zip(operands)
                .find(|((span, _), _)| span == &declaration.span)
                .map(|(_, v)| v)
        })
    }
    fn bind_subject(
        &self,
        site: &Site,
    ) -> Self {
        self.rewrite(0, &|t, d| match t {
            Self::Subject { declaration, version: None } if declaration == site => Some(Self::BoundSubject(d)),
            _ => None,
        })
    }
    fn apply_subject(
        &self,
        subject: &Self,
    ) -> Self {
        self.rewrite(0, &|t, d| match t {
            Self::BoundSubject(i) if *i == d => Some(subject.rewrite(0, &|s, under| match s {
                Self::BoundSubject(j) if *j >= under => Some(Self::BoundSubject(j + d)),
                _ => None,
            })),
            Self::BoundSubject(i) if *i > d => Some(Self::BoundSubject(i - 1)),
            _ => None,
        })
    }
}

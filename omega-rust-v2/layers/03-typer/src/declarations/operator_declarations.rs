use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::ResolvedStage;

pub(crate) fn index_operator_declarations<'a>(
    units: &[ResolvedSourceFile<'a>],
    resolved: &'a ResolvedStage,
    catalog: &mut DeclarationCatalog<'a>,
) -> Result<(), TypeError> {
    for binding in &resolved.operator_declarations {
        let signature = catalog
            .signatures
            .get(&binding.machine)
            .ok_or_else(|| TypeError {
                package: binding.machine.package.clone(),
                file: String::new(),
                span: SourceSpan::new(0, 0),
                message: "operator has no typed signature".into(),
            })?;
        let arity = signature.params.len();
        let valid = match binding.token.as_str() {
            "!" | "~" => arity == 1,
            "-" => matches!(arity, 1 | 2),
            "[..]" => arity >= 2,
            _ => arity == 2,
        };
        let signature_site = || {
            let unit = units
                .iter()
                .find(|u| u.definitions.values().any(|&d| d == &binding.machine))
                .unwrap();
            let span = unit
                .definitions
                .iter()
                .find_map(|(s, &d)| (d == &binding.machine).then_some(*s))
                .unwrap();
            (unit.file.path.to_owned(), span)
        };
        let family = catalog
            .token_bindings
            .entry(binding.token.as_str())
            .or_default();
        let primitive = binding.home.is_none()
            && signature
                .params
                .iter()
                .all(|(_, t)| t.numeric() || t == &ValueType::Bool);
        let duplicate = family.iter().any(|b| {
            b.home == binding.home
                && b.machine.package == binding.machine.package
                && catalog.signatures[&b.machine]
                    .params
                    .iter()
                    .map(|(_, t)| t)
                    .eq(signature.params.iter().map(|(_, t)| t))
        });
        if !valid || duplicate || primitive {
            let (file, span) = signature_site();
            return Err(TypeError {
                package: binding.machine.package.clone(),
                file,
                span,
                message: if !valid {
                    "incorrect operator arity"
                } else if primitive {
                    "cannot bind a compiler-owned primitive operator family"
                } else {
                    "duplicate normalized operator binding"
                }
                .into(),
            });
        }
        family.push(binding);
    }
    Ok(())
}

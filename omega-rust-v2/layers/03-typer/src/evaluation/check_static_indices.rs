use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::{TypeError, TypedStage};

pub(crate) fn check_static_indices(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut open_indices: Vec<_> = catalog
        .types
        .open_indices
        .borrow()
        .iter()
        .map(|(site, (n, carrier))| (site.clone(), *n, carrier.clone()))
        .collect();
    open_indices.sort_by_cached_key(|(site, _, _)| (site.package.clone(), site.file.clone(), site.span.get_start_offset()));
    for (site, n, carrier) in open_indices {
        let unit = units
            .iter()
            .find(|u| u.file.package == site.package && u.file.path == site.file)
            .unwrap();
        declaration_bodies::check_static_argument(unit, catalog, n, &carrier, output)?;
    }
    let mut evaluated: Vec<_> = catalog
        .types
        .evaluations
        .borrow()
        .iter()
        .map(|(request, (n, value))| (request.clone(), *n, value.clone()))
        .collect();
    evaluated.sort_by_cached_key(|(request, _, _)| {
        (
            request.expression.package.clone(),
            request.expression.file.clone(),
            request.expression.span.get_start_offset(),
            request.carrier.to_string(),
        )
    });
    for (request, n, value) in evaluated {
        let unit = units
            .iter()
            .find(|u| u.file.package == request.expression.package && u.file.path == request.expression.file)
            .unwrap();
        declaration_bodies::check_static_argument(unit, catalog, n, &request.carrier, output)?;
        output.evaluated_indices.push((request, value));
    }
    // Recheck every authored intermediate at a closed call, even when a canonical
    // rewrite or a surrounding operation makes the final index representable.
    for call in &output.calls {
        if let Some(formations) = catalog.types.formations.borrow().get(&call.machine) {
            let mut ordered: Vec<_> = formations.iter().collect();
            ordered.sort_by_key(|(site, _)| (&site.package, &site.file, site.span.get_start_offset(), site.span.get_end_offset()));
            for (_, term) in ordered {
                term.substitute(&call.machine, &call.type_arguments)
                    .validate_closed_indices()
                    .map_err(|message| TypeError {
                        package: call.site.package.clone(),
                        file: call.site.file.clone(),
                        span: call.site.span,
                        message,
                    })?;
            }
        }
    }
    Ok(())
}

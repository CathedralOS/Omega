//! A package has one external representation. Checking that property uses the
//! typed fields of records and cases, including carriers declared in other files.
use super::declaration_catalog::DeclarationCatalog;
use crate::ValueType;
use omega_resolver::DeclarationId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Check {
    Uniform,
    Sized,
}

struct Application<'a> {
    owner: &'a DeclarationId,
    arguments: Vec<(&'a ValueType, bool)>,
    parent: Option<usize>,
    check: Check,
}

impl DeclarationCatalog<'_> {
    /// With a package owner, check uniformity in its private witnesses. At a
    /// construction site, check that the inferred concrete witness is sized.
    pub(crate) fn package_representation(
        &self,
        ty: &ValueType,
        package: Option<&DeclarationId>,
    ) -> bool {
        use ValueType::*;
        let mut frames: Vec<Application<'_>> = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut pending = vec![(ty, None::<usize>, if package.is_some() { Check::Uniform } else { Check::Sized })];
        while let Some((ty, scope, check)) = pending.pop() {
            let binding = |part: &ValueType| {
                let Parameter { owner, index } = part else { return None };
                let frame = &frames[scope?];
                self.types
                    .scoped_arguments(frame.owner, &frame.arguments)
                    .find(|(binder, _)| *binder == owner.as_ref())
                    .map(|(_, values)| (values[*index], frame.parent))
            };
            let depends = |ty: &ValueType| {
                ty.components().any(|part| match part {
                    Witness { owner, .. } => Some(owner.as_ref()) == package,
                    Parameter { .. } => binding(part).is_some_and(|((_, dependent), _)| dependent),
                    _ => false,
                })
            };
            if check == Check::Uniform && !depends(ty) {
                continue;
            }
            if let Some(((argument, _), caller)) = binding(ty) {
                pending.push((argument, caller, check));
                continue;
            }
            match ty {
                Slice(_) => return false,
                Witness { .. } if check == Check::Uniform => return false,
                Reference(_, referent) if check == Check::Uniform => pending.push((referent, scope, Check::Sized)),
                Callable(sig) if check == Check::Uniform => pending.extend(
                    sig.parameters
                        .iter()
                        .chain([&sig.result])
                        .map(|t| (t, scope, check)),
                ),
                AtLifetime(t, _) | Qualified(t, _) | Array(t, _) => pending.push((t, scope, check)),
                IndexedArray(t, len) => {
                    pending.push((t, scope, check));
                    pending.push((len, scope, check));
                }
                IndexedInteger { width, .. } => pending.push((width, scope, check)),
                Tuple(parts) => pending.extend(parts.iter().map(|t| (t, scope, check))),
                Nominal(_) | Applied(..) => {
                    let (owner, arguments) = ty.nominal_parts().unwrap();
                    // Following a parameter resumes the caller's scope. Thus Cell<Cell<T>>
                    // consumes two finite applications, while a by-value field cycle recurs.
                    let mut ancestor = scope;
                    while let Some(i) = ancestor {
                        let frame = &frames[i];
                        if frame.owner == owner && frame.check == check {
                            return false;
                        }
                        ancestor = frame.parent;
                    }
                    // Closed applications have the same fields in every scope.
                    let environment = if arguments.is_empty() { None } else { scope };
                    if !visited.insert((owner, arguments, environment, check)) {
                        continue;
                    }
                    let nested = Some(frames.len());
                    let arguments = arguments
                        .iter()
                        .map(|t| (t, check == Check::Uniform && depends(t)))
                        .collect();
                    frames.push(Application {
                        owner,
                        arguments,
                        parent: scope,
                        check,
                    });
                    pending.extend(
                        self.stored_fields(owner, None)
                            .map(|(_, field)| (&field.ty, nested, check)),
                    );
                }
                Evidence(_) | Callable(_) | Reference(..) => {}
                _ if check == Check::Uniform => return false,
                _ => {}
            }
        }
        true
    }
}

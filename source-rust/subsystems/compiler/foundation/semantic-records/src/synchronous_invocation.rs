use crate::CanonicalStructuralPathSegment;

/// Names in a published path survive moving a declaration between artifacts.
pub type InvocationPathSegment = CanonicalStructuralPathSegment<String, String>;

/// Format an already-resolved field path for diagnostics, not for identity.
pub fn display_invocation_path(path: &[InvocationPathSegment]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for step in path {
        match step {
            InvocationPathSegment::Field(name) => {
                let _ = write!(out, ".{name}");
            }
            InvocationPathSegment::Case(name) => {
                let _ = write!(out, "::{name}");
            }
            InvocationPathSegment::FixedIndex(index) => {
                let _ = write!(out, "[{index}]");
            }
            InvocationPathSegment::Referent => out.push_str(".*"),
        }
    }
    out
}

/// A synchronous call is either to a service or through a callable parameter.
/// Invoking one field does not imply that a sibling callback can run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SynchronousInvocation<S> {
    Parameter(u32),
    Service(S),
    ProjectedParameter {
        parameter: u32,
        path: Vec<InvocationPathSegment>,
    },
}

impl<S> SynchronousInvocation<S> {
    pub fn parameter_path(&self) -> Option<(u32, &[InvocationPathSegment])> {
        match self {
            Self::Parameter(position) => Some((*position, &[])),
            Self::ProjectedParameter { parameter, path } => Some((*parameter, path)),
            Self::Service(_) => None,
        }
    }

    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position)
            | Self::ProjectedParameter {
                parameter: position,
                ..
            } => Some(*position),
            Self::Service(_) => None,
        }
    }

    pub const fn service(&self) -> Option<&S> {
        match self {
            Self::Service(service) => Some(service),
            _ => None,
        }
    }

    pub fn try_map_service<T, E>(
        &self,
        map: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<SynchronousInvocation<T>, E> {
        Ok(match self {
            Self::Parameter(position) => SynchronousInvocation::Parameter(*position),
            Self::ProjectedParameter { parameter, path } => {
                SynchronousInvocation::ProjectedParameter {
                    parameter: *parameter,
                    path: path.clone(),
                }
            }
            Self::Service(service) => SynchronousInvocation::Service(map(service)?),
        })
    }

    pub fn validate_parameter(&self, parameter_count: usize) -> Result<(), &'static str> {
        if self
            .parameter()
            .is_some_and(|position| position as usize >= parameter_count)
        {
            return Err("synchronous invocation refers to a parameter outside the signature");
        }
        if let Self::ProjectedParameter { path, .. } = self
            && (path.is_empty() || path.iter().any(|step| matches!(step,
                InvocationPathSegment::Field(name) | InvocationPathSegment::Case(name) if name.is_empty())))
        {
            return Err("synchronous invocation needs a nonempty projection with named fields and cases");
        }
        Ok(())
    }
}

impl SynchronousInvocation<String> {
    pub fn validate_entries(entries: &[Self], parameter_count: usize) -> Result<(), &'static str> {
        if entries.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("synchronous invocations are unordered or repeated");
        }
        for entry in entries {
            entry.validate_parameter(parameter_count)?;
            if entry.service().is_some_and(|name| name.is_empty()) {
                return Err("synchronous invocation has an empty service identity");
            }
        }
        Ok(())
    }
}

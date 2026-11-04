//! The compiler-execution floor, independent of Source or Terminal storage.
//!
//! Producers retain checked facts. Admission supplies invocation-specific
//! closure, premise and authority decisions before any evaluator effects.

use crate::PackageKeyIdentity;

pub trait DeclarationSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool;

    fn package_label(&self, identity: PackageKeyIdentity) -> String;

    /// Complete allowed-owner sets for known requesters, for comparing private
    /// source checkpoints only. A custom predicate may decline persistence.
    /// This metadata never replaces the live predicate during admission.
    fn declaration_selection_scope(
        &self,
    ) -> Option<std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum DeclarationCustody {
    Package(PackageKeyIdentity),
    Toolchain,
    UnownedUser,
    Missing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CompilerEvaluationEffects {
    pub services: Vec<String>,
    pub may_suspend: bool,
    pub may_block: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CompilerEvaluationDeclaration {
    pub name: String,
    pub custody: DeclarationCustody,
    pub effects: CompilerEvaluationEffects,
    /// Body-local checked progress only. Complete invocation termination also
    /// requires every selected callee and each recurrent component edge.
    pub local_terminates: bool,
    pub entry_requires: bool,
    pub linear_carrier: Option<String>,
    pub static_application: Option<String>,
    pub selections: Result<Vec<DeclarationAuthorityRequest>, String>,
}

/// Comparison inputs only. A retained request is discharged exclusively by
/// the invocation's live authority, including custom nonpersistable predicates.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct DeclarationAuthorityRequest {
    pub requester: crate::PackageKeyIdentity,
    pub owner: crate::PackageKeyIdentity,
}

/// One policy for Early Build's inferred facts and retained canonical facts.
/// Neither an empty effect row nor a successful execution supplies a missing
/// closure proof or declaration-selection judgment.
pub fn require_compiler_evaluation_floor(
    machine: &str,
    effects: &CompilerEvaluationEffects,
    closure_violation: Option<&str>,
    selection_violation: Option<&str>,
) -> Result<(), String> {
    let mut violations = Vec::new();
    if !effects.services.is_empty() {
        violations.push(format!("service reach [{}]", effects.services.join(", ")));
    }
    if effects.may_suspend {
        violations.push("may suspend".to_owned());
    }
    if effects.may_block {
        violations.push("may block".to_owned());
    }
    violations.extend(closure_violation.map(str::to_owned));
    violations.extend(selection_violation.map(str::to_owned));
    if violations.is_empty() {
        return Ok(());
    }
    Err(format!(
        "machine `{machine}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
        violations.join("; ")
    ))
}

/// Prevent constant execution while crash safety is unproved. Termination and
/// harmless effects do not establish that an invocation cannot crash. Callers
/// discharge every guarded route using its exact arguments and entry facts.
pub fn require_compiler_evaluation_failures(
    machine: &str,
    failures: Result<(), &str>,
) -> Result<(), String> {
    failures.map_err(|reason| format!(
        "machine `{machine}` is not build-time admissible: {reason}; constant evaluation requires proof that no crash route can occur before execution"
    ))
}

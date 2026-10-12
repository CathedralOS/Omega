//! Exact source mathematical assumptions reached by retained dependency roots.
//!
//! The kernel's original signature owns statement/body references and computes
//! their transitive assumption closure. Interned carrier/denotation assumptions
//! remain kernel coordinates; an authored boundary-let assumption additionally
//! retains its exact source symbol. Canonical original cohort content stays
//! beside this roster; unreached cohort declarations never become roots or
//! admitted assumptions. Neither category is receiving consent.

use arena::Arena;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use super::AssumedBindingMathematicalApplicationUse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingMathematicalRoot {
    pub symbol: SymbolHandle,
    pub declaration_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingMathematicalAssumption {
    pub declaration_index: u32,
    /// Zero denotes an interned kernel assumption, never a source declaration
    /// inferred from its printed mathematical name or statement.
    pub authored_symbol: SymbolHandle,
}

/// Exact closure of the explicitly reached mathematical declaration roots.
/// This is not a claim that every compiler proof judgment has exported its
/// used-root association. The signature and indices stay under Prepared's Arc.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingMathematicalDependencies {
    roots: Vec<AssumedBindingMathematicalRoot>,
    kernel_roots: Arena<u32>,
    assumptions: Vec<AssumedBindingMathematicalAssumption>,
    cohort_content: Option<AssumedBindingMathematicalCohortContent>,
}

/// Canonical content of the original authored mathematical signature cohort,
/// including its interned declarations. Construction is tied to the checked
/// owner, not to a fabricated certificate or a declaration's debug spelling.
/// This does not identify source declarations or certify reached closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingMathematicalCohortContent {
    digest: [u8; 32],
}

impl AssumedBindingMathematicalCohortContent {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

impl AssumedBindingMathematicalDependencies {
    /// Available only when explicit reached roots retained their original
    /// signature owner. The entire cohort can include unreached declarations;
    /// only roots()/assumptions() define the actual dependency roster.
    pub fn cohort_content(&self) -> Option<AssumedBindingMathematicalCohortContent> {
        self.cohort_content
    }

    pub fn roots(&self) -> &[AssumedBindingMathematicalRoot] {
        &self.roots
    }
    /// Exact original signature seeds, including captured domain/operation
    /// references with no authored source declaration. These are owner-local
    /// coordinates, not portable identities or receiving consent.
    pub fn kernel_roots(&self) -> &Arena<u32> {
        &self.kernel_roots
    }
    pub fn assumptions(&self) -> &[AssumedBindingMathematicalAssumption] {
        &self.assumptions
    }
}

pub(super) fn capture(
    program: &CheckedTrees,
    reached: &[SymbolHandle],
    applications: &Arena<AssumedBindingMathematicalApplicationUse>,
) -> Result<AssumedBindingMathematicalDependencies, Diagnostic> {
    let proof = &program.facts.proof;
    let mut inventory = AssumedBindingMathematicalDependencies::default();
    for (ordinal, declaration) in proof.mathematical_declarations.iter().enumerate() {
        if !reached.contains(&declaration.symbol) {
            continue;
        }
        let signature = proof.mathematical_signature.as_ref().ok_or_else(|| {
            Diagnostic::error(
                "assumed binding mathematical dependency has no original checked signature owner",
            )
        })?;
        let declaration_index = *signature.authored.get(ordinal).ok_or_else(|| {
            Diagnostic::error(
                "assumed binding mathematical dependency lost its authored kernel coordinate",
            )
        })?;
        inventory.roots.push(AssumedBindingMathematicalRoot {
            symbol: declaration.symbol,
            declaration_index,
        });
    }
    if inventory.roots.is_empty() && applications.is_empty() {
        return Ok(inventory);
    }
    let signature = proof.mathematical_signature.as_ref().ok_or_else(|| {
        Diagnostic::error(
            "assumed binding mathematical application has no original checked signature owner",
        )
    })?;
    let mut kernel_roots = inventory
        .roots
        .iter()
        .map(|root| root.declaration_index)
        .collect::<std::collections::BTreeSet<_>>();
    for (_, application_use) in applications.iter() {
        let application = proof
            .mathematical_applications
            .get(application_use.application());
        let ordinal = proof
            .mathematical_declarations
            .iter()
            .position(|declaration| declaration.symbol == application.declaration)
            .ok_or_else(|| {
                Diagnostic::error(
                    "assumed binding mathematical application lost its original authored declaration",
                )
            })?;
        let declaration_index = *signature.authored.get(ordinal).ok_or_else(|| {
            Diagnostic::error(
                "assumed binding mathematical application lost its authored kernel coordinate",
            )
        })?;
        kernel_roots.insert(declaration_index);
        let mut terms = vec![application.term, application.result_type];
        terms.extend(
            proof
                .mathematical_application_captures
                .span_or_empty(application.captures)
                .iter()
                .map(|capture| capture.domain),
        );
        kernel_roots.extend(proof_admission::term_declaration_references(
            &signature.arena,
            &terms,
        ));
    }
    // Keep source association for every original authored reference, including
    // higher-order and generic supplied terms. No normalized-term reconstruction
    // or debug spelling supplies a declaration identity.
    for (ordinal, declaration) in proof.mathematical_declarations.iter().enumerate() {
        let Some(&declaration_index) = signature.authored.get(ordinal) else {
            continue;
        };
        if kernel_roots.contains(&declaration_index)
            && !inventory
                .roots
                .iter()
                .any(|root| root.symbol == declaration.symbol)
        {
            inventory.roots.push(AssumedBindingMathematicalRoot {
                symbol: declaration.symbol,
                declaration_index,
            });
        }
    }
    let kernel_roots = kernel_roots.into_iter().collect::<Vec<_>>();
    inventory
        .kernel_roots
        .insert_many(kernel_roots.iter().copied());
    for declaration_index in signature.assumption_dependencies(&kernel_roots) {
        let authored_symbol = signature
            .authored
            .iter()
            .position(|index| *index == declaration_index)
            .and_then(|ordinal| proof.mathematical_declarations.get(ordinal))
            .map_or_else(SymbolHandle::invalid, |declaration| declaration.symbol);
        inventory
            .assumptions
            .push(AssumedBindingMathematicalAssumption {
                declaration_index,
                authored_symbol,
            });
    }
    let encoded = terminal_codec::encode_mathematical_signature(
        &signature.arena,
        &signature.declarations,
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "assumed binding mathematical dependency cannot encode its original signature cohort: {error}"
        ))
    })?;
    inventory.cohort_content = Some(AssumedBindingMathematicalCohortContent {
        digest: crate::digest_encoder::DomainDigestEncoder::content(
            b"omega.assumed-binding-mathematical-cohort.sha256.v1\0",
            &encoded,
        ),
    });
    Ok(inventory)
}

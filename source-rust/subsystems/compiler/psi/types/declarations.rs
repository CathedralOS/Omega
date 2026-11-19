use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeDeclarations {
    pub scalar_qualifications: ScalarQualificationCatalog,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_referents: Vec<types::StructuralReferentIdentity>,
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    pub structural_qualification_changes: Vec<types::StructuralQualificationChange>,
    pub opaque_representations: Vec<crate::TerminalOpaqueRepresentation>,
    pub mathematical_carriers: Vec<TerminalMathematicalCarrier>,
    pub identity_formations: TerminalIdentityFormations,
    pub quotients: quotients::TerminalQuotientCatalog,
}

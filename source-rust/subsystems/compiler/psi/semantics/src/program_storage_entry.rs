//! Recover the granted Extent required by the bundled storage-entry declaration.
//! A matching parameter consumes an existing grant. These checks never introduce one.

use application_identity::type_identity::encoding::{
    CoordinateTag, atom, bare, byte_atom, compound,
};
use application_identity::type_identity::vocabulary::{
    ConstraintForm, Label, NameForm, OwnerForm, TypeForm,
};
use language_core::bundled_files::core_revision;
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    NominalDeclarationIdentity, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalIdentityIndex, TerminalModule,
};

const GRANTED: &str = "Extent::Granted";
pub const SURFACE_GRANT: &str = "toolchain:product::Extent::Granted";

pub(crate) fn bundled_extent_declaration(path: &str) -> String {
    compound(
        NameForm::Nominal,
        [
            byte_atom(OwnerForm::ToolchainSource, &core_revision::EXTENT),
            atom(Label::Path, path),
        ],
    )
}

fn nominal(path: &str) -> String {
    compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(Label::Path, path)],
    )
}

fn carrier() -> String {
    compound(TypeForm::Named, [atom(Label::Name, &nominal("Extent"))])
}

fn domain(name: &str) -> String {
    compound(ConstraintForm::DeclaredDomain, [atom(Label::Name, name)])
}

pub(crate) fn surface_parameter() -> String {
    let ty = compound(TypeForm::Constrained, [carrier(), domain(SURFACE_GRANT)]);
    compound(
        CoordinateTag::Parameter,
        [
            atom(CoordinateTag::Mutable, "no"),
            atom(CoordinateTag::Const, "no"),
            ty,
        ],
    )
}

pub struct ProgramStorageGrant {
    pub declared_type: TerminalIdentityIndex,
    carrier: StructuralTypeId,
    domain: StructuralDomainId,
}

impl ProgramStorageGrant {
    pub fn from_module(
        module: &TerminalModule,
        requirement: TerminalIdentityIndex,
    ) -> Result<Self, &'static str> {
        let identities = &module.identities;
        let name = carrier();
        let owned = compound(
            CoordinateTag::Declaration,
            [
                atom(CoordinateTag::Path, GRANTED),
                compound(CoordinateTag::Owner, [name.clone()]),
            ],
        );
        let domain_identity = identities
            .index_of(&bundled_extent_declaration(&owned))
            .ok_or("storage entry lost its bundled grant domain")?;
        let owner = identities
            .index_of(&bundled_extent_declaration("ProgramStorageEntry::enter"))
            .map(NominalDeclarationIdentity::new)
            .ok_or("storage entry lost the bundled requirement owner")?;
        let mut domains = module
            .types
            .structural_domains
            .iter()
            .filter(|d| d.identity == domain_identity);
        let (Some(domain), None) = (domains.next(), domains.next()) else {
            return Err("storage entry needs a unique granted domain declaration");
        };
        let carrier = module
            .structural_type(domain.carrier)
            .ok_or("storage entry lost the granted carrier")?;
        if !carrier.owns_linear_claim
            || carrier.multiplicity != StructuralMultiplicity::Linear
            || identities.text(carrier.declaration_identity) != Some("toolchain::Extent")
            || identities.text(carrier.identity) != Some(name.as_str())
            || domain.subject_identity != carrier.identity
            || !domain.authorizes_boundary_requirement(requirement, Some(&owner))
        {
            return Err("storage entry grant has a substituted carrier or issuer");
        }
        let checked = compound(
            TypeForm::Constrained,
            [name, self::domain(&self::domain(&nominal(GRANTED)))],
        );
        let declared_type = identities
            .index_of(&checked)
            .ok_or("storage entry lost its declared granted parameter type")?;
        Ok(Self {
            declared_type,
            carrier: carrier.id,
            domain: domain.id,
        })
    }

    pub fn accepts(&self, ordinal: usize, formal: &StructuralParameterDeclaration) -> bool {
        formal.position as usize == ordinal
            && formal.structural_type == self.carrier
            && formal.multiplicity == StructuralMultiplicity::Linear
            && formal.access == StructuralAccess::Owned
            && formal.qualifications == [self.domain]
            && formal.projected_qualifications.is_empty()
            && formal.additional_input_qualifications.is_empty()
    }
}

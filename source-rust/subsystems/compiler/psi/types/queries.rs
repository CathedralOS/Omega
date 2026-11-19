use crate::*;

impl Psi {
    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        find_structural_type(&self.types.structural_types, id)
    }

    pub fn foreign_reference_descriptor(
        &self,
        referent: semantic_vocabulary::StructuralTypeId,
    ) -> Option<bool> {
        if let Some(declaration) = self.structural_type(referent) {
            return Some(matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. }
            ));
        }
        self.types.structural_referents
            .iter()
            .find(|row| row.id == referent)
            .map(|row| row.foreign_descriptor)
    }

    pub fn structural_domain(
        &self,
        id: semantic_vocabulary::StructuralDomainId,
    ) -> Option<&StructuralDomainDeclaration> {
        self.types.structural_domains
            .iter()
            .find(|declaration| declaration.id == id)
    }
}

use crate::LoweringError;
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::Site;
/// Each source declaration kind that has a Psi identity, beside that identity's kind.
const KINDS: [(NodeKind, omega_psi::DeclarationKind); 11] = [
    (NodeKind::Const, omega_psi::DeclarationKind::Const),
    (NodeKind::Data, omega_psi::DeclarationKind::Data),
    (NodeKind::Field, omega_psi::DeclarationKind::Field),
    (NodeKind::Case, omega_psi::DeclarationKind::Case),
    (NodeKind::CaseField, omega_psi::DeclarationKind::CaseField),
    (NodeKind::Domain, omega_psi::DeclarationKind::Domain),
    (NodeKind::Trait, omega_psi::DeclarationKind::Trait),
    (NodeKind::Conformance, omega_psi::DeclarationKind::Conformance),
    (NodeKind::Machine, omega_psi::DeclarationKind::Machine),
    (NodeKind::AsmMachine, omega_psi::DeclarationKind::AsmMachine),
    (NodeKind::MathLet, omega_psi::DeclarationKind::MathLet),
];
/// Map a resolved declaration, including its overload signature, into Psi's identity space.
pub fn portable(id: &DeclarationId) -> Option<omega_psi::DeclarationId> {
    id.try_map_kind(&|kind| KINDS.iter().find(|(source, _)| source == kind).map(|(_, psi)| *psi))
}
/// The source declaration a Psi identity was lowered from. A value the
/// interpreter returns names its data and cases this way.
pub fn source(id: &omega_psi::DeclarationId) -> Option<DeclarationId> {
    id.try_map_kind(&|kind| KINDS.iter().find(|(_, psi)| psi == kind).map(|(source, _)| *source))
}
pub(crate) fn declaration(
    id: &DeclarationId,
    site: &Site,
) -> Result<omega_psi::DeclarationId, LoweringError> {
    portable(id).ok_or_else(|| LoweringError::new(site, "declaration has no portable Psi identity"))
}

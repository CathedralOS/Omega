use crate::Site;
use omega_resolver::DeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingWitness {
    pub site: Site,
    pub machine: DeclarationId,
    pub subject: Site,
    pub view: DeclarationId,
    pub arguments: Vec<Site>,
    pub bounds: Vec<Site>,
    pub inclusive_upper: bool,
}

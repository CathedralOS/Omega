use crate::Site;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Opening {
    pub subject: Site,
    pub fields: Vec<String>,
    pub version: Option<Site>,
}

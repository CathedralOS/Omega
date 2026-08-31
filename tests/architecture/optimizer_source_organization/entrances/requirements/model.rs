pub(crate) struct RequiredCoordinationEntrance {
    pub(crate) path: &'static str,
    pub(crate) coordination_marker: &'static str,
}

pub(crate) struct ExecutableEntranceDomain {
    pub(crate) name: &'static str,
    pub(crate) entrances: &'static [RequiredCoordinationEntrance],
}

pub(crate) struct SemanticLadder {
    pub(crate) family: &'static str,
    pub(crate) paths: &'static [&'static str],
}

pub(crate) struct SemanticLadderDomain {
    pub(crate) name: &'static str,
    pub(crate) ladders: &'static [SemanticLadder],
}

// Domain-separated identities for post-Terminal function fragments and text
// sections.

s!(FunctionFragmentEmissionIdentity => b"omega.terminal-function-fragment-emission-identity.v1\0");
canonical_identity!(
    FunctionFragmentEmissionManifestIdentity,
    b"omega.function-fragment-emission-manifest-identity.v1\0"
);
canonical_identity!(
    TerminalRelocationFreeTextSectionIdentity,
    b"omega.terminal-relocation-free-text-section-identity.v1\0"
);
canonical_identity!(
    FunctionFragmentTextSectionManifestIdentity,
    b"omega.function-fragment-text-section-manifest-identity.v1\0"
);

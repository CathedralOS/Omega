//! Session handle retained while callers migrate off the deleted checked-tree
//! Build cache. An accepted module needs a source-bound reuse key of its own;
//! this handle grants no reuse of an old checked result or invocation authority.

#[derive(Clone, Default)]
pub struct PureBuildCompilation {
    _private: (),
}

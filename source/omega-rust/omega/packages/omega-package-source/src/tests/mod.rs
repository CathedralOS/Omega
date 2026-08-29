//! Responsibility-mirrored resolver regression suite.

use super::*;
#[allow(unused_imports)]
use crate::custody::{lock::*, platform::*, publication::*, tree::*};
#[allow(unused_imports)]
use crate::git::cache::{
    creation::*, custody::*, identity::*, invalidation::*, repository::*, snapshots::*,
};
#[allow(unused_imports)]
use crate::git::executable::{budget::*, custody::*, executor::*, identity::*, selection::*};
#[allow(unused_imports)]
use crate::git::objects::{authentication::*, batch::*, identity::*, tree::*, *};
#[allow(unused_imports)]
use crate::git::process::{capture::*, command::*, identity::*, invocation::*, reconciliation::*};
use crate::git::request::GitExecutionTransport;
#[allow(unused_imports)]
use crate::git::resolve::*;
#[allow(unused_imports)]
use crate::git::snapshot::{
    construction::*, materialization::*, metadata::*, permissions::*, publication::*,
};
use crate::identity::PackageName;
#[allow(unused_imports)]
use crate::limits::*;
#[allow(unused_imports)]
use crate::local::{capture::*, snapshot::*};
#[allow(unused_imports)]
use crate::observations::{accounting::*, execution::*, receipt::*, resolution::*, resolved::*};
use cap_std::ambient_authority;
use cap_std::fs::Dir as CapabilityDirectory;
use omega_resolver_execution::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, ResolverExecutionPhase,
    ResolverExecutionRequestedEndpoint,
};
use sha1_checked::Sha1 as CheckedSha1;
use sha2::Digest;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

mod custody;
mod git;
mod local;
mod objects;
mod process;
mod request;
mod support;

use support::*;

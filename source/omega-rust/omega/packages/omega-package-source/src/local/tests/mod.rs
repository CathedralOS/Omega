//! Local-source behavior tests grouped by resolver responsibility.

use super::{capture::*, model::*, operations::*, snapshot::*};
use crate::error::SourceResolveError;
use crate::git::snapshot::permissions::*;
use crate::identity::*;
use crate::limits::*;
use crate::test_support::*;
use cap_std::ambient_authority;
use cap_std::fs::Dir as CapabilityDirectory;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod capture;
mod identity;
mod limits;
mod observation;
mod path_policy;
mod snapshots;

//! Local-source behavior tests grouped by resolver responsibility.

use super::{model::*, operations::*, snapshot::*};
use crate::error::SourceResolveError;
use crate::identity::*;
use crate::limits::*;
use crate::snapshot::permissions::*;
use crate::test_support::*;
use crate::tree::capture::*;
use crate::tree::filesystem::*;
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

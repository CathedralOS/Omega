//! Local-source behavior tests grouped by resolver responsibility.

use super::{operations::*, resolution_observations::*, snapshot::*};
use crate::package_source::error::SourceResolveError;
use crate::package_source::identity::*;
use crate::package_source::limits::*;
use crate::package_source::snapshot::permissions::*;
use crate::package_source::test_support::*;
use crate::package_source::tree::capture::*;
#[cfg(unix)]
use crate::package_source::tree::filesystem::*;
#[cfg(unix)]
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::Dir as CapabilityDirectory;
use std::collections::BTreeSet;
#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

mod admissions_file;
mod capture;
mod identity;
mod limits;
mod lock_file;
mod observation;
mod path_policy;
mod snapshots;
mod staging;

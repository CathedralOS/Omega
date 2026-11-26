//! Immutable workspace member lists held by the original Build invocation.
use build_declarations::WorkspaceMemberPath;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use std::sync::Mutex;
use terminal_interpreter::{TerminalEffectRejection, TerminalStructuralValue};

const FIRST_SNAPSHOT: u64 = 1 << 59;

struct Member {
    marker: TerminalStructuralValue,
    prior: Option<usize>,
    path: Vec<u8>,
    _storage: Option<EvaluationStorageLease>,
}

#[derive(Default)]
pub(super) struct WorkspaceCatalogs(Mutex<Vec<Member>>);

impl WorkspaceCatalogs {
    pub(super) fn extend(
        &self,
        prior: &TerminalStructuralValue,
        path: &[u8],
        ty: StructuralTypeId,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<TerminalStructuralValue, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let mut members = self
            .0
            .lock()
            .map_err(|_| refuse("workspace catalog lock poisoned"))?;
        let prefix = Self::index(&members, prior);
        if prefix.is_none() && (FIRST_SNAPSHOT..2 * FIRST_SNAPSHOT).contains(&prior.opaque_identity)
        {
            return Err(refuse("workspace prefix was not issued by this Build"));
        }
        let ordinal = u64::try_from(members.len())
            .ok()
            .filter(|n| *n < FIRST_SNAPSHOT)
            .ok_or_else(|| refuse("workspace member space exhausted"))?;
        let bytes =
            u64::try_from(path.len()).map_err(|_| refuse("workspace member path is too large"))?;
        let storage = account
            .map(|account| account.reserve_storage(1, bytes))
            .transpose()
            .map_err(|error| refuse(format!("workspace member snapshot storage: {error:?}")))?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(path.len())
            .map_err(|_| refuse("cannot retain workspace member path"))?;
        owned.extend_from_slice(path);
        let marker = TerminalStructuralValue::minted(FIRST_SNAPSHOT + ordinal, ty, Vec::new());
        members
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain workspace member"))?;
        members.push(Member {
            marker: marker.clone(),
            prior: prefix,
            path: owned,
            _storage: storage,
        });
        Ok(marker)
    }

    fn index(members: &[Member], marker: &TerminalStructuralValue) -> Option<usize> {
        let index = usize::try_from(marker.opaque_identity.checked_sub(FIRST_SNAPSHOT)?).ok()?;
        members
            .get(index)
            .filter(|member| &member.marker == marker)
            .map(|_| index)
    }

    pub(super) fn paths(
        &self,
        marker: &TerminalStructuralValue,
    ) -> Result<Vec<WorkspaceMemberPath>, String> {
        let members = self
            .0
            .lock()
            .map_err(|_| "workspace catalog lock poisoned")?;
        let mut next = Some(
            Self::index(&members, marker)
                .ok_or("workspace catalog was not issued by this Build")?,
        );
        let mut selected = Vec::new();
        let mut unique = BTreeSet::new();
        while let Some(index) = next {
            let member = &members[index];
            let text = std::str::from_utf8(&member.path)
                .map_err(|_| "workspace member path must be UTF-8")?;
            if !unique.insert(text) {
                return Err(format!("Build repeated workspace member `{text}`"));
            }
            selected.push(WorkspaceMemberPath::parse(text).map_err(|error| error.to_string())?);
            next = member.prior;
        }
        selected.reverse();
        Ok(selected)
    }
}

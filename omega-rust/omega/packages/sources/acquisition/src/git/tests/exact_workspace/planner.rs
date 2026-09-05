use super::*;

#[derive(Default)]
pub(super) struct Planner {
    pub(super) discoveries: usize,
    pub(super) selections: usize,
    pub(super) select_undiscovered: bool,
}

impl GitWorkspaceProjectionPlanner for Planner {
    type Error = &'static str;
    type Evidence = &'static str;

    fn discover_members(
        &mut self,
        root: &GitWorkspaceDeclaration,
    ) -> Result<Vec<SourceRelativePath>, Self::Error> {
        self.discoveries += 1;
        if root.bytes() != b"old workspace declaration\n" {
            return Err("root declaration drifted from exact old tree");
        }
        Ok(vec![SourceRelativePath::parse("packages/member").unwrap()])
    }

    fn select_member(
        &mut self,
        root: &GitWorkspaceDeclaration,
        members: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error> {
        self.selections += 1;
        let [member] = members else {
            return Err("one authenticated member expected");
        };
        if root.bytes() != b"old workspace declaration\n"
            || member.bytes() != b"old member declaration\n"
            || member.member_path().map(SourceRelativePath::as_str) != Some("packages/member")
        {
            return Err("member declaration drifted from exact old tree");
        }
        Ok(GitWorkspaceSelection::new(
            SourceRelativePath::parse(if self.select_undiscovered {
                "packages/other"
            } else {
                "packages/member"
            })
            .unwrap(),
            "selected from exact authenticated declarations",
        ))
    }
}

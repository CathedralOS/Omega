//! Resolve dependency helpers against the file imported by the Build being edited.
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, UseItem};

fn file_aliases(trees: &SyntaxTrees) -> impl Iterator<Item = (&str, bool)> {
    trees.root_items().filter_map(|item| {
        let Item::Use(UseItem::File {
            root,
            components,
            alias,
            ..
        }) = item
        else {
            return None;
        };
        let is_dependency = root
            .as_ref()
            .is_some_and(|root| root.as_str(trees) == "omega")
            && matches!(trees.items.identifier_path_members(*components), [dir, file]
                if dir.as_str(trees) == "build" && file.as_str(trees) == "dependency");
        Some((alias.as_str(trees), is_dependency))
    })
}

pub(crate) fn helper<'a>(trees: &SyntaxTrees, target: &'a str) -> Option<&'a str> {
    let (alias, member) = target.strip_prefix('|')?.split_once("|::")?;
    file_aliases(trees)
        .any(|(name, installed)| name == alias && installed)
        .then_some(member)
}

pub(crate) struct DependencyImport {
    pub alias: String,
    insert: bool,
}

impl DependencyImport {
    pub fn for_edit(trees: &SyntaxTrees, required: bool) -> Self {
        if let Some((alias, _)) = file_aliases(trees).find(|(_, installed)| *installed) {
            return Self {
                alias: alias.to_owned(),
                insert: false,
            };
        }
        let alias = std::iter::once("dependency".to_owned())
            .chain((2u32..).map(|n| format!("dependency_{n}")))
            .find(|candidate| !file_aliases(trees).any(|(taken, _)| taken == candidate))
            .expect("a finite file uses finitely many aliases");
        Self {
            alias,
            insert: required,
        }
    }

    fn required_declaration(&self) -> Option<String> {
        self.insert.then(|| {
            let suffix = if self.alias == "dependency" {
                String::new()
            } else {
                format!(" as {}", self.alias)
            };
            format!("use {{omega}}/build/dependency.omg{suffix};")
        })
    }

    pub fn include_in(&self, source: String) -> String {
        let Some(declaration) = self.required_declaration() else {
            return source;
        };
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        format!("{declaration}{newline}{newline}{source}")
    }

    pub fn manual_proposal(&self, statement: String) -> String {
        match self.required_declaration() {
            Some(declaration) => format!(
                "Add at file scope\n{declaration}\n\nAdd inside the Build machine\n{statement}"
            ),
            None => statement,
        }
    }
}

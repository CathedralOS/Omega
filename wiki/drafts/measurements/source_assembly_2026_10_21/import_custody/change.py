from pathlib import Path
root=Path('omega-rust/omega/packages/compilation-inputs/src'); f=root/'source_imports/frontier.rs';s=f.read_text()
s=s.replace('    toolchain_contract_roots: BTreeMap<PathBuf, PathBuf>,\n    toolchain_contract_dirs: Vec<PathBuf>,','    // Import reconciliation borrows custody independently of its mutable results.\n    pub toolchain_contract_custody: ToolchainContractCustody,')
s=s.replace('self.toolchain_contract_roots','self.toolchain_contract_custody.roots').replace('self.toolchain_contract_dirs','self.toolchain_contract_custody.dirs')
a=s.index('    /// Snapshot toolchain contract custody');b=s.index('    fn register_package_source(',a)
s=s[:a]+s[b:]
s=s.replace('self.toolchain_contract_root_for(path)','self.toolchain_contract_custody.root_for(path)')
a=s.index('impl ToolchainContractCustody {');b=s.index('#[derive(Debug, Clone, PartialEq, Eq, Default)]',a)
s=s[:a]+"""impl ToolchainContractCustody {
    pub(crate) fn root_for(&self, path: &Path) -> Option<&Path> {
        self.roots.get(path).map(PathBuf::as_path).or_else(|| {
            self.dirs.iter()
                .find(|dir| path.starts_with(dir.as_path()))
                .map(PathBuf::as_path)
        })
    }
}

"""+s[b:]
f.write_text(s)
f=root/'source_assembly.rs';s=f.read_text().replace('        let contract_custody = source_storage.toolchain_contract_custody();\n','').replace('&contract_custody,','&source_storage.toolchain_contract_custody,');f.write_text(s)

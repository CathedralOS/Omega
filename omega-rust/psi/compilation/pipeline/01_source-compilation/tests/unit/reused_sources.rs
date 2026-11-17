use std::path::PathBuf;
use super::*;
use crate::UnmeasuredProduction;
use source_text::SourceId;
use std::sync::Arc;

fn core_sources() -> Vec<(PathBuf, Arc<str>)> {
    let core =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let mut paths = std::fs::read_dir(&core)
        .expect("bundled core is readable")
        .map(|entry| entry.expect("core entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("core source is UTF-8");
            (path, Arc::from(text))
        })
        .collect()
}

fn loaded(source_id: usize, path: &std::path::Path, source: &Arc<str>) -> LoadedSource {
    LoadedSource {
        source_id: SourceId(source_id),
        path: path.to_path_buf(),
        source: source.clone(),
        ..Default::default()
    }
}

// Discovery parses every unit under private coordinates; admission
// gives each file a different source ID and a later page. The adopted
// parse must be exactly the forest and roots a fresh parse of the
// same bytes would have produced there, for every core construct.
#[test]
fn adopted_discovery_parse_equals_a_fresh_parse_at_admitted_coordinates() {
    let sources = core_sources();
    assert!(sources.len() > 20, "core sources were found");
    let units = SourceInput(
        sources
            .iter()
            .enumerate()
            .map(|(index, (path, text))| loaded(usize::MAX - index, path, text))
            .collect(),
    )
    .into_parsed_units(&mut UnmeasuredProduction)
    .expect("core parses");
    let prefix = Arc::from("data Prefix {}");
    let mut fresh = SyntaxTrees::new(SourceId(500));
    let mut adopted = SyntaxTrees::new(SourceId(500));
    for forest in [&mut fresh, &mut adopted] {
        SourceInput(
            [loaded(500, std::path::Path::new("prefix.omg"), &prefix)]
                .into_iter()
                .collect(),
        )
        .append(forest, &mut UnmeasuredProduction)
        .unwrap();
    }
    for (index, (path, text)) in sources.iter().enumerate().rev() {
        let admitted =
            || -> LoadedSources { [loaded(1000 + index, path, text)].into_iter().collect() };
        let expected = SourceInput(admitted())
            .append(&mut fresh, &mut UnmeasuredProduction)
            .unwrap();
        let mut offered = 0;
        let actual = SourceInput(admitted())
            .append_reusing(&mut adopted, &mut UnmeasuredProduction, |source| {
                offered += 1;
                assert_eq!(source.path, *path);
                Some(&units[index])
            })
            .unwrap();
        assert_eq!(offered, 1);
        assert_eq!(actual, expected, "{}", path.display());
        if path.file_name().is_some_and(|name| name == "extent.omg") {
            // Equality alone would miss both paths dropping mathematical roots.
            for forest in [&fresh, &adopted] {
                let handle = forest
                    .root_mathematical_definition_handles()
                    .iter()
                    .copied()
                    .find(|handle| {
                        forest
                            .root_mathematical_definition(*handle)
                            .name
                            .as_str(forest)
                            == "no_wrap"
                    })
                    .expect("the admitted extent predicate remains a root");
                let definition = forest.root_mathematical_definition(handle);
                assert_eq!(
                    definition.name.source_span().source_id,
                    SourceId(1000 + index)
                );
                assert_eq!(
                    handle.generation(),
                    expected.sources.span_or_empty(expected.batch)[0].root_items[0].generation()
                );
            }
        }
    }
    assert_eq!(adopted, fresh);
    assert_eq!(adopted.page_count(), fresh.page_count());
}

// A retained parse of different bytes is never adopted: the file
// parses afresh, so a changed capture cannot reuse stale syntax.
#[test]
fn retained_parse_of_other_bytes_is_not_adopted() {
    let path = std::path::Path::new("unit.omg");
    let old: Arc<str> = Arc::from("data Old {}");
    let new: Arc<str> = Arc::from("data New { value: u8 }");
    let units = SourceInput([loaded(usize::MAX, path, &old)].into_iter().collect())
        .into_parsed_units(&mut UnmeasuredProduction)
        .unwrap();
    let mut fresh = SyntaxTrees::new(SourceId(3));
    let mut reused = SyntaxTrees::new(SourceId(3));
    let expected = SourceInput([loaded(3, path, &new)].into_iter().collect())
        .append(&mut fresh, &mut UnmeasuredProduction)
        .unwrap();
    let actual = SourceInput([loaded(3, path, &new)].into_iter().collect())
        .append_reusing(&mut reused, &mut UnmeasuredProduction, |_| Some(&units[0]))
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(reused, fresh);
}

use super::*;

#[test]
fn numstat_reads_counts_and_paths() {
    let out = "10\t2\tsrc/main.rs\n-\t-\tassets/demo.png\n0\t0\told.rs => new.rs\n";
    assert_eq!(
        parse_numstat(out),
        vec![
            StagedFile {
                path: "src/main.rs".into(),
                added: 10,
                removed: 2,
            },
            StagedFile {
                path: "assets/demo.png".into(),
                added: 0,
                removed: 0,
            },
            StagedFile {
                path: "old.rs => new.rs".into(),
                added: 0,
                removed: 0,
            },
        ]
    );
}

#[test]
fn numstat_empty_is_nothing_staged() {
    assert_eq!(parse_numstat(""), Vec::new());
}

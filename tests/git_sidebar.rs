use buntrail_core::contracts::*;

#[test]
fn git_sidebar_scope_preserves_raw_references_and_rejects_ambiguous_selectors() {
    for scope in [
        GitHistoryScope::Head,
        GitHistoryScope::All,
        GitHistoryScope::References(vec![b"refs/heads/a-\xff".to_vec()]),
    ] {
        let bytes = serde_json::to_vec(&scope).unwrap();
        assert_eq!(
            serde_json::from_slice::<GitHistoryScope>(&bytes).unwrap(),
            scope
        );
        scope.validate().unwrap();
    }
    for refs in [
        vec![],
        vec![vec![]],
        vec![b"a\0b".to_vec()],
        vec![vec![b'a'; 4097]],
        vec![b"refs/heads/a".to_vec(); 513],
        vec![vec![b'a'; 4096]; 9],
    ] {
        assert!(GitHistoryScope::References(refs).validate().is_err());
    }
    assert!(serde_json::from_str::<GitHistoryScope>(r#"{"All":[],"Head":null}"#).is_err());
}

#[test]
fn git_sidebar_optional_context_is_backward_readable() {
    let old = serde_json::json!({"worktree":{"target":{"security_binding":[],"effective_user":[],"filesystem_namespace":[],"container":null},"canonical_path":{"PosixBytes":[47,114]},"common_git_directory_identity":[]},"state":{"head":{"Unborn":[109]},"index_fingerprint":vec![0;32],"worktree_fingerprint":vec![0;32]},"refs":[],"changes":[],"stashes":[],"remotes":[],"worktrees":[],"merge":null});
    let snapshot: GitRepositorySnapshot = serde_json::from_value(old).unwrap();
    assert_eq!(snapshot.sidebar, None);
    let summary = GitCommitSummary {
        oid: "a".repeat(40),
        changed_file_count: 3,
        insertion_count: 4,
        deletion_count: 5,
        binary_file_count: 1,
    };
    assert_eq!(
        serde_json::from_slice::<GitCommitSummary>(&serde_json::to_vec(&summary).unwrap()).unwrap(),
        summary
    );
}

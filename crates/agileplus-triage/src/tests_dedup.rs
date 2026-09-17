// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the new dedup / claim / repo_introspect modules.
//!
//! Run with `cargo test -p agileplus-triage`.

#[cfg(test)]
mod tests {
    use crate::claim::*;
    use crate::dedup::*;
    use crate::repo_introspect::*;
    use chrono::Utc;

    #[test]
    fn token_jaccard_identical() {
        assert!((token_jaccard("hello world", "hello world") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn token_jaccard_disjoint() {
        assert_eq!(token_jaccard("foo bar", "baz qux"), 0.0);
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn fuzzy_ratio_perfect() {
        assert!((fuzzy_ratio("hello", "hello") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ngram_jaccard_works() {
        assert!(ngram_jaccard("abcdef", "abcxyf", 3) > 0.0);
    }

    #[test]
    fn simhash_similar_texts_low_distance() {
        let a = simhash64("implement auth flow");
        let b = simhash64("implement authentication flow");
        let d = simhash_distance(a, b);
        assert!(d < 20, "expected low distance, got {}", d);
    }

    #[test]
    fn hybrid_score_high_for_similar() {
        let (s, _, _, _, _) = hybrid_score("add login button", "add login form");
        assert!(s > 0.5, "expected >0.5, got {}", s);
    }

    #[test]
    fn hybrid_score_low_for_different() {
        let (s, _, _, _, _) = hybrid_score("add login button", "remove old cache");
        assert!(s < 0.3, "expected <0.3, got {}", s);
    }

    #[test]
    fn find_duplicates_threshold() {
        let items = vec![
            ("a".into(), "add login button to header".into()),
            ("b".into(), "add login form to header".into()),
            ("c".into(), "completely unrelated content".into()),
        ];
        let dups = find_duplicates(&items, 0.5);
        assert!(dups.iter().any(|c| c.a_id == "a" && c.b_id == "b"));
        assert!(!dups.iter().any(|c| {
            (c.a_id == "a" && c.b_id == "c") || (c.a_id == "c" && c.b_id == "a")
        }));
    }

    #[test]
    fn claim_issue_and_release() {
        let mut s = ClaimStore::new();
        let c1 = s.claim("c1", "repo:foo", ClaimKind::Repo, "agent-a", 60, ClaimReason::default());
        assert!(c1.is_some());
        let c2 = s.claim("c2", "repo:foo", ClaimKind::Repo, "agent-b", 60, ClaimReason::default());
        assert!(c2.is_none());
        assert!(s.release("c1"));
        let c3 = s.claim("c3", "repo:foo", ClaimKind::Repo, "agent-b", 60, ClaimReason::default());
        assert!(c3.is_some());
    }

    #[test]
    fn claim_heartbeat_prevents_expiry() {
        let mut s = ClaimStore::new();
        s.claim("c1", "branch:feat", ClaimKind::Branch, "agent-a", 1, ClaimReason::default());
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let reaped_before = s.reap_expired(Utc::now());
        assert_eq!(reaped_before, 1);
    }

    #[test]
    fn claim_heartbeat_refreshes() {
        let mut s = ClaimStore::new();
        s.claim("c1", "branch:feat", ClaimKind::Branch, "agent-a", 1, ClaimReason::default());
        std::thread::sleep(std::time::Duration::from_millis(500));
        s.heartbeat("c1");
        std::thread::sleep(std::time::Duration::from_millis(700));
        let reaped = s.reap_expired(Utc::now());
        assert_eq!(reaped, 0);
    }

    #[test]
    fn claim_lookup_by_resource() {
        let mut s = ClaimStore::new();
        s.claim("c1", "wt:/tmp/wt1", ClaimKind::Worktree, "agent-a", 60, ClaimReason::default());
        let found = s.lookup(ClaimKind::Worktree, "wt:/tmp/wt1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().agent_id, "agent-a");
    }

    #[test]
    fn inspect_repo_no_git() {
        let tmp = std::env::temp_dir().join("agileplus_introspect_test_nogit");
        let _ = std::fs::create_dir_all(&tmp);
        let info = inspect_repo(&tmp);
        assert_eq!(info.state, RepoState::NoGit);
        assert!(info.hygiene_score >= 30);
    }

    #[test]
    fn inspect_repo_mangled_git() {
        let tmp = std::env::temp_dir().join("agileplus_introspect_test_mangled");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        // No HEAD file on purpose -> mangled.
        let info = inspect_repo(&tmp);
        assert_eq!(info.state, RepoState::MangledGit);
        assert_eq!(info.hygiene_score, 50);
    }

    #[test]
    fn inspect_repo_valid_git() {
        let tmp = std::env::temp_dir().join("agileplus_introspect_test_valid");
        let _ = std::fs::remove_dir_all(&tmp);
        let git = tmp.join(".git");
        std::fs::create_dir_all(git.join("refs/heads")).unwrap();
        std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let info = inspect_repo(&tmp);
        assert_eq!(info.state, RepoState::Git);
        assert_eq!(info.current_branch.as_deref(), Some("main"));
        assert_eq!(info.hygiene_score, 100);
    }
}

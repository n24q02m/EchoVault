import sys

with open("apps/core/src/storage/vault_db.rs", "r") as f:
    content = f.read()

new_test = """
    #[test]
    fn test_get_all_session_mtimes() -> Result<()> {
        let mut db = VaultDb::open_in_memory()?;

        let sessions = vec![
            create_test_session("s1", 1000),
            create_test_session("s2", 2000),
            create_test_session("s3", 3000),
        ];

        db.upsert_batch(&sessions)?;

        let mtimes = db.get_all_session_mtimes()?;
        assert_eq!(mtimes.len(), 3);
        assert_eq!(mtimes.get("s1"), Some(&1000));
        assert_eq!(mtimes.get("s2"), Some(&2000));
        assert_eq!(mtimes.get("s3"), Some(&3000));
        assert_eq!(mtimes.get("s4"), None);

        Ok(())
    }
}
"""

# Find the closing brace of the module  (which is at the very end of file likely)
# Wait, the file ends with closing brace of .
# Let's check the end of file.

lines = content.splitlines()
# Remove the last closing brace and append the test, then add closing brace back
# Or search for  and find its closing brace.

# Assuming the file ends with closing brace for  block.
# Let's verify by checking the last few lines.
pass

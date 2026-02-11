# EchoVault Testing Guide

Quy trinh test hoan chinh truoc khi release. Moi release phai pass TAT CA cac buoc duoi day.

---

## 1. Automated Tests (CI)

CI chay tu dong tren moi push/PR vao `dev` va `main`. Phai pass 100%.

### 1.1 Rust Tests

```bash
cargo test --workspace
```

| Test Suite | File | Mo ta |
|------------|------|-------|
| Parser fixtures | `apps/core/tests/parser_tests.rs` | VS Code Copilot V1/V3, Zed, JetBrains, Codex |
| Registry checks | `parser_tests.rs::registry` | 11 extractors = 11 parsers, names match |
| Config migration | `apps/core/src/config.rs` | v1 -> v2 migration, defaults |
| Cosine similarity | `apps/core/src/embedding/provider.rs` | Math correctness |
| Conversation utils | `parser_tests.rs::conversation_utils` | is_empty, count_by_role |

### 1.2 Frontend

```bash
cd apps/web
pnpm typecheck   # TypeScript compilation
pnpm lint         # Biome lint
```

### 1.3 Rust Lint

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo audit
```

---

## 2. Pre-Release Checklist (Manual)

Chay TRUOC khi promote `dev` -> `main`.

### 2.1 Extract

```bash
echovault-cli extract
```

- [ ] Khong crash, khong panic
- [ ] Output hien thi so sessions tim duoc
- [ ] Thu muc `vault/sessions/` duoc tao voi cac subdirectories (vscode-copilot, cursor, v.v.)

### 2.2 Parse

```bash
echovault-cli parse
```

- [ ] Khong crash
- [ ] Output hien thi so sessions parsed
- [ ] Thu muc `vault/parsed/` chua cac file `.md`
- [ ] Mo 1-2 file `.md` kiem tra: co YAML frontmatter, co messages user/assistant

### 2.3 Embed (yeu cau Ollama hoac OpenAI)

```bash
# Khong co embedding provider
echovault-cli embed
# -> Expect: Error message "Cannot connect to embedding provider..."

# Co Ollama
ollama pull nomic-embed-text
echovault-cli embed
```

- [ ] Khi KHONG co provider: hien thi error message than thien, KHONG crash
- [ ] Khi CO provider: embed thanh cong, hien thi so chunks
- [ ] File `vault/embeddings.db` duoc tao

### 2.4 Search

```bash
echovault-cli search "test query" --limit 5
```

- [ ] Tra ve results voi relevance score
- [ ] Khong crash khi chua co embeddings (hien thi message phu hop)

### 2.5 MCP Server

```bash
echo '{"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{}},"id":1}' | echovault-cli mcp
```

- [ ] Tra ve JSON response hop le
- [ ] Khong hang, khong crash

### 2.6 Desktop App (Tauri)

```bash
cargo tauri dev
```

- [ ] App khoi dong khong crash
- [ ] Sessions tab: hien thi danh sach sessions tu cac sources
- [ ] Click vao session: hien thi noi dung trong text editor
- [ ] Settings overlay: mo/dong duoc
- [ ] Embedding Provider:
  - [ ] Chon preset (Ollama/OpenAI/Custom) -> fields update dung
  - [ ] Ollama model dropdown hien thi khi Ollama detected
  - [ ] Save & Test -> auto test connection
  - [ ] Hien thi trang thai ket noi (xanh/do)
- [ ] Search tab:
  - [ ] Build Index KHI CHUA config embedding -> hien thi error toast, KHONG crash
  - [ ] Build Index khi CO config -> embed thanh cong
  - [ ] Search query tra ve results
  - [ ] Empty state hien thi huong dan 3 buoc khi chua co index

### 2.7 Cross-Platform

CI chay tren 3 OS (ubuntu, windows, macos). Neu co the, test thu cong tren OS khac.

- [ ] `cargo test --workspace` pass tren Linux
- [ ] `cargo test --workspace` pass tren Windows (CI)
- [ ] `cargo test --workspace` pass tren macOS (CI)

---

## 3. Release Workflow

### 3.1 Beta Release (dev branch)

1. Push to `dev` -> CI chay tu dong
2. Neu CI pass -> CD tu dong tao beta release
3. Test beta release tren may local (download + cai dat)

### 3.2 Stable Release (main branch)

1. Chay Pre-Release Checklist (Section 2) tren may local
2. Su dung CD workflow: `promote-to-stable`
3. Review PR `dev -> main`
4. Merge PR -> CD tao stable release
5. Verify: download stable release, cai dat, test co ban

### 3.3 Rollback

Neu stable release co loi:
1. KHONG xoa release
2. Chay `gh release edit <tag> --prerelease` de danh dau la pre-release
3. Edit release notes: them "[WITHDRAWN] - Known issues: ..."
4. Fix tren `dev`, release stable moi

---

## 4. Adding New Tests

### 4.1 Parser Test (fixture-based)

1. Tao fixture file trong `apps/core/tests/fixtures/`
2. Them test module trong `apps/core/tests/parser_tests.rs`
3. Pattern: parse fixture -> assert fields (id, source, messages, title)

```rust
mod new_source {
    use super::*;
    use echovault_core::parsers::new_source::NewSourceParser;

    #[test]
    fn test_basic_parse() {
        let parser = NewSourceParser;
        let path = fixture_path("new_source_sample.json");
        let conv = parser.parse(&path).expect("parse failed");
        assert_eq!(conv.source, "new-source");
        assert!(!conv.is_empty());
    }
}
```

### 4.2 Extractor Test

Extractors kho test tu dong vi phu thuoc file system thuc. Dung registry tests:

```rust
#[test]
fn test_new_extractor_in_registry() {
    let extractors = extractors::all_extractors();
    assert!(extractors.iter().any(|e| e.source_name() == "new-source"));
}
```

### 4.3 E2E Test (Playwright)

```bash
cd apps/web
pnpm exec playwright test
```

Xem `apps/web/tests/` de tham khao pattern mock Tauri IPC.

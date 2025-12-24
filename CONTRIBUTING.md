# Contributing to EchoVault

Thank you for your interest in contributing to EchoVault!

## Getting Started

### Prerequisites

- **mise** (recommended) or Rust 1.70+, Node.js 22+, pnpm
- Git

### Setup Development Environment

```bash
git clone https://github.com/n24q02m/EchoVault
cd EchoVault
pnpm setup
```

This will install all dependencies including Rust toolchain, Node.js, and pnpm.

## Development Workflow

### Running Locally

```bash
# Development mode with hot reload
cargo tauri dev

# Build production
cargo tauri build
```

### Making Changes

1. Create a branch: `git checkout -b feature/your-feature`
2. Make your changes
3. Run checks: `cargo clippy && cargo fmt --check`
4. Run tests: `cargo test --workspace`
5. Lint frontend: `pnpm --filter web lint`
6. Commit (Conventional Commits): `feat: add feature`
7. Push to your fork: `git push origin feature/your-feature`
8. Open a Pull Request

## Commit Convention

We use [Conventional Commits](https://www.conventionalcommits.org/):

| Type       | Description                        |
| ---------- | ---------------------------------- |
| `feat`     | New feature                        |
| `fix`      | Bug fix                            |
| `docs`     | Documentation changes              |
| `style`    | Code style (formatting, no logic)  |
| `refactor` | Code refactoring                   |
| `perf`     | Performance improvements           |
| `test`     | Adding or updating tests           |
| `chore`    | Maintenance tasks, dependencies    |
| `ci`       | CI/CD changes                      |

### Examples

```text
feat: add support for Claude extractor
fix: handle corrupted SQLite database gracefully
docs: update README with installation instructions
test: add integration tests for Cursor extractor
```

## Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Update documentation if needed
- Add tests for new functionality
- Ensure all checks pass

### PR Checklist

- [ ] Code follows Rust best practices
- [ ] All tests pass (`cargo test --workspace`)
- [ ] Clippy passes (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt --check`)
- [ ] Commit messages follow Conventional Commits
- [ ] Documentation updated (if needed)

## Code Style

### Rust

- Format with `cargo fmt`
- Lint with `cargo clippy`
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### TypeScript (Frontend)

- Format and lint with Biome (`pnpm --filter web lint`)
- Use TypeScript strict mode

## Project Structure

```text
EchoVault/
├── apps/
│   ├── core/           # Core Rust library
│   │   ├── extractors/ # Chat extractors (Copilot, Cursor, Cline, Antigravity)
│   │   ├── storage/    # SQLite index and vault database
│   │   ├── sync/       # Rclone sync engine
│   │   └── utils/      # Utilities
│   ├── tauri/          # Tauri desktop app backend
│   └── web/            # React frontend
├── scripts/            # Development scripts
└── .github/            # CI/CD workflows
```

## Adding a New Extractor

1. Create a new file in `apps/core/src/extractors/`
2. Implement the `Extractor` trait
3. Register in `apps/core/src/extractors/mod.rs`
4. Add to `apps/tauri/src/commands.rs`

See existing extractors for examples.

## Questions?

Feel free to open an issue for:

- Bug reports
- Feature requests
- Questions about the codebase
- Discussion about architecture

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

**Thank you for contributing to EchoVault! 🎉**

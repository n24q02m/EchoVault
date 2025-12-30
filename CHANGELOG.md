## [1.8.6](https://github.com/n24q02m/EchoVault/compare/v1.8.5...v1.8.6) (2025-12-30)


### Bug Fixes

* **bundle:** Disable symbol stripping to fix AppImage corruption ([6abaeb1](https://github.com/n24q02m/EchoVault/commit/6abaeb14f3d22e98e5008b150db8ff4bdf7bdb38))

## [1.8.5](https://github.com/n24q02m/EchoVault/compare/v1.8.4...v1.8.5) (2025-12-29)


### Bug Fixes

* Specify ubuntu-22.04 runner for GitHub Actions workflow instead of ubuntu-latest. ([9fe864c](https://github.com/n24q02m/EchoVault/commit/9fe864cced5791c0d5a3d14eb7439c08bf3fc8df))

## [1.8.4](https://github.com/n24q02m/EchoVault/compare/v1.8.3...v1.8.4) (2025-12-29)


### Bug Fixes

* **ci:** Downgrade Linux runner to ubuntu-22.04 for broader GLIBC compatibility ([9ab5984](https://github.com/n24q02m/EchoVault/commit/9ab598419809599e4c70839f0237413a2d481177))

## [1.8.3](https://github.com/n24q02m/EchoVault/compare/v1.8.2...v1.8.3) (2025-12-29)


### Bug Fixes

* Correct version placeholder in tauri.conf.json for proper release naming ([d2a5bf8](https://github.com/n24q02m/EchoVault/commit/d2a5bf8715c2b38630c7e8a1938a568112370e64))

## [1.8.2](https://github.com/n24q02m/EchoVault/compare/v1.8.1...v1.8.2) (2025-12-26)


### Bug Fixes

* Correct package name in uninstall instructions in README. ([d7aa7df](https://github.com/n24q02m/EchoVault/commit/d7aa7df363e0fc656886fcbfff6b7c21eca9f64f))

## [1.8.1](https://github.com/n24q02m/EchoVault/compare/v1.8.0...v1.8.1) (2025-12-26)


### Bug Fixes

* **ci:** use GITHUB_WORKSPACE for vault path in sync tests ([b9ebdcc](https://github.com/n24q02m/EchoVault/commit/b9ebdcc7d7d83a074035d99f3d5eb561c4ae7e4f))

# [1.8.0](https://github.com/n24q02m/EchoVault/compare/v1.7.0...v1.8.0) (2025-12-26)


### Features

* Add multi-machine sync tests to CI to verify conflict resolution. ([541a895](https://github.com/n24q02m/EchoVault/commit/541a89561b3fb3ac4339d595542445eb0d8267cd))

# [1.7.0](https://github.com/n24q02m/EchoVault/compare/v1.6.0...v1.7.0) (2025-12-26)


### Features

* introduce `mise run setup` task for automated development environment provisioning and update README. ([2730302](https://github.com/n24q02m/EchoVault/commit/27303023219a673859772ead63185b19fa3e71d4))

# [1.6.0](https://github.com/n24q02m/EchoVault/compare/v1.5.0...v1.6.0) (2025-12-25)


### Features

* official release polish ([e8dfb66](https://github.com/n24q02m/EchoVault/commit/e8dfb66b1ad50ebe42e025062e2b046da38b7c2d))

# [1.5.0](https://github.com/n24q02m/EchoVault/compare/v1.4.0...v1.5.0) (2025-12-24)


### Features

* migrate logging from println to tracing ([d7469b4](https://github.com/n24q02m/EchoVault/commit/d7469b44598db30f6d24e07abb65c2c10075c25b))

# [1.4.0](https://github.com/n24q02m/EchoVault/compare/v1.3.0...v1.4.0) (2025-12-24)


### Features

* improve concurrency and error handling ([18bcc0e](https://github.com/n24q02m/EchoVault/commit/18bcc0e230e7b9c4975dca5b82b4d60c92886275))

# [1.3.0](https://github.com/n24q02m/EchoVault/compare/v1.2.0...v1.3.0) (2025-12-24)


### Features

* prepare for public release ([ec13b3e](https://github.com/n24q02m/EchoVault/commit/ec13b3e701d465f939bd85b6bae580b1f319a9ae))

# [1.2.0](https://github.com/n24q02m/EchoVault/compare/v1.1.1...v1.2.0) (2025-12-19)


### Features

* include source in vault session index and use it for session identification and grouping ([d20e18c](https://github.com/n24q02m/EchoVault/commit/d20e18c1367bed52babe58e76adf70f5519047ae))

## [1.1.1](https://github.com/n24q02m/EchoVault/compare/v1.1.0...v1.1.1) (2025-12-18)


### Bug Fixes

* Add directory creation for rclone binaries on Windows release workflow. ([5cbd781](https://github.com/n24q02m/EchoVault/commit/5cbd781d61b89b01d41bfb7b99b3fd8518bfdfe7))

# [1.1.0](https://github.com/n24q02m/EchoVault/compare/v1.0.3...v1.1.0) (2025-12-18)


### Features

* Add macOS to CI and release workflows, including Rclone binary setup. ([2a41029](https://github.com/n24q02m/EchoVault/commit/2a41029b325f51b714fe78326c743cb8ad5a4cf5))
* Implement CI/CD workflows, refactor project structure, and add core watcher module with automated Rclone download. ([b920727](https://github.com/n24q02m/EchoVault/commit/b920727b41d54ec9068aec64aedf0a45f143b9cc))
* implement Cline (Claude Dev) VS Code extension extractor and enhance release workflow with new build steps and dependencies ([054dd12](https://github.com/n24q02m/EchoVault/commit/054dd12ac381c4615bc965c2e22c844a28753d20))
* Implement cursor extractor and simplify web UI by removing the settings tab. ([01f1a9e](https://github.com/n24q02m/EchoVault/commit/01f1a9e2e89cd52610674c32997463fc40d21548))
* Update README for clarity, add setup script, and remove logo processing script ([b02f077](https://github.com/n24q02m/EchoVault/commit/b02f077332e255ee1323ea93fd27fb48298e4c51))

## [1.0.3](https://github.com/n24q02m/EchoVault/compare/v1.0.2...v1.0.3) (2025-12-11)


### Bug Fixes

* move pre-commit installation instructions to the development section in README.md ([78b9246](https://github.com/n24q02m/EchoVault/commit/78b92464fefc42244968b8a7dc52f5c412346077))

## [1.0.2](https://github.com/n24q02m/EchoVault/compare/v1.0.1...v1.0.2) (2025-12-10)


### Bug Fixes

* update Mise configuration and pre-commit commands for EchoVault ([29c623a](https://github.com/n24q02m/EchoVault/commit/29c623a7ddf9812b68d9a8f784ee5a237453cba6))

## [1.0.1](https://github.com/n24q02m/EchoVault/compare/v1.0.0...v1.0.1) (2025-12-10)


### Bug Fixes

* add permissions for issues in release workflow ([e23b694](https://github.com/n24q02m/EchoVault/commit/e23b694b98885047e011c1501ce10c50df5ac5c8))

# 1.0.0 (2025-12-10)


### Bug Fixes

* conflict pnpm config ([d980fd4](https://github.com/n24q02m/EchoVault/commit/d980fd4259fa94ac7486792b5099d1b81bc12fcc))
* disable sematic-release-cargo's publish ([9dbe529](https://github.com/n24q02m/EchoVault/commit/9dbe529e7456082f7448b73a5b3b9c65e314cf8f))
* Improve git pull failure handling by prioritizing remote vault.json and .gitignore while preserving local sessions. ([d0065d3](https://github.com/n24q02m/EchoVault/commit/d0065d373544b93fa709c2fb140a50271f06e30e))
* Improve vault session detection for Antigravity artifacts with slash in ID ([89e1449](https://github.com/n24q02m/EchoVault/commit/89e1449a5e1d73830ad00b83bfa433491f54c0a3))
* missing package.json ([1d178a3](https://github.com/n24q02m/EchoVault/commit/1d178a3188d3de53158d250fad540aa0263ea96f))
* update lint and format commands to use Biome ([900d133](https://github.com/n24q02m/EchoVault/commit/900d133d2e9d7603fc8d42e1a7670932ba89091e))
* update Windows instructions for rclone extraction and renaming ([72ffb5f](https://github.com/n24q02m/EchoVault/commit/72ffb5fb6f1274d841d6ccd649a8835101de6c41))


### Features

* Add .gitignore file to exclude temporary files and update developer handbook version and content ([fad1b5e](https://github.com/n24q02m/EchoVault/commit/fad1b5e0b257b78e16173772001cb99216bd1b9c))
* Add Antigravity artifact extraction and Markdown syntax highlighting to the text editor ([7f3d66b](https://github.com/n24q02m/EchoVault/commit/7f3d66bbfce46e65966f3bf23455b4b3a304d8dd))
* Add Antigravity data extractor with Windows/WSL support, update Windows setup instructions in README, and include new handbook and Windows schema. ([b51983f](https://github.com/n24q02m/EchoVault/commit/b51983f5baed1a77f0b978ff5b2e439193687554))
* Add Antigravity extractor and integrate it into session scanning and ingestion. ([9655993](https://github.com/n24q02m/EchoVault/commit/9655993db902d3b2ada35c07b5bd82e92b9fd0dc))
* Add binary alias 'ev' for EchoVault commands and enhance mise tasks for development ([18f69fb](https://github.com/n24q02m/EchoVault/commit/18f69fb421f7ba00ebbdcaf0a6e41f79eb85a4a7))
* Add initial project handbook, agent rules, and context instructions. ([df7d317](https://github.com/n24q02m/EchoVault/commit/df7d317da7015dc3c1de5dbbf955d636f514f4b4))
* Add vault-synced sessions to `scan_sessions` and implement remote pull in `sync_vault`. ([911dde8](https://github.com/n24q02m/EchoVault/commit/911dde8a0351b9c32d4bb0857b3a47e87ff58fd8))
* Enhance `scan_sessions` to include sessions from local extractors and the vault index. ([bc0d8c5](https://github.com/n24q02m/EchoVault/commit/bc0d8c57b391e5e4566d393067db383b77355e54))
* Enhance EchoVault with GitHub repository auto-setup and improved sync functionality ([c987f96](https://github.com/n24q02m/EchoVault/commit/c987f968517654a775b1fc5361351262ac99762e))
* enhance Git push with auto-rebase on rejection, enforce encryption and compression for GitHub, and add copy to clipboard for authentication code. ([0dda703](https://github.com/n24q02m/EchoVault/commit/0dda703ba31a31f0de69d759a5bcc823421ca99d))
* Enhance GitHub sync functionality with repository existence check and improved OAuth credential handling ([36c8623](https://github.com/n24q02m/EchoVault/commit/36c862396db74b0d07618786ac967e638dc3ab85))
* Enhance push conflict resolution in `push_with_pull` by adding rebase abort, hard reset, and merge --theirs strategies. ([4df097f](https://github.com/n24q02m/EchoVault/commit/4df097f29dfc239249dfc812f30f7babd9f8e3c2))
* Enhance README and HANDBOOK with installation instructions and update extractor functionality for parallel processing ([1c55c56](https://github.com/n24q02m/EchoVault/commit/1c55c562562d792fbbe3f035fed719e3f3f1aaf5))
* Implement a text editor component using CodeMirror and a new backend command for reading file content. ([fc13a38](https://github.com/n24q02m/EchoVault/commit/fc13a389d816952361408acf2c2246e0fed98d43))
* Implement chunked storage with compression for large files and enhance GitHub sync functionality ([0fcb358](https://github.com/n24q02m/EchoVault/commit/0fcb3582e9f9184d6e30d3cbafd49ce0938637c0))
* Implement Google Drive synchronization and abstract sync providers for multi-provider support. ([3fdd383](https://github.com/n24q02m/EchoVault/commit/3fdd383674f37110046a197b3ecf8a54d946389c))
* Implement initial setup flow with GitHub sync and OAuth device authentication. ([38d9040](https://github.com/n24q02m/EchoVault/commit/38d9040ae8553abe52bf6b35e1040530aa2a0ec6))
* implement vault synchronization with Git/GitHub, add sync UI with session caching, and integrate `init_provider` command. ([0428975](https://github.com/n24q02m/EchoVault/commit/0428975b7199edfeb2d51397e83b95cbc9abdd31))
* Improve GitHub sync functionality with automatic repository creation and enhanced error handling ([b5ba2ec](https://github.com/n24q02m/EchoVault/commit/b5ba2ec94a228190545fe546efc3da542c87e84a))
* Initialize EchoVault project with CLI for extracting AI chat history ([4914759](https://github.com/n24q02m/EchoVault/commit/4914759cd683a502f533b8774c12c93c645ac486))
* initialize echovault Rust crate and add GUI-related Linux dependencies to CI workflow. ([d11254d](https://github.com/n24q02m/EchoVault/commit/d11254d35aac0fd1948cfc5b739130fc1f7ac923))
* Introduce Tauri application with web frontend and refactor Rust core into a dedicated crate. ([499b31f](https://github.com/n24q02m/EchoVault/commit/499b31f3c079e46644f721c7b7dd35c674980aff))
* Migrate web frontend to Biome for linting and formatting, simplify core browser and storage path logic, and add Tauri sync event. ([b1d6565](https://github.com/n24q02m/EchoVault/commit/b1d656553f2b63d05eee36bc2883056db0613c72))
* prioritize text editors for Linux file opening and update README run command. ([8f6766d](https://github.com/n24q02m/EchoVault/commit/8f6766dee53a16ff4d9b9cc9929d8ea61f6f05fc))
* replace periodic sync with real-time file watcher for IDE directories and event-driven frontend synchronization. ([c48c731](https://github.com/n24q02m/EchoVault/commit/c48c731b443c2af6132d9d8a9cde43e63e8c2a79))
* update ([696f637](https://github.com/n24q02m/EchoVault/commit/696f637756fbebc2de58c6cbbd58de1615035c2a))
* Update application logos and icons, including new SVG and transparent PNG assets. ([581d09e](https://github.com/n24q02m/EchoVault/commit/581d09eb3bb5d93300ae494bf15317be57e40b90))
* Update handbook to reflect hybrid storage solution and enhance data handling details ([19b80c7](https://github.com/n24q02m/EchoVault/commit/19b80c758949dd5589d777ab03d2437fb22b42a6))

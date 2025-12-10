# Changelog

## [0.2.0](https://github.com/n24q02m/EchoVault/compare/echovault-v0.1.0...echovault-v0.2.0) (2025-12-10)


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


### Bug Fixes

* Improve git pull failure handling by prioritizing remote vault.json and .gitignore while preserving local sessions. ([d0065d3](https://github.com/n24q02m/EchoVault/commit/d0065d373544b93fa709c2fb140a50271f06e30e))
* Improve vault session detection for Antigravity artifacts with slash in ID ([89e1449](https://github.com/n24q02m/EchoVault/commit/89e1449a5e1d73830ad00b83bfa433491f54c0a3))
* missing package.json ([1d178a3](https://github.com/n24q02m/EchoVault/commit/1d178a3188d3de53158d250fad540aa0263ea96f))


### Refactoring

* Refactor EchoVault CLI to copy raw JSON files without formatting ([b50324c](https://github.com/n24q02m/EchoVault/commit/b50324cfec2626eec6d4f845b3adfa7a0adaf49a))
* Simplify codebase by removing encryption, compression, and GitHub provider. Now using Rclone as the only sync provider with a simplified 2-step setup flow. ([1c353ad](https://github.com/n24q02m/EchoVault/commit/1c353ad63cab8dbe1cd2d2eb9aa0cb30798fc06a))
* Simplify print statements and enhance OAuth error handling with new fields and dynamic polling intervals ([793c766](https://github.com/n24q02m/EchoVault/commit/793c76693ed041e207a3c43ba9762a08a7666ffe))


### Documentation

* Add pre-commit install step to development setup instructions. ([d69a8f5](https://github.com/n24q02m/EchoVault/commit/d69a8f5039ab2a51d198280139abb235f4e75d20))
* Centralize project context by removing redundant details and directing to the developer handbook. ([0fa4563](https://github.com/n24q02m/EchoVault/commit/0fa456366fb74b32b0f95ae48df48cbe435bdfb9))
* Mark Antigravity as supported, update app run command to release build, and revise handbook version with improved credential clearing instructions. ([ce7844c](https://github.com/n24q02m/EchoVault/commit/ce7844c8618a01071b8df8244aa0dc3f545aff07))
* Update installation instructions for frontend setup and clarify pre-commit hooks installation ([b37183f](https://github.com/n24q02m/EchoVault/commit/b37183fa6833db6cd06d5a511a3cc076e893b021))
* update installation instructions to include Tauri CLI and frontend build step. ([2d216ce](https://github.com/n24q02m/EchoVault/commit/2d216ceb4cbba19d62ce6366ae1ad311767e43a1))
* update installation instructions to remove frontend build steps and streamline app setup ([36e55a3](https://github.com/n24q02m/EchoVault/commit/36e55a37aa3057e4dde025ddf571d1fa3460624a))

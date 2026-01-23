#!/usr/bin/env node
/**
 * Build Rust workspace with fallback to CLI-only if Tauri build fails.
 *
 * This script attempts to build the full workspace (including Tauri).
 * If that fails (e.g., missing Tauri dependencies on older Linux),
 * it falls back to building only the CLI.
 */

import { execSync, spawnSync } from "node:child_process";

const isWindows = process.platform === "win32";

function run(command, options = {}) {
    console.log(`> ${command}`);
    try {
        execSync(command, {
            stdio: "inherit",
            shell: isWindows ? "cmd.exe" : "/bin/sh",
            ...options,
        });
        return true;
    } catch {
        return false;
    }
}

function main() {
    console.log("Building Rust workspace...\n");

    // Try full build (includes Tauri)
    const fullBuildSuccess = run("cargo build");

    if (fullBuildSuccess) {
        console.log("\n✓ Full build successful (Tauri + CLI)");
        return;
    }

    // Full build failed, try CLI only
    console.log("\n⚠ Full build failed (likely missing Tauri dependencies)");
    console.log("Building CLI only...\n");

    const cliBuildSuccess = run("cargo build -p echovault-core -p echovault-cli");

    if (cliBuildSuccess) {
        console.log("\n✓ CLI build successful");
        console.log("");
        console.log("Note: Desktop app not available on this system.");
        console.log("Use CLI instead:");
        console.log("  cargo build -p echovault-cli --release");
        console.log("  ./target/release/echovault-cli auth");
        console.log("  ./target/release/echovault-cli sync");
    } else {
        console.error("\n✗ CLI build also failed");
        process.exit(1);
    }
}

main();

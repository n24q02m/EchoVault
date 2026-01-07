#!/usr/bin/env node
// Reset app - remove config to setup again
//
// Usage:
//   pnpm reset        - Reset config only (triggers setup wizard)
//   pnpm reset --db   - Delete vault.db only (rebuild from files)
//   pnpm reset --all  - Delete everything (config + vault data + db)

import { existsSync, rmSync, unlinkSync, readdirSync } from "fs";
import { homedir, platform } from "os";
import { join } from "path";

/**
 * Get platform-specific config directory.
 * - Windows: %APPDATA%\echovault
 * - macOS: ~/Library/Application Support/echovault
 * - Linux: ~/.config/echovault
 */
function getConfigDir() {
    const home = homedir();
    const os = platform();

    if (os === "win32") {
        return join(process.env.APPDATA || join(home, "AppData", "Roaming"), "echovault");
    } else if (os === "darwin") {
        return join(home, "Library", "Application Support", "echovault");
    } else {
        // Linux and others
        return join(process.env.XDG_CONFIG_HOME || join(home, ".config"), "echovault");
    }
}

/**
 * Get platform-specific data directory.
 * - Windows: %LOCALAPPDATA%\echovault\vault
 * - macOS: ~/Library/Application Support/echovault/vault
 * - Linux: ~/.local/share/echovault/vault
 */
function getDataDir() {
    const home = homedir();
    const os = platform();

    if (os === "win32") {
        return join(process.env.LOCALAPPDATA || join(home, "AppData", "Local"), "echovault", "vault");
    } else if (os === "darwin") {
        return join(home, "Library", "Application Support", "echovault", "vault");
    } else {
        // Linux and others
        return join(process.env.XDG_DATA_HOME || join(home, ".local", "share"), "echovault", "vault");
    }
}

// Config file path (echovault.toml)
const configDir = getConfigDir();
const configPath = join(configDir, "echovault.toml");

// Vault data path
const vaultPath = getDataDir();

console.log("EchoVault Reset\n");
console.log("Platform:", platform());
console.log("Config Dir:", configDir);
console.log("Data Dir:", vaultPath);
console.log("");

const args = process.argv.slice(2);

// --db: Delete only vault.db files (rebuild from session files)
if (args.includes("--db")) {
    const dbFiles = ["vault.db", "vault.db-wal", "vault.db-shm"];
    let deleted = false;

    for (const file of dbFiles) {
        const dbPath = join(vaultPath, file);
        if (existsSync(dbPath)) {
            unlinkSync(dbPath);
            console.log("Database file removed:", dbPath);
            deleted = true;
        }
    }

    if (!deleted) {
        console.log("No database files found at:", vaultPath);
    }

    console.log("\nDatabase reset complete.");
    console.log("Session files in vault/sessions are preserved.");
    console.log("Restart the app to rebuild database from files.");
    process.exit(0);
}

// --all: Delete everything (config + vault data)
if (args.includes("--all") || args.includes("-a")) {
    // Remove config
    if (existsSync(configPath)) {
        rmSync(configPath);
        console.log("Config removed:", configPath);
    } else {
        console.log("No config found at:", configPath);
    }

    // Remove vault data
    if (existsSync(vaultPath)) {
        rmSync(vaultPath, { recursive: true, force: true });
        console.log("Vault data removed:", vaultPath);
    } else {
        console.log("No vault data found at:", vaultPath);
    }

    console.log("\nFull reset complete. Restart the app to setup again.");
    process.exit(0);
}

// Default: Remove config only
if (existsSync(configPath)) {
    rmSync(configPath);
    console.log("Config removed:", configPath);
} else {
    console.log("No config found at:", configPath);
}

console.log("\nReset complete. Restart the app to setup again.");
console.log("\nOptions:");
console.log("  pnpm reset --db   - Delete vault.db only (rebuild from files)");
console.log("  pnpm reset --all  - Delete everything (config + vault data)");

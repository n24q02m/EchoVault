#!/usr/bin/env node
// Reset app - remove config to setup again


import { existsSync, rmSync } from "fs";
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

// 1. Remove config file
if (existsSync(configPath)) {
    rmSync(configPath);
    console.log("Config removed:", configPath);
} else {
    console.log("No config found at:", configPath);
}

// 2. Ask to remove vault data
const args = process.argv.slice(2);
if (args.includes("--all") || args.includes("-a")) {
    if (existsSync(vaultPath)) {
        rmSync(vaultPath, { recursive: true, force: true });
        console.log("Vault data removed:", vaultPath);
    } else {
        console.log("No vault data found at:", vaultPath);
    }
} else {
    console.log("\nNote: Vault data still exists at:", vaultPath);
    console.log("Run 'pnpm reset --all' to also remove vault data.");
}

console.log("\nReset complete. Restart the app to setup again.");

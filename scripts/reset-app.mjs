#!/usr/bin/env node
/**
 * Script reset app - xóa config để setup lại từ đầu
 */

import { existsSync, rmSync } from "fs";
import { homedir } from "os";
import { join } from "path";

// Config file path (echovault.toml, không phải config.toml)
const configDir = join(homedir(), ".config", "echovault");
const configPath = join(configDir, "echovault.toml");

// Vault data path
const vaultPath = join(homedir(), ".local", "share", "echovault", "vault");

console.log("EchoVault Reset\n");

// 1. Xóa config file
if (existsSync(configPath)) {
    rmSync(configPath);
    console.log("✓ Config removed:", configPath);
} else {
    console.log("✗ No config found at:", configPath);
}

// 2. Hỏi có muốn xóa vault data không
const args = process.argv.slice(2);
if (args.includes("--all") || args.includes("-a")) {
    if (existsSync(vaultPath)) {
        rmSync(vaultPath, { recursive: true, force: true });
        console.log("✓ Vault data removed:", vaultPath);
    } else {
        console.log("✗ No vault data found at:", vaultPath);
    }
} else {
    console.log("\nNote: Vault data still exists at:", vaultPath);
    console.log("Run 'pnpm reset --all' to also remove vault data.");
}

console.log("\nReset complete. Restart the app to setup again.");

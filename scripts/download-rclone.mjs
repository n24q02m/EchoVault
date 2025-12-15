#!/usr/bin/env node
// Script download Rclone binary cho dev environment

import { chmodSync, createReadStream, createWriteStream, existsSync, mkdirSync, renameSync, rmSync, unlinkSync } from "fs";
import { dirname, join } from "path";
import { pipeline } from "stream/promises";
import { Extract } from "unzipper";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const RCLONE_VERSION = "v1.68.2";
const BINARIES_DIR = join(__dirname, "..", "apps", "tauri", "binaries");

// Detect OS and architecture
function getPlatformInfo() {
    const platform = process.platform;
    const arch = process.arch;

    let os, ext, archName;
    if (platform === "win32") {
        os = "windows";
        ext = ".exe";
        archName = arch === "x64" ? "amd64" : "386";
    } else if (platform === "darwin") {
        os = "osx";
        ext = "";
        archName = arch === "arm64" ? "arm64" : "amd64";
    } else {
        os = "linux";
        ext = "";
        archName = arch === "x64" ? "amd64" : arch === "arm64" ? "arm64" : "386";
    }

    return { os, ext, archName };
}

// Get Tauri target triple
function getTargetTriple() {
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "win32") {
        return arch === "x64" ? "x86_64-pc-windows-msvc" : "i686-pc-windows-msvc";
    } else if (platform === "darwin") {
        return arch === "arm64"
            ? "aarch64-apple-darwin"
            : "x86_64-apple-darwin";
    } else {
        return arch === "x64"
            ? "x86_64-unknown-linux-gnu"
            : "aarch64-unknown-linux-gnu";
    }
}

async function downloadRclone() {
    const { os, ext, archName } = getPlatformInfo();
    const targetTriple = getTargetTriple();

    // Rclone expects naming like: rclone-x86_64-pc-windows-msvc.exe
    const binaryName = `rclone-${targetTriple}${ext}`;
    const binaryPath = join(BINARIES_DIR, binaryName);

    // Skip if already exists
    if (existsSync(binaryPath)) {
        console.log(`Rclone binary already exists: ${binaryPath}`);
        return;
    }

    console.log(`Downloading Rclone ${RCLONE_VERSION} for ${os}-${archName}...`);

    // Create binaries directory
    if (!existsSync(BINARIES_DIR)) {
        mkdirSync(BINARIES_DIR, { recursive: true });
    }

    // Download URL
    const archiveName =
        os === "windows"
            ? `rclone-${RCLONE_VERSION}-${os}-${archName}.zip`
            : `rclone-${RCLONE_VERSION}-${os}-${archName}.zip`;
    const url = `https://github.com/rclone/rclone/releases/download/${RCLONE_VERSION}/${archiveName}`;

    console.log(`Fetching from: ${url}`);

    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Failed to download: ${response.statusText}`);
    }

    const tempZip = join(BINARIES_DIR, "rclone-temp.zip");
    const tempWriter = createWriteStream(tempZip);
    await pipeline(response.body, tempWriter);

    console.log("Extracting...");

    // Extract using unzipper
    await new Promise((resolve, reject) => {
        const extractor = Extract({ path: BINARIES_DIR });
        extractor.on("close", resolve);
        extractor.on("error", reject);
        createReadStream(tempZip).pipe(extractor);
    });

    // Find and rename the rclone binary
    const extractedDir = join(
        BINARIES_DIR,
        `rclone-${RCLONE_VERSION}-${os}-${archName}`
    );
    const extractedBinary = join(extractedDir, `rclone${ext}`);

    if (existsSync(extractedBinary)) {
        renameSync(extractedBinary, binaryPath);
        rmSync(extractedDir, { recursive: true });
        unlinkSync(tempZip);
    }

    // Make executable on Unix
    if (os !== "windows") {
        chmodSync(binaryPath, 0o755);
    }

    console.log(`Rclone binary saved to: ${binaryPath}`);
}

downloadRclone().catch((err) => {
    console.error("Failed to download Rclone:", err.message);
    process.exit(1);
});

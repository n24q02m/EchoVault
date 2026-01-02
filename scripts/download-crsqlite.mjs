#!/usr/bin/env node
// Download cr-sqlite extension binary for dev environment and bundling

import {
    chmodSync,
    createReadStream,
    createWriteStream,
    existsSync,
    mkdirSync,
    renameSync,
    rmSync,
    unlinkSync,
} from "fs";
import { dirname, join } from "path";
import { pipeline } from "stream/promises";
import { Extract } from "unzipper";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// cr-sqlite version to download
const CRSQLITE_VERSION = "0.16.3";
const BINARIES_DIR = join(__dirname, "..", "apps", "tauri", "binaries");

// Detect OS and architecture
function getPlatformInfo() {
    const platform = process.platform;
    const arch = process.arch;

    let os, ext, archName;
    if (platform === "win32") {
        os = "windows";
        ext = ".dll";
        archName = "x86_64";
    } else if (platform === "darwin") {
        os = "darwin";
        ext = ".dylib";
        archName = arch === "arm64" ? "aarch64" : "x86_64";
    } else {
        os = "linux";
        ext = ".so";
        archName = arch === "x64" ? "x86_64" : "aarch64";
    }

    return { os, ext, archName };
}

// Get Tauri target triple for naming
function getTargetTriple() {
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "win32") {
        return "x86_64-pc-windows-msvc";
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

async function downloadCrsqlite() {
    const { os, ext, archName } = getPlatformInfo();
    const targetTriple = getTargetTriple();

    // Name format: crsqlite-<target>.so/dll/dylib
    const binaryName = `crsqlite-${targetTriple}${ext}`;
    const binaryPath = join(BINARIES_DIR, binaryName);

    // Skip if already exists
    if (existsSync(binaryPath)) {
        console.log(`cr-sqlite binary already exists: ${binaryPath}`);
        return;
    }

    console.log(
        `Downloading cr-sqlite v${CRSQLITE_VERSION} for ${os}-${archName}...`
    );

    // Create binaries directory
    if (!existsSync(BINARIES_DIR)) {
        mkdirSync(BINARIES_DIR, { recursive: true });
    }

    // GitHub release asset naming: crsqlite-<os>-<arch>.zip
    // Examples: crsqlite-linux-x86_64.zip, crsqlite-darwin-aarch64.zip, crsqlite-win-x86_64.zip
    const osName = os === "windows" ? "win" : os;
    const archiveName = `crsqlite-${osName}-${archName}.zip`;
    const url = `https://github.com/vlcn-io/cr-sqlite/releases/download/v${CRSQLITE_VERSION}/${archiveName}`;

    console.log(`Fetching from: ${url}`);

    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Failed to download: ${response.status} ${response.statusText}`);
    }

    const tempZip = join(BINARIES_DIR, "crsqlite-temp.zip");
    const tempWriter = createWriteStream(tempZip);
    await pipeline(response.body, tempWriter);

    console.log("Extracting...");

    // Extract using unzipper
    const tempExtractDir = join(BINARIES_DIR, "crsqlite-temp");
    if (!existsSync(tempExtractDir)) {
        mkdirSync(tempExtractDir, { recursive: true });
    }

    await new Promise((resolve, reject) => {
        const extractor = Extract({ path: tempExtractDir });
        extractor.on("close", resolve);
        extractor.on("error", reject);
        createReadStream(tempZip).pipe(extractor);
    });

    // Find the extracted binary (crsqlite.so, crsqlite.dll, or crsqlite.dylib)
    const extractedBinary = join(tempExtractDir, `crsqlite${ext}`);

    if (existsSync(extractedBinary)) {
        renameSync(extractedBinary, binaryPath);
        rmSync(tempExtractDir, { recursive: true });
        unlinkSync(tempZip);
    } else {
        // Clean up and throw error
        rmSync(tempExtractDir, { recursive: true, force: true });
        unlinkSync(tempZip);
        throw new Error(`Binary not found in archive: crsqlite${ext}`);
    }

    // Make executable on Unix
    if (os !== "windows") {
        chmodSync(binaryPath, 0o755);
    }

    console.log(`cr-sqlite binary saved to: ${binaryPath}`);
}

downloadCrsqlite().catch((err) => {
    console.error("Failed to download cr-sqlite:", err.message);
    process.exit(1);
});

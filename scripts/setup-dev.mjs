#!/usr/bin/env node
// Setup development environment for EchoVault
// Step order:
// 1. OS packages (Tauri dependencies)
// 2. mise
// 3. mise tools (Rust, Node.js, uv)
// 4. pnpm
// 5. Node.js dependencies
// 6. Rclone binary
// 7. Pre-commit hooks

import { execSync } from "child_process";
import { existsSync } from "fs";
import { platform } from "os";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Colors for terminal output
const colors = {
  reset: "\x1b[0m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  blue: "\x1b[34m",
  cyan: "\x1b[36m",
};

function printStep(message) {
  console.log(`\n${colors.blue}==>${colors.reset} ${message}`);
}

function printSuccess(message) {
  console.log(`${colors.green}✓${colors.reset} ${message}`);
}

function printWarning(message) {
  console.log(`${colors.yellow}⚠${colors.reset} ${message}`);
}

function printError(message) {
  console.error(`${colors.red}✗${colors.reset} ${message}`);
}

function runCommand(command, options = {}) {
  try {
    execSync(command, {
      stdio: options.silent ? "pipe" : "inherit",
      ...options,
    });
    return { success: true, error: null };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

function checkCommandExists(cmd) {
  try {
    const checkCmd = platform() === "win32" ? `where ${cmd}` : `which ${cmd}`;
    execSync(checkCmd, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

// ============================================================================
// Step 1: Install OS-level Tauri dependencies
// ============================================================================

function installTauriDepsLinux() {
  if (!checkCommandExists("pkg-config")) {
    printWarning("Tauri dependencies not found, installing...");

    const deps = [
      "pkg-config",
      "libgtk-3-dev",
      "libwebkit2gtk-4.1-dev",
      "libayatana-appindicator3-dev",
      "librsvg2-dev",
    ];

    let installCmd;
    if (checkCommandExists("apt")) {
      installCmd = `sudo apt update && sudo apt install -y ${deps.join(" ")}`;
    } else if (checkCommandExists("dnf")) {
      const depsRpm = [
        "pkg-config",
        "gtk3-devel",
        "webkit2gtk4.1-devel",
        "libayatana-appindicator-gtk3-devel",
        "librsvg2-devel",
      ];
      installCmd = `sudo dnf install -y ${depsRpm.join(" ")}`;
    } else if (checkCommandExists("pacman")) {
      const depsArch = [
        "pkg-config",
        "gtk3",
        "webkit2gtk-4.1",
        "libayatana-appindicator",
        "librsvg",
      ];
      installCmd = `sudo pacman -S --noconfirm ${depsArch.join(" ")}`;
    } else {
      printError("Package manager not found");
      return false;
    }

    const result = runCommand(installCmd);
    if (!result.success) {
      printError(`Failed to install Tauri dependencies: ${result.error}`);
      return false;
    }

    printSuccess("Tauri dependencies installed");
  } else {
    printSuccess("Tauri dependencies already installed");
  }

  return true;
}

function installTauriDepsMacOS() {
  if (!checkCommandExists("brew")) {
    printWarning("Homebrew not found, installing...");

    const installCmd = '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"';
    console.log("Installing Homebrew (may need password and take a few minutes)...");

    const result = runCommand(installCmd);
    if (!result.success) {
      printError("Failed to install Homebrew");
      return false;
    }

    printSuccess("Homebrew installed");
  } else {
    printSuccess("Homebrew already installed");
  }

  printSuccess("Tauri dependencies ready (macOS built-in)");
  return true;
}

function installTauriDepsWindows() {
  const vsPaths = [
    "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022",
    "C:\\Program Files\\Microsoft Visual Studio\\2022",
    "C:\\Program Files (x86)\\Microsoft Visual Studio\\2019",
    "C:\\Program Files\\Microsoft Visual Studio\\2019",
  ];

  const vsFound = vsPaths.some((path) => existsSync(path));

  if (!vsFound) {
    printError("Visual Studio Build Tools not found");
    console.log("\nManually install:");
    console.log('winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"');
    return false;
  }

  printSuccess("Visual Studio Build Tools already installed");
  return true;
}

function installTauriDependencies() {
  printStep("Step 1: Install OS packages (Tauri dependencies)");

  const platformName = platform();

  if (platformName === "linux") {
    return installTauriDepsLinux();
  } else if (platformName === "darwin") {
    return installTauriDepsMacOS();
  } else if (platformName === "win32") {
    return installTauriDepsWindows();
  } else {
    printError(`Platform not supported: ${platformName}`);
    return false;
  }
}

// ============================================================================
// Step 2: Install mise
// ============================================================================

function installMise() {
  printStep("Step 2: Install mise");

  if (checkCommandExists("mise")) {
    printSuccess("mise already installed");
    return true;
  }

  printWarning("mise not found, installing...");

  const platformName = platform();
  let installCmd;

  if (platformName === "win32") {
    installCmd = 'powershell -Command "irm https://mise.run | iex"';
  } else {
    installCmd = 'curl https://mise.run | sh';
  }

  console.log("Installing mise...");
  const result = runCommand(installCmd);

  if (!result.success) {
    printError("Failed to install mise");
    return false;
  }

  // Reload PATH
  if (platformName !== "win32") {
    process.env.PATH = `${process.env.HOME}/.local/bin:${process.env.PATH}`;
  }

  printSuccess("mise installed");
  return true;
}

// ============================================================================
// Step 3: Install mise tools (Rust, Node.js, uv)
// ============================================================================

function installMiseTools() {
  printStep("Step 3: Install mise tools (Rust, Node.js, uv)");

  const projectRoot = join(__dirname, "..");
  console.log("Running 'mise install'...");

  const result = runCommand("mise install", { cwd: projectRoot });

  if (!result.success) {
    printError("Failed to install mise tools");
    return false;
  }

  // Reload PATH để có các tools
  if (platform() !== "win32") {
    process.env.PATH = `${process.env.HOME}/.local/share/mise/shims:${process.env.PATH}`;
  }

  printSuccess("mise tools installed");

  // Verify Rust
  if (checkCommandExists("cargo")) {
    try {
      const version = execSync("cargo --version", { encoding: "utf-8" }).trim();
      printSuccess(`Rust: ${version}`);
    } catch { }
  }

  return true;
}

// ============================================================================
// Step 4: Install pnpm
// ============================================================================

function installPnpm() {
  printStep("Step 4: Install pnpm");

  if (checkCommandExists("pnpm")) {
    printSuccess("pnpm already installed");
    return true;
  }

  printWarning("pnpm not found, installing...");

  // Try npm first
  if (checkCommandExists("npm")) {
    const result = runCommand("npm install -g pnpm", { silent: true });
    if (result.success) {
      printSuccess("pnpm installed via npm");
      return true;
    }
  }

  // Fallback to standalone
  const platformName = platform();
  let installCmd;

  if (platformName === "win32") {
    installCmd = 'powershell -Command "iwr https://get.pnpm.io/install.ps1 -useb | iex"';
  } else {
    installCmd = 'curl -fsSL https://get.pnpm.io/install.sh | sh -';
  }

  const result = runCommand(installCmd);
  if (!result.success) {
    printError("Failed to install pnpm");
    return false;
  }

  printSuccess("pnpm installed");
  return true;
}

// ============================================================================
// Step 5: Install Node.js dependencies
// ============================================================================

function installNodeDependencies() {
  printStep("Step 5: Install Node.js dependencies");

  const projectRoot = join(__dirname, "..");
  const result = runCommand("pnpm install", { cwd: projectRoot });

  if (!result.success) {
    printError("Failed to install Node dependencies");
    return false;
  }

  printSuccess("Node dependencies installed");
  return true;
}

// ============================================================================
// Step 6: Download Rclone binary
// ============================================================================

function downloadRclone() {
  printStep("Step 6: Download Rclone binary");

  const downloadScript = join(__dirname, "download-rclone.mjs");

  if (!existsSync(downloadScript)) {
    printError("download-rclone.mjs not found");
    return false;
  }

  const result = runCommand(`node "${downloadScript}"`);

  if (!result.success) {
    printError("Failed to download Rclone");
    return false;
  }

  printSuccess("Rclone binary downloaded");
  return true;
}

// ============================================================================
// Step 7: Setup pre-commit hooks
// ============================================================================

function setupPreCommit() {
  printStep("Step 7: Setup pre-commit hooks");

  const projectRoot = join(__dirname, "..");
  const venvPath = join(projectRoot, ".venv");

  if (!checkCommandExists("uv")) {
    printWarning("uv not found, skipping pre-commit");
    return false;
  }

  // Create venv
  if (!existsSync(venvPath)) {
    console.log("Creating Python venv...");
    const venvResult = runCommand("uv venv", { cwd: projectRoot, silent: true });
    if (!venvResult.success) {
      printWarning("Failed to create venv, skipping pre-commit");
      return false;
    }
  }

  // Install pre-commit
  console.log("Installing pre-commit...");
  const installResult = runCommand("uv pip install pre-commit", { cwd: projectRoot, silent: true });
  if (!installResult.success) {
    printWarning("Failed to install pre-commit");
    return false;
  }

  // Install hooks
  console.log("Installing hooks...");
  const hooksResult = runCommand("uv run pre-commit install", { cwd: projectRoot, silent: true });
  if (!hooksResult.success) {
    printWarning("Failed to install hooks");
    return false;
  }

  printSuccess("Pre-commit hooks installed");
  return true;
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  console.log(`\n${colors.blue}${"=".repeat(60)}${colors.reset}`);
  console.log(`${colors.blue}  EchoVault Development Environment Setup${colors.reset}`);
  console.log(`${colors.blue}${"=".repeat(60)}${colors.reset}\n`);

  const successSteps = [];
  const failedSteps = [];

  // Step 1
  if (installTauriDependencies()) {
    successSteps.push("OS packages (Tauri)");
  } else {
    failedSteps.push("OS packages");
  }

  // Step 2
  if (installMise()) {
    successSteps.push("mise");
  } else {
    failedSteps.push("mise");
    printError("Failed to install mise");
    process.exit(1);
  }

  // Step 3
  if (installMiseTools()) {
    successSteps.push("mise tools (Rust, Node, uv)");
  } else {
    failedSteps.push("mise tools");
    printError("Failed to install mise tools");
    process.exit(1);
  }

  // Step 4
  if (installPnpm()) {
    successSteps.push("pnpm");
  } else {
    failedSteps.push("pnpm");
  }

  // Step 5
  if (installNodeDependencies()) {
    successSteps.push("Node dependencies");
  } else {
    failedSteps.push("Node dependencies");
  }

  // Step 6
  if (downloadRclone()) {
    successSteps.push("Rclone binary");
  } else {
    failedSteps.push("Rclone binary");
  }

  // Step 7
  if (setupPreCommit()) {
    successSteps.push("Pre-commit hooks");
  } else {
    failedSteps.push("Pre-commit hooks (optional)");
  }

  // Summary
  console.log(`\n${colors.blue}${"=".repeat(60)}${colors.reset}`);
  console.log(`${colors.blue}  Setup Summary${colors.reset}`);
  console.log(`${colors.blue}${"=".repeat(60)}${colors.reset}\n`);

  if (successSteps.length > 0) {
    console.log(`${colors.green}Success:${colors.reset}`);
    successSteps.forEach((step) => {
      console.log(`  ${colors.green}✓${colors.reset} ${step}`);
    });
  }

  if (failedSteps.length > 0) {
    console.log(`\n${colors.red}Failed:${colors.reset}`);
    failedSteps.forEach((step) => {
      console.log(`  ${colors.red}✗${colors.reset} ${step}`);
    });
  }

  console.log(`\n${colors.blue}${"=".repeat(60)}${colors.reset}`);

  const criticalFailed = failedSteps.filter(
    (step) => !step.includes("optional")
  );

  if (criticalFailed.length === 0) {
    console.log(`\n${colors.green}✓ Setup complete!${colors.reset}`);
    console.log("\nYou can now run:");
    console.log(`  ${colors.cyan}pnpm dev${colors.reset}          - Dev mode (web only)`);
    console.log(`  ${colors.cyan}cargo tauri dev${colors.reset}   - Dev mode (full app)`);
    console.log(`  ${colors.cyan}cargo tauri build${colors.reset} - Production build`);
    process.exit(0);
  } else {
    console.log(`\n${colors.red}✗ Setup incomplete${colors.reset}`);
    process.exit(1);
  }
}

main().catch((error) => {
  printError(`Setup failed: ${error.message}`);
  process.exit(1);
});

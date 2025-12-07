import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Types
interface SessionInfo {
  id: string;
  source: string;
  title: string | null;
  workspace_name: string | null;
  created_at: string | null;
  file_size: number;
  path: string;
}

interface ScanResult {
  sessions: SessionInfo[];
  total: number;
}

interface AuthStatusResponse {
  status: "not_authenticated" | "pending" | "authenticated" | "error";
  user_code: string | null;
  verify_url: string | null;
  error: string | null;
}

interface AppConfig {
  setup_complete: boolean;
  provider: string;
  repo_name: string | null;
  encrypt: boolean;
  compress: boolean;
}

// Views
type View = "setup" | "main";
type Tab = "sessions" | "settings";

// ==================== SETUP WIZARD ====================
type SetupStep =
  | "auth"
  | "checking"
  | "cloning"
  | "passphrase_check"
  | "config"
  | "done";

interface VaultMetadataResponse {
  exists: boolean;
  encrypted: boolean;
  compressed: boolean;
}

function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState<SetupStep>("auth");
  const [authStatus, setAuthStatus] = useState<AuthStatusResponse | null>(null);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [repoName, setRepoName] = useState("my-vault");
  const [encrypt, setEncrypt] = useState(true);
  const [compress, setCompress] = useState(true);
  const [passphrase, setPassphrase] = useState("");
  const [confirmPassphrase, setConfirmPassphrase] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Clone flow states
  const [isCloning, setIsCloning] = useState(false);
  const [existingVault, setExistingVault] =
    useState<VaultMetadataResponse | null>(null);
  const [checkingProgress, setCheckingProgress] = useState("");

  const handleStartAuth = async () => {
    setIsAuthenticating(true);
    setError(null);
    try {
      const status = await invoke<AuthStatusResponse>("start_auth");
      setAuthStatus(status);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleCompleteAuth = async () => {
    setIsAuthenticating(true);
    setError(null);
    try {
      const status = await invoke<AuthStatusResponse>("complete_auth");
      setAuthStatus(status);
      if (status.status === "authenticated") {
        // Sau khi auth xong, check xem repo đã tồn tại chưa
        setStep("checking");
        await checkExistingVault();
      } else if (status.status === "pending") {
        // Chưa xác thực, poll lại sau 3 giây
        setTimeout(handleCompleteAuth, 3000);
      }
    } catch (err) {
      setError(String(err));
      setIsAuthenticating(false);
    }
  };

  const checkExistingVault = async () => {
    setCheckingProgress("Checking for existing vault...");
    try {
      const exists = await invoke<boolean>("check_repo_exists", { repoName });
      if (exists) {
        // Vault đã tồn tại, clone về
        setCheckingProgress("Found existing vault! Cloning...");
        setStep("cloning");
        setIsCloning(true);
        await invoke("clone_vault", { repoName });
        setIsCloning(false);

        // Đọc metadata
        const metadata =
          await invoke<VaultMetadataResponse>("get_vault_metadata");
        setExistingVault(metadata);
        setEncrypt(metadata.encrypted);
        setCompress(metadata.compressed);

        if (metadata.encrypted) {
          // Cần nhập passphrase
          setStep("passphrase_check");
        } else {
          // Không encrypted, hoàn tất
          await invoke("complete_setup", {
            request: {
              provider: "github",
              repo_name: repoName,
              encrypt: metadata.encrypted,
              compress: metadata.compressed,
              passphrase: null,
            },
          });
          setStep("done");
          setTimeout(onComplete, 1000);
        }
      } else {
        // Không có vault, tạo mới
        setStep("config");
      }
    } catch (err) {
      setError(String(err));
      setStep("config"); // Fallback to manual config
    }
  };

  const handleVerifyPassphrase = async () => {
    if (!passphrase) {
      setError("Please enter your passphrase");
      return;
    }
    if (passphrase.length < 8) {
      setError("Passphrase must be at least 8 characters");
      return;
    }

    setError(null);
    setCheckingProgress("Verifying passphrase...");

    try {
      const valid = await invoke<boolean>("verify_passphrase_cmd", {
        passphrase,
      });
      if (valid) {
        // Passphrase đúng, lưu vào keyring và hoàn tất
        await invoke("complete_setup", {
          request: {
            provider: "github",
            repo_name: repoName,
            encrypt: true,
            compress: existingVault?.compressed ?? true,
            passphrase,
          },
        });
        setStep("done");
        setTimeout(onComplete, 1000);
      } else {
        setError("Incorrect passphrase. Please try again.");
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleFinishSetup = async () => {
    // Validate passphrase if encryption enabled
    if (encrypt) {
      if (!passphrase) {
        setError("Please enter a passphrase");
        return;
      }
      if (passphrase.length < 8) {
        setError("Passphrase must be at least 8 characters");
        return;
      }
      if (passphrase !== confirmPassphrase) {
        setError("Passphrases do not match");
        return;
      }
    }

    try {
      await invoke("complete_setup", {
        request: {
          provider: "github",
          repo_name: repoName,
          encrypt,
          compress,
          passphrase: encrypt ? passphrase : null,
        },
      });
      setStep("done");
      setTimeout(onComplete, 1000);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-8 text-center">
        <img
          src="/logo.png"
          alt="EchoVault"
          className="mx-auto mb-4 h-16 w-16 rounded-2xl"
        />
        <h1 className="text-xl font-bold">EchoVault</h1>
        <p className="mt-1 text-sm text-[var(--text-secondary)]">
          First Time Setup
        </p>
      </div>

      {step === "auth" && (
        <div className="flex flex-1 flex-col">
          <div className="glass mb-4 rounded-xl p-5">
            <h2 className="mb-3 font-semibold">1. Authenticate with GitHub</h2>

            {!authStatus || authStatus.status === "not_authenticated" ? (
              <button
                onClick={handleStartAuth}
                disabled={isAuthenticating}
                className="w-full rounded-lg bg-[var(--accent)] py-2.5 font-medium text-white disabled:opacity-50"
              >
                {isAuthenticating ? "Processing..." : "Login with GitHub"}
              </button>
            ) : authStatus.status === "pending" ? (
              <div className="space-y-3">
                <p className="text-sm">
                  1. Click{" "}
                  <button
                    onClick={() =>
                      invoke("open_url", { url: authStatus.verify_url })
                    }
                    className="text-[var(--accent)] underline"
                  >
                    Open GitHub
                  </button>{" "}
                  to authenticate
                </p>
                <p className="text-sm">
                  2. Enter code:{" "}
                  <code className="rounded bg-[var(--bg-card)] px-2 py-1 font-mono text-lg">
                    {authStatus.user_code}
                  </code>
                </p>
                <button
                  onClick={handleCompleteAuth}
                  disabled={isAuthenticating}
                  className="w-full rounded-lg bg-[var(--success)] py-2.5 font-medium text-white disabled:opacity-50"
                >
                  {isAuthenticating ? "Checking..." : "I've authorized"}
                </button>
              </div>
            ) : authStatus.status === "authenticated" ? (
              <div className="py-3 text-center">
                <span className="text-[var(--success)]">Authenticated!</span>
              </div>
            ) : null}
          </div>

          {error && <p className="text-center text-sm text-red-400">{error}</p>}
        </div>
      )}

      {/* Checking for existing vault */}
      {(step === "checking" || step === "cloning") && (
        <div className="flex flex-1 flex-col items-center justify-center">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
          <p className="mt-4 text-[var(--text-secondary)]">
            {checkingProgress || "Please wait..."}
          </p>
          {isCloning && (
            <p className="mt-2 text-sm text-[var(--text-secondary)]">
              Downloading your vault from GitHub...
            </p>
          )}
        </div>
      )}

      {/* Passphrase verification for existing vault */}
      {step === "passphrase_check" && (
        <div className="flex flex-1 flex-col">
          <div className="glass mb-4 rounded-xl p-5">
            <h2 className="mb-3 font-semibold">Unlock Your Vault</h2>
            <p className="mb-4 text-sm text-[var(--text-secondary)]">
              Your vault is encrypted. Please enter your passphrase to unlock.
            </p>

            <div className="relative">
              <input
                type={showPassword ? "text" : "password"}
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
                placeholder="Enter your passphrase"
                className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] px-3 py-2 pr-10"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              >
                {showPassword ? (
                  <svg
                    className="h-5 w-5"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
                    />
                  </svg>
                ) : (
                  <svg
                    className="h-5 w-5"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                    />
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                    />
                  </svg>
                )}
              </button>
            </div>
          </div>

          {error && (
            <p className="mb-2 text-center text-sm text-red-400">{error}</p>
          )}

          <button
            onClick={handleVerifyPassphrase}
            className="w-full rounded-lg bg-[var(--accent)] py-3 font-semibold text-white"
          >
            Unlock Vault
          </button>
        </div>
      )}

      {step === "config" && (
        <div className="flex flex-1 flex-col overflow-y-auto">
          <div className="glass mb-4 rounded-xl p-5">
            <h2 className="mb-4 font-semibold">2. Configure Vault</h2>

            <div className="space-y-4">
              <div>
                <label className="mb-1.5 block text-sm">Repository Name</label>
                <input
                  type="text"
                  value={repoName}
                  onChange={(e) => setRepoName(e.target.value)}
                  placeholder="my-vault"
                  className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] px-3 py-2"
                />
                <p className="mt-1 text-xs text-[var(--text-secondary)]">
                  Will create: github.com/username/{repoName}
                </p>
              </div>

              <div className="flex items-center justify-between py-2">
                <div>
                  <p className="font-medium">Encryption (AES-256)</p>
                  <p className="text-xs text-[var(--text-secondary)]">
                    Lose passphrase = lose data
                  </p>
                </div>
                <button
                  onClick={() => setEncrypt(!encrypt)}
                  className={`h-6 w-12 rounded-full transition-colors ${encrypt ? "bg-[var(--accent)]" : "bg-[var(--bg-card)]"}`}
                >
                  <div
                    className={`h-5 w-5 rounded-full bg-white shadow transition-transform ${encrypt ? "translate-x-6" : "translate-x-0.5"}`}
                  />
                </button>
              </div>

              <div className="flex items-center justify-between py-2">
                <div>
                  <p className="font-medium">Compression</p>
                  <p className="text-xs text-[var(--text-secondary)]">
                    Reduce storage size
                  </p>
                </div>
                <button
                  onClick={() => setCompress(!compress)}
                  className={`h-6 w-12 rounded-full transition-colors ${compress ? "bg-[var(--accent)]" : "bg-[var(--bg-card)]"}`}
                >
                  <div
                    className={`h-5 w-5 rounded-full bg-white shadow transition-transform ${compress ? "translate-x-6" : "translate-x-0.5"}`}
                  />
                </button>
              </div>

              {/* Passphrase inputs when encryption enabled */}
              {encrypt && (
                <div className="mt-2 space-y-3 border-t border-[var(--border)] pt-4">
                  <div>
                    <label className="mb-1 block text-sm">Passphrase</label>
                    <div className="relative">
                      <input
                        type={showPassword ? "text" : "password"}
                        value={passphrase}
                        onChange={(e) => setPassphrase(e.target.value)}
                        placeholder="Enter passphrase (min 8 chars)"
                        className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] px-3 py-2 pr-10 text-sm"
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      >
                        {showPassword ? (
                          <svg
                            className="h-5 w-5"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
                            />
                          </svg>
                        ) : (
                          <svg
                            className="h-5 w-5"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                            />
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                            />
                          </svg>
                        )}
                      </button>
                    </div>
                  </div>
                  <div>
                    <label className="mb-1 block text-sm">
                      Confirm Passphrase
                    </label>
                    <div className="relative">
                      <input
                        type={showPassword ? "text" : "password"}
                        value={confirmPassphrase}
                        onChange={(e) => setConfirmPassphrase(e.target.value)}
                        placeholder="Confirm passphrase"
                        className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] px-3 py-2 pr-10 text-sm"
                      />
                    </div>
                  </div>
                  <p className="text-xs text-yellow-400">
                    Warning: Lost passphrase = lost data!
                  </p>
                </div>
              )}
            </div>
          </div>

          {error && (
            <p className="mb-2 text-center text-sm text-red-400">{error}</p>
          )}

          <button
            onClick={handleFinishSetup}
            className="w-full rounded-lg bg-[var(--accent)] py-3 font-semibold text-white"
          >
            Complete Setup
          </button>
        </div>
      )}

      {step === "done" && (
        <div className="flex flex-1 items-center justify-center">
          <div className="text-center">
            <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-[var(--success)]">
              <svg
                className="h-8 w-8 text-white"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </div>
            <p className="text-lg font-medium">Setup Complete!</p>
          </div>
        </div>
      )}
    </div>
  );
}

// ==================== MAIN APP ====================

// Cache keys for localStorage
const CACHE_KEY = "echovault_sessions_cache";
const CACHE_TIMESTAMP_KEY = "echovault_cache_timestamp";

// Helper functions for cache
const loadCachedSessions = (): SessionInfo[] => {
  try {
    const cached = localStorage.getItem(CACHE_KEY);
    return cached ? JSON.parse(cached) : [];
  } catch {
    return [];
  }
};

const saveCachedSessions = (sessions: SessionInfo[]) => {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(sessions));
    localStorage.setItem(CACHE_TIMESTAMP_KEY, Date.now().toString());
  } catch (err) {
    console.error("Failed to cache sessions:", err);
  }
};

function MainApp() {
  const [activeTab, setActiveTab] = useState<Tab>("sessions");
  // Initialize with cached data for instant display
  const [sessions, setSessions] = useState<SessionInfo[]>(loadCachedSessions());
  const [isScanning, setIsScanning] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [expandedSources, setExpandedSources] = useState<Set<string>>(
    new Set()
  );
  const [isSyncing, setIsSyncing] = useState(false);
  const [lastSyncTime, setLastSyncTime] = useState<string | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [scanStatus, setScanStatus] = useState<"idle" | "scanning" | "syncing">(
    "idle"
  );

  const groupedSessions = sessions.reduce(
    (acc, session) => {
      const source = session.source || "unknown";
      if (!acc[source]) acc[source] = [];
      acc[source].push(session);
      return acc;
    },
    {} as Record<string, SessionInfo[]>
  );

  // Initialize provider with saved credentials and load sessions
  useEffect(() => {
    const initialize = async () => {
      try {
        // Load config first
        const cfg = await invoke<AppConfig>("get_config");
        setConfig(cfg);

        // Initialize provider with saved credentials (for sync later)
        try {
          await invoke<boolean>("init_provider");
        } catch (initErr) {
          console.error("init_provider failed:", initErr);
        }

        // Scan sessions (separate from sync)
        await loadSessions();
      } catch (err) {
        console.error("Initialization failed:", err);
      }
    };

    initialize();
  }, []);

  // Load sessions from backend
  const loadSessions = async () => {
    setIsScanning(true);
    setScanStatus("scanning");

    try {
      const result = await invoke<ScanResult>("scan_sessions");
      setSessions(result.sessions);
      saveCachedSessions(result.sessions);
    } catch (err) {
      console.error("Failed to scan sessions:", err);
    } finally {
      setIsScanning(false);
      setScanStatus("idle");
    }
  };

  // Background sync (separate function, called by interval or manual button)
  const backgroundSync = async () => {
    if (isSyncing) return;

    setIsSyncing(true);
    setScanStatus("syncing");
    setSyncError(null);

    try {
      const success = await invoke<boolean>("sync_vault");
      if (success) {
        setLastSyncTime(new Date().toLocaleTimeString());
      } else {
        setLastSyncTime(new Date().toLocaleTimeString());
      }
    } catch (syncErr) {
      console.error("Sync failed:", syncErr);
      setSyncError(String(syncErr));
    } finally {
      setIsSyncing(false);
      setScanStatus("idle");
    }
  };

  const handleOpenFile = async (sessionId: string) => {
    try {
      await invoke("open_file", { filePath: sessionId });
    } catch (err) {
      console.error("Failed to open file:", err);
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDate = (dateStr: string | null): string => {
    if (!dateStr) return "";
    const date = new Date(dateStr);
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const toggleSource = (source: string) => {
    setExpandedSources((prev) => {
      const next = new Set(prev);
      if (next.has(source)) {
        next.delete(source);
      } else {
        next.add(source);
      }
      return next;
    });
  };

  // Expand all sources by default when sessions load
  useEffect(() => {
    if (sessions.length > 0) {
      const sources = [...new Set(sessions.map((s) => s.source || "unknown"))];
      setExpandedSources(new Set(sources));
    }
  }, [sessions]);

  // Manual sync handler
  const handleSync = async () => {
    setIsSyncing(true);
    setSyncError(null);
    try {
      const success = await invoke<boolean>("sync_vault");
      if (success) {
        setLastSyncTime(new Date().toLocaleTimeString());
      } else {
        setLastSyncTime(new Date().toLocaleTimeString());
      }
    } catch (err) {
      console.error("Sync failed:", err);
      setSyncError(String(err));
    } finally {
      setIsSyncing(false);
    }
  };

  // Auto-sync every 5 minutes
  useEffect(() => {
    const interval = setInterval(
      () => {
        if (!isScanning && !isSyncing) {
          backgroundSync();
        }
      },
      5 * 60 * 1000
    ); // 5 minutes

    return () => clearInterval(interval);
  }, [isScanning, isSyncing]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3">
        <div className="flex items-center gap-2">
          <img src="/logo.png" alt="EchoVault" className="h-8 w-8 rounded-lg" />
          <span className="font-semibold">EchoVault</span>
        </div>
        <div className="flex items-center gap-3">
          {syncError && (
            <span className="text-xs text-red-400" title={syncError}>
              Sync error
            </span>
          )}
          {lastSyncTime && !syncError && (
            <span className="text-xs text-[var(--text-secondary)]">
              Last sync: {lastSyncTime}
            </span>
          )}
          <button
            onClick={handleSync}
            disabled={isSyncing}
            className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-sm transition-colors hover:bg-[var(--bg-card)] disabled:opacity-50"
            title="Sync to cloud"
          >
            <svg
              className={`h-4 w-4 ${isSyncing ? "animate-spin" : ""}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            {isSyncing ? "Syncing..." : "Sync"}
          </button>
          <div
            className={`h-2 w-2 rounded-full ${isSyncing ? "animate-pulse bg-yellow-500" : "bg-[var(--success)]"}`}
            title={isSyncing ? "Syncing" : "Online"}
          />
        </div>
      </header>

      <main className="flex-1 overflow-y-auto">
        {activeTab === "sessions" && (
          <div className="p-4">
            {/* Show cached data immediately, with spinner if scanning */}
            {sessions.length === 0 && isScanning ? (
              <div className="flex flex-col items-center justify-center py-8">
                <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
                <p className="mt-2 text-sm text-[var(--text-secondary)]">
                  Scanning sessions...
                </p>
              </div>
            ) : sessions.length === 0 ? (
              <div className="py-8 text-center text-[var(--text-secondary)]">
                <p>No sessions found</p>
                <button
                  onClick={loadSessions}
                  className="mt-2 text-sm text-[var(--accent)] hover:underline"
                >
                  Scan again
                </button>
              </div>
            ) : (
              <div className="space-y-3">
                {Object.entries(groupedSessions).map(([source, items]) => (
                  <div
                    key={source}
                    className="glass overflow-hidden rounded-lg"
                  >
                    <button
                      onClick={() => toggleSource(source)}
                      className="flex w-full items-center justify-between px-3 py-2.5 text-left hover:bg-[var(--bg-card)]"
                    >
                      <span className="text-sm font-medium uppercase text-[var(--text-secondary)]">
                        {source} ({items.length})
                      </span>
                      <svg
                        className={`h-4 w-4 text-[var(--text-secondary)] transition-transform ${
                          expandedSources.has(source) ? "rotate-180" : ""
                        }`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M19 9l-7 7-7-7"
                        />
                      </svg>
                    </button>
                    {expandedSources.has(source) && (
                      <div className="space-y-1 px-3 pb-3">
                        {items.map((session) => (
                          <div
                            key={session.id}
                            onClick={() => handleOpenFile(session.path)}
                            className="cursor-pointer rounded-lg bg-[var(--bg-card)] p-2.5 transition-colors hover:bg-[var(--border)]"
                          >
                            <div className="flex items-start justify-between">
                              <div className="min-w-0 flex-1">
                                <p className="truncate text-sm font-medium">
                                  {session.title ||
                                    session.workspace_name ||
                                    "Untitled"}
                                </p>
                                <p className="mt-0.5 text-xs text-[var(--text-secondary)]">
                                  {formatFileSize(session.file_size)}
                                </p>
                              </div>
                              <span className="text-xs text-[var(--text-secondary)]">
                                {formatDate(session.created_at)}
                              </span>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "settings" && config && (
          <div className="space-y-4 p-4">
            <div className="glass rounded-lg p-4">
              <h3 className="mb-1 text-sm text-[var(--text-secondary)]">
                Provider
              </h3>
              <p className="font-medium">{config.provider}</p>
            </div>
            <div className="glass rounded-lg p-4">
              <h3 className="mb-1 text-sm text-[var(--text-secondary)]">
                Repository
              </h3>
              <p className="font-medium">
                {config.repo_name || "Not configured"}
              </p>
            </div>
            <div className="glass rounded-lg p-4">
              <h3 className="mb-1 text-sm text-[var(--text-secondary)]">
                Encryption
              </h3>
              <p className="font-medium">
                {config.encrypt ? "Enabled (AES-256-GCM)" : "Disabled"}
              </p>
            </div>
            <div className="glass rounded-lg p-4">
              <h3 className="mb-1 text-sm text-[var(--text-secondary)]">
                Compression
              </h3>
              <p className="font-medium">
                {config.compress ? "Enabled" : "Disabled"}
              </p>
            </div>
          </div>
        )}
      </main>
      <nav className="flex border-t border-[var(--border)] bg-[var(--bg-secondary)]">
        <button
          onClick={() => setActiveTab("sessions")}
          className={`flex flex-1 flex-col items-center gap-1 py-3 transition-colors ${activeTab === "sessions" ? "text-[var(--accent)]" : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"}`}
        >
          <svg
            className="h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 6h16M4 12h16M4 18h16"
            />
          </svg>
          <span className="text-xs">Sessions</span>
        </button>
        <button
          onClick={() => setActiveTab("settings")}
          className={`flex flex-1 flex-col items-center gap-1 py-3 transition-colors ${activeTab === "settings" ? "text-[var(--accent)]" : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"}`}
        >
          <svg
            className="h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
          <span className="text-xs">Settings</span>
        </button>
      </nav>
    </div>
  );
}

// ==================== ROOT APP ====================
function App() {
  const [view, setView] = useState<View>("main");
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    checkSetup();
  }, []);

  const checkSetup = async () => {
    try {
      const complete = await invoke<boolean>("check_setup_complete");
      setView(complete ? "main" : "setup");
    } catch {
      setView("setup");
    } finally {
      setIsChecking(false);
    }
  };

  if (isChecking) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
      </div>
    );
  }

  if (view === "setup") {
    return <SetupWizard onComplete={() => setView("main")} />;
  }

  return <MainApp />;
}

export default App;

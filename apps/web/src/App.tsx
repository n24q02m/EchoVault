import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { Toaster, toast } from "sonner";
import { TextEditor } from "./TextEditor";

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
  message: string | null;
}

interface AppConfig {
  setup_complete: boolean;
  vault_path: string;
  remote_name: string | null;
  folder_name: string;
}

interface AppInfo {
  version: string;
  data_dir: string;
  config_dir: string;
  logs_dir: string;
}

interface UpdateCheckResult {
  update_available: boolean;
  current_version: string;
  new_version: string | null;
}

// Views
type View = "setup" | "main";

// ==================== SETUP WIZARD ====================
type SetupStep = "connect" | "config" | "done";

function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState<SetupStep>("connect");
  const [authStatus, setAuthStatus] = useState<AuthStatusResponse | null>(null);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [folderName, setFolderName] = useState("EchoVault");
  const [error, setError] = useState<string | null>(null);
  const [isCheckingAuth, setIsCheckingAuth] = useState(true);

  // Check if already authenticated on mount (e.g., rclone configured on host)
  useEffect(() => {
    const checkExistingAuth = async () => {
      try {
        const status = await invoke<AuthStatusResponse>("complete_auth");
        setAuthStatus(status);
        if (status.status === "authenticated") {
          // Already authenticated, skip to config step
          setStep("config");
        }
      } catch (err) {
        // Ignore errors, just show connect button
        console.log("Auth check failed:", err);
      } finally {
        setIsCheckingAuth(false);
      }
    };
    checkExistingAuth();
  }, []);

  const handleStartAuth = async () => {
    setIsAuthenticating(true);
    setError(null);
    try {
      const status = await invoke<AuthStatusResponse>("start_auth");
      setAuthStatus(status);
      if (status.status === "authenticated") {
        setStep("config");
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsAuthenticating(false);
    }
  };

  const handleCheckAuth = async () => {
    setIsAuthenticating(true);
    setError(null);
    try {
      const status = await invoke<AuthStatusResponse>("complete_auth");
      setAuthStatus(status);
      if (status.status === "authenticated") {
        setStep("config");
      } else if (status.status === "pending") {
        // Keep polling
        setTimeout(handleCheckAuth, 3000);
      }
    } catch (err) {
      setError(String(err));
      setIsAuthenticating(false);
    }
  };

  const handleFinishSetup = async () => {
    if (!folderName.trim()) {
      setError("Please enter a folder name");
      return;
    }

    try {
      await invoke("complete_setup", {
        request: { folder_name: folderName },
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
        <img src="/logo.png" alt="EchoVault" className="mx-auto mb-4 h-16 w-16 rounded-2xl" />
        <h1 className="text-xl font-bold">EchoVault</h1>
        <p className="mt-1 text-sm text-[var(--text-secondary)]">First Time Setup</p>
      </div>

      {step === "connect" && (
        <div className="flex flex-1 flex-col">
          <div className="glass mb-4 rounded-xl p-5">
            <h2 className="mb-3 font-semibold">1. Connect Cloud Storage</h2>
            <p className="mb-4 text-sm text-[var(--text-secondary)]">
              Connect to Google Drive via Rclone.
            </p>

            {isCheckingAuth ? (
              <div className="flex items-center justify-center py-4">
                <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
                <span className="ml-2 text-sm text-[var(--text-secondary)]">
                  Checking connection...
                </span>
              </div>
            ) : !authStatus || authStatus.status === "not_authenticated" ? (
              <button
                onClick={handleStartAuth}
                disabled={isAuthenticating}
                className="w-full rounded-lg bg-[var(--accent)] py-2.5 font-medium text-white disabled:opacity-50"
              >
                {isAuthenticating ? "Connecting..." : "Connect Cloud Storage"}
              </button>
            ) : authStatus.status === "pending" ? (
              <div className="space-y-3">
                <p className="text-sm text-[var(--text-secondary)]">
                  {authStatus.message || "Please complete authentication in your browser..."}
                </p>
                <button
                  onClick={handleCheckAuth}
                  disabled={isAuthenticating}
                  className="w-full rounded-lg bg-[var(--success)] py-2.5 font-medium text-white disabled:opacity-50"
                >
                  {isAuthenticating ? "Checking..." : "I've connected"}
                </button>
              </div>
            ) : authStatus.status === "authenticated" ? (
              <div className="py-3 text-center">
                <span className="text-[var(--success)]">Connected!</span>
              </div>
            ) : authStatus.status === "error" ? (
              <div className="space-y-3">
                <p className="text-sm text-red-400">{authStatus.message}</p>
                <button
                  onClick={handleStartAuth}
                  className="w-full rounded-lg bg-[var(--accent)] py-2.5 font-medium text-white"
                >
                  Try Again
                </button>
              </div>
            ) : null}
          </div>

          {error && <p className="text-center text-sm text-red-400">{error}</p>}
        </div>
      )}

      {step === "config" && (
        <div className="flex flex-1 flex-col">
          <div className="glass mb-4 rounded-xl p-5">
            <h2 className="mb-4 font-semibold">2. Configure Sync Folder</h2>

            <div className="space-y-4">
              <div>
                <label className="mb-1.5 block text-sm">Folder Name</label>
                <input
                  type="text"
                  value={folderName}
                  onChange={(e) => setFolderName(e.target.value)}
                  placeholder="EchoVault"
                  className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] px-3 py-2"
                />
                <p className="mt-1 text-xs text-[var(--text-secondary)]">
                  Your data will be synced to this folder in cloud storage.
                </p>
              </div>
            </div>
          </div>

          {error && <p className="mb-2 text-center text-sm text-red-400">{error}</p>}

          <button
            type="button"
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

// ==================== SETTINGS OVERLAY ====================

function SettingsOverlay({ onClose }: { onClose: () => void }) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [autoLaunch, setAutoLaunch] = useState(false);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const [info, autostart] = await Promise.all([
          invoke<AppInfo>("get_app_info"),
          invoke<boolean>("get_autostart_status"),
        ]);
        setAppInfo(info);
        setAutoLaunch(autostart);
      } catch (err) {
        toast.error(`Failed to load settings: ${String(err)}`);
      } finally {
        setIsLoading(false);
      }
    };
    loadSettings();
  }, []);

  const handleAutoLaunchToggle = async () => {
    const newValue = !autoLaunch;
    try {
      await invoke("set_autostart", { enabled: newValue });
      setAutoLaunch(newValue);
      toast.success(newValue ? "Auto-launch enabled" : "Auto-launch disabled");
    } catch (err) {
      toast.error(`Failed to update auto-launch: ${String(err)}`);
    }
  };

  const handleOpenDataFolder = async () => {
    try {
      await invoke("open_data_folder");
    } catch (err) {
      toast.error(`Failed to open data folder: ${String(err)}`);
    }
  };

  const handleCheckUpdate = async () => {
    setIsCheckingUpdate(true);
    try {
      const result = await invoke<UpdateCheckResult>("check_update_manual");
      if (result.update_available) {
        toast.success(`Update available: v${result.new_version}`);
      } else {
        toast.info("You are on the latest version");
      }
    } catch {
      // In dev mode, updater is not available - show info instead of error
      toast.info("Update check not available in dev mode");
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  const handleOpenGitHub = async () => {
    try {
      await invoke("open_url", { url: "https://github.com/n24q02m/EchoVault" });
    } catch (err) {
      toast.error(`Failed to open GitHub: ${String(err)}`);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="glass mx-4 max-h-[90vh] w-full max-w-sm overflow-y-auto rounded-2xl p-5">
        {/* Header */}
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <img src="/logo.png" alt="EchoVault" className="h-10 w-10 rounded-xl" />
            <div>
              <h2 className="font-semibold">EchoVault</h2>
              <p className="text-xs text-[var(--text-secondary)]">
                {isLoading ? "Loading..." : `v${appInfo?.version}`}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 hover:bg-[var(--bg-card)]"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <title>Close</title>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
          </div>
        ) : (
          <>
            {/* Settings Section */}
            <div className="mb-4">
              <h3 className="mb-2 text-xs font-medium uppercase text-[var(--text-secondary)]">
                Settings
              </h3>
              <div className="space-y-2">
                {/* Auto Launch Toggle - fixed CSS */}
                <div className="flex items-center justify-between rounded-lg bg-[var(--bg-card)] p-3">
                  <span className="text-sm">Auto Launch</span>
                  <button
                    type="button"
                    onClick={handleAutoLaunchToggle}
                    className={`relative h-6 w-11 rounded-full transition-colors ${autoLaunch ? "bg-[var(--accent)]" : "bg-gray-500"}`}
                  >
                    <span
                      className="absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all duration-200"
                      style={{ left: autoLaunch ? "calc(100% - 22px)" : "2px" }}
                    />
                  </button>
                </div>

                {/* Data Folder */}
                <button
                  type="button"
                  onClick={handleOpenDataFolder}
                  className="flex w-full items-center justify-between rounded-lg bg-[var(--bg-card)] p-3 text-left hover:bg-[var(--border)]"
                >
                  <span className="text-sm">Open Data Folder</span>
                  <svg
                    className="h-4 w-4 text-[var(--text-secondary)]"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <title>Open</title>
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                    />
                  </svg>
                </button>
              </div>
            </div>

            {/* Actions Section */}
            <div>
              <h3 className="mb-2 text-xs font-medium uppercase text-[var(--text-secondary)]">
                Actions
              </h3>
              <div className="space-y-2">
                <button
                  type="button"
                  onClick={handleCheckUpdate}
                  disabled={isCheckingUpdate}
                  className="w-full rounded-lg bg-[var(--accent)] py-2.5 text-sm font-medium text-white disabled:opacity-50"
                >
                  {isCheckingUpdate ? "Checking..." : "Check for Updates"}
                </button>
                <button
                  type="button"
                  onClick={handleOpenGitHub}
                  className="flex w-full items-center justify-center gap-2 rounded-lg border border-[var(--border)] py-2.5 text-sm font-medium hover:bg-[var(--bg-card)]"
                >
                  <svg className="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
                    <title>GitHub</title>
                    <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                  </svg>
                  GitHub
                </button>
              </div>
            </div>
          </>
        )}
      </div>
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
  const [sessions, setSessions] = useState<SessionInfo[]>(loadCachedSessions());
  const [isScanning, setIsScanning] = useState(false);
  const [expandedSources, setExpandedSources] = useState<Set<string>>(new Set());
  const [isSyncing, setIsSyncing] = useState(false);
  // syncError is kept to be able to display toast notification in the future
  const [, setSyncError] = useState<string | null>(null);
  const [visibleCounts, setVisibleCounts] = useState<Record<string, number>>({});
  const [viewingSession, setViewingSession] = useState<SessionInfo | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const ITEMS_PER_PAGE = 10;

  const groupedSessions = sessions.reduce(
    (acc, session) => {
      const source = session.source;
      if (!source) return acc; // Skip sessions without source
      if (!acc[source]) acc[source] = [];
      acc[source].push(session);
      return acc;
    },
    {} as Record<string, SessionInfo[]>
  );

  useEffect(() => {
    const initialize = async () => {
      try {
        // Config is still loaded to check setup, but not saved to state because UI has removed Settings
        await invoke<AppConfig>("get_config");
        await loadSessions();
      } catch (err) {
        toast.error(`Initialization failed: ${String(err)}`);
      }
    };
    initialize();
  }, []);

  const loadSessions = async () => {
    setIsScanning(true);
    try {
      const result = await invoke<ScanResult>("scan_sessions");
      setSessions(result.sessions);
      saveCachedSessions(result.sessions);
    } catch (err) {
      toast.error(`Failed to scan sessions: ${String(err)}`);
    } finally {
      setIsScanning(false);
    }
  };

  const handleSync = async () => {
    if (isSyncing) return;
    setIsSyncing(true);
    setSyncError(null);
    try {
      await invoke<string>("sync_vault");
      await loadSessions();
    } catch (err) {
      setSyncError(String(err));
    } finally {
      setIsSyncing(false);
    }
  };

  const handleOpenFile = (session: SessionInfo) => {
    setViewingSession(session);
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

  useEffect(() => {
    if (sessions.length > 0) {
      const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))] as string[];
      setExpandedSources(new Set(sources));
    }
  }, [sessions]);

  // Auto-sync interval
  const syncRef = useRef(handleSync);
  syncRef.current = handleSync;

  useEffect(() => {
    // Initial sync after 5s
    const timeout = setTimeout(() => syncRef.current(), 5000);
    // Periodic sync every 30s
    const interval = setInterval(() => syncRef.current(), 30000);

    // Listen for tray menu event
    const unlisten = listen("trigger-sync", () => syncRef.current());

    return () => {
      clearTimeout(timeout);
      clearInterval(interval);
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3">
        <div className="flex items-center gap-2">
          <img src="/logo.png" alt="EchoVault" className="h-8 w-8 rounded-lg" />
          <span className="font-semibold">EchoVault</span>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm">
            {isSyncing ? (
              <>
                <div className="h-3 w-3 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
                <span className="text-[var(--text-secondary)]">Syncing...</span>
              </>
            ) : (
              <>
                <div className="h-2 w-2 rounded-full bg-[var(--success)]" />
                <span className="text-[var(--text-secondary)]">Synced</span>
              </>
            )}
          </div>
          <button
            type="button"
            onClick={() => setShowSettings(true)}
            className="rounded-lg p-1.5 hover:bg-[var(--bg-card)]"
            title="Settings"
          >
            <svg
              className="h-5 w-5 text-[var(--text-secondary)]"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <title>Settings</title>
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
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-4">
          {isScanning && sessions.length === 0 && (
            <div className="flex items-center justify-center py-8">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
            </div>
          )}

          {!isScanning && sessions.length === 0 && (
            <div className="py-8 text-center text-[var(--text-secondary)]">
              <p>No sessions found.</p>
              <p className="mt-2 text-sm">
                Open VS Code with GitHub Copilot to create chat sessions.
              </p>
            </div>
          )}

          {Object.entries(groupedSessions).map(([source, sourceSessions]) => {
            const isExpanded = expandedSources.has(source);
            const visibleCount = visibleCounts[source] || ITEMS_PER_PAGE;
            const visibleSessions = sourceSessions.slice(0, visibleCount);
            const hasMore = sourceSessions.length > visibleCount;

            return (
              <div key={source} className="glass rounded-xl">
                <button
                  onClick={() => toggleSource(source)}
                  className="flex w-full items-center justify-between px-4 py-3"
                >
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{source}</span>
                    <span className="rounded-full bg-[var(--accent)] px-2 py-0.5 text-xs text-white">
                      {sourceSessions.length}
                    </span>
                  </div>
                  <svg
                    className={`h-5 w-5 transition-transform ${isExpanded ? "rotate-180" : ""}`}
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

                {isExpanded && (
                  <div className="border-t border-[var(--border)]">
                    {visibleSessions.map((session) => (
                      <button
                        type="button"
                        key={session.id}
                        onClick={() => handleOpenFile(session)}
                        className="flex w-full items-center justify-between px-4 py-3 text-left hover:bg-[var(--bg-card)]"
                      >
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-medium">
                            {session.title || session.workspace_name || session.id}
                          </p>
                          <p className="text-xs text-[var(--text-secondary)]">
                            {formatDate(session.created_at)} - {formatFileSize(session.file_size)}
                          </p>
                        </div>
                        <svg
                          className="h-4 w-4 text-[var(--text-secondary)]"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <title>Open</title>
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M9 5l7 7-7 7"
                          />
                        </svg>
                      </button>
                    ))}
                    {hasMore && (
                      <button
                        type="button"
                        onClick={() =>
                          setVisibleCounts((prev) => ({
                            ...prev,
                            [source]: visibleCount + ITEMS_PER_PAGE,
                          }))
                        }
                        className="block w-full py-2 text-center text-sm text-[var(--accent)]"
                      >
                        Show more ({sourceSessions.length - visibleCount} remaining)
                      </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* TextEditor Overlay */}
      {viewingSession && (
        <TextEditor
          path={viewingSession.path}
          title={viewingSession.title || viewingSession.workspace_name || viewingSession.id}
          onClose={() => setViewingSession(null)}
        />
      )}

      {/* Settings Overlay */}
      {showSettings && <SettingsOverlay onClose={() => setShowSettings(false)} />}
    </div>
  );
}

// ==================== ROOT APP ====================
function App() {
  const [view, setView] = useState<View>("main");
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const checkSetup = async () => {
      try {
        const complete = await invoke<boolean>("check_setup_complete");
        setView(complete ? "main" : "setup");
      } catch (err) {
        toast.error(`Failed to check setup: ${String(err)}`);
        setView("setup");
      } finally {
        setIsLoading(false);
      }
    };
    checkSetup();
  }, []);

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
      </div>
    );
  }

  if (view === "setup") {
    return (
      <>
        <Toaster richColors position="bottom-center" />
        <SetupWizard onComplete={() => setView("main")} />
      </>
    );
  }

  return (
    <>
      <Toaster richColors position="bottom-center" />
      <MainApp />
    </>
  );
}

export default App;

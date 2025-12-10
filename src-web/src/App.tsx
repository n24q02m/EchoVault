import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
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

// Views
type View = "setup" | "main";
type Tab = "sessions" | "settings";

// ==================== SETUP WIZARD ====================
type SetupStep = "connect" | "config" | "done";

function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState<SetupStep>("connect");
  const [authStatus, setAuthStatus] = useState<AuthStatusResponse | null>(null);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [folderName, setFolderName] = useState("EchoVault");
  const [error, setError] = useState<string | null>(null);

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
              Connect to Google Drive, Dropbox, OneDrive, or any other cloud service via Rclone.
            </p>

            {!authStatus || authStatus.status === "not_authenticated" ? (
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
  const [sessions, setSessions] = useState<SessionInfo[]>(loadCachedSessions());
  const [isScanning, setIsScanning] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [expandedSources, setExpandedSources] = useState<Set<string>>(new Set());
  const [isSyncing, setIsSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [visibleCounts, setVisibleCounts] = useState<Record<string, number>>({});
  const [viewingSession, setViewingSession] = useState<SessionInfo | null>(null);
  const ITEMS_PER_PAGE = 10;

  const groupedSessions = sessions.reduce(
    (acc, session) => {
      const source = session.source || "unknown";
      if (!acc[source]) acc[source] = [];
      acc[source].push(session);
      return acc;
    },
    {} as Record<string, SessionInfo[]>
  );

  useEffect(() => {
    const initialize = async () => {
      try {
        const cfg = await invoke<AppConfig>("get_config");
        setConfig(cfg);
        await loadSessions();
      } catch (err) {
        console.error("Initialization failed:", err);
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
      console.error("Failed to scan sessions:", err);
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
      console.error("Sync failed:", err);
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
      const sources = [...new Set(sessions.map((s) => s.source || "unknown"))];
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
        <div className="flex items-center gap-2">
          <button
            onClick={handleSync}
            disabled={isSyncing}
            className="rounded-lg bg-[var(--accent)] px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
          >
            {isSyncing ? "Syncing..." : "Sync"}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-[var(--border)]">
        <button
          onClick={() => setActiveTab("sessions")}
          className={`flex-1 py-2 text-sm font-medium ${
            activeTab === "sessions"
              ? "border-b-2 border-[var(--accent)] text-[var(--accent)]"
              : "text-[var(--text-secondary)]"
          }`}
        >
          Sessions ({sessions.length})
        </button>
        <button
          onClick={() => setActiveTab("settings")}
          className={`flex-1 py-2 text-sm font-medium ${
            activeTab === "settings"
              ? "border-b-2 border-[var(--accent)] text-[var(--accent)]"
              : "text-[var(--text-secondary)]"
          }`}
        >
          Settings
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === "sessions" && (
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
        )}

        {activeTab === "settings" && config && (
          <div className="space-y-4">
            <div className="glass rounded-xl p-4">
              <h3 className="mb-3 font-semibold">Vault Settings</h3>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Vault Path</span>
                  <span className="truncate max-w-[200px]">{config.vault_path}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Cloud Folder</span>
                  <span>{config.folder_name}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Remote</span>
                  <span>{config.remote_name || "Not connected"}</span>
                </div>
              </div>
            </div>

            {syncError && (
              <div className="rounded-xl bg-red-500/10 p-4">
                <p className="text-sm text-red-400">{syncError}</p>
              </div>
            )}
          </div>
        )}
      </div>

      {/* TextEditor Overlay */}
      {viewingSession && (
        <TextEditor
          path={viewingSession.path}
          title={viewingSession.title || viewingSession.workspace_name || viewingSession.id}
          onClose={() => setViewingSession(null)}
        />
      )}
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
        console.error("Failed to check setup:", err);
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
    return <SetupWizard onComplete={() => setView("main")} />;
  }

  return <MainApp />;
}

export default App;

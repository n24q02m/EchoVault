import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Types matching Rust structs
interface SessionInfo {
  id: string;
  source: string;
  title: string | null;
  workspace_name: string | null;
  created_at: string | null;
  file_size: number;
}

interface ScanResult {
  sessions: SessionInfo[];
  total: number;
}

interface SyncResult {
  success: boolean;
  message: string;
  extracted_count: number;
  encrypted_count: number;
}

// Navigation tabs
type Tab = "home" | "activity" | "settings";

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("home");
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);

  // Load sessions on mount
  useEffect(() => {
    loadSessions();
  }, []);

  const loadSessions = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<ScanResult>("scan_sessions");
      setSessions(result.sessions);
    } catch (err) {
      console.error("Failed to scan sessions:", err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSync = async () => {
    setIsSyncing(true);
    setSyncMessage(null);
    try {
      const result = await invoke<SyncResult>("sync_vault");
      setLastSync(new Date().toLocaleTimeString("vi-VN"));
      setSyncMessage(
        result.success ? "Đồng bộ thành công!" : `Lỗi: ${result.message}`
      );
      await loadSessions();
    } catch (err) {
      setSyncMessage(`Lỗi: ${err}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-[var(--accent)] to-purple-600 flex items-center justify-center">
            <span className="text-white font-bold text-sm">EV</span>
          </div>
          <span className="font-semibold">EchoVault</span>
        </div>
        <button
          onClick={handleSync}
          disabled={isSyncing}
          className="px-3 py-1.5 rounded-lg bg-[var(--accent)] hover:bg-[var(--accent-hover)] text-white text-sm font-medium transition-colors disabled:opacity-50"
        >
          {isSyncing ? "Đang đồng bộ..." : "Đồng bộ"}
        </button>
      </header>

      {/* Sync Status */}
      <div className="px-4 py-3 border-b border-[var(--border)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${
              isSyncing ? "bg-yellow-400 animate-pulse" : "bg-[var(--success)]"
            }`}
          ></div>
          <span className="text-sm text-[var(--text-secondary)]">
            {isSyncing
              ? "Đang đồng bộ..."
              : lastSync
              ? `Đồng bộ lần cuối: ${lastSync}`
              : "Chưa đồng bộ"}
          </span>
        </div>
        {syncMessage && (
          <p
            className={`text-xs mt-1 ${
              syncMessage.includes("Lỗi")
                ? "text-red-400"
                : "text-[var(--success)]"
            }`}
          >
            {syncMessage}
          </p>
        )}
      </div>

      {/* Content */}
      <main className="flex-1 overflow-y-auto">
        {activeTab === "home" && (
          <div className="p-4">
            <h2 className="text-sm font-medium text-[var(--text-secondary)] mb-3">
              Sessions ({sessions.length})
            </h2>
            {isLoading ? (
              <div className="flex items-center justify-center py-8">
                <div className="w-6 h-6 border-2 border-[var(--accent)] border-t-transparent rounded-full animate-spin"></div>
              </div>
            ) : sessions.length === 0 ? (
              <div className="text-center py-8 text-[var(--text-secondary)]">
                <p>Không tìm thấy session nào</p>
                <button
                  onClick={loadSessions}
                  className="mt-2 text-[var(--accent)] hover:underline text-sm"
                >
                  Quét lại
                </button>
              </div>
            ) : (
              <div className="space-y-2">
                {sessions.map((session) => (
                  <div
                    key={session.id}
                    className="p-3 rounded-lg glass hover:bg-[var(--bg-card)] transition-colors cursor-pointer"
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex-1 min-w-0">
                        <p className="font-medium truncate">
                          {session.title ||
                            session.workspace_name ||
                            "Untitled"}
                        </p>
                        <p className="text-xs text-[var(--text-secondary)] mt-0.5">
                          {session.source} - {formatFileSize(session.file_size)}
                        </p>
                      </div>
                      <span className="text-xs text-[var(--text-secondary)]">
                        {session.created_at
                          ? new Date(session.created_at).toLocaleDateString(
                              "vi-VN"
                            )
                          : ""}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "activity" && (
          <div className="p-4 text-center text-[var(--text-secondary)]">
            <p>Lịch sử đồng bộ sẽ hiển thị ở đây</p>
          </div>
        )}

        {activeTab === "settings" && (
          <div className="p-4 space-y-4">
            <div className="glass rounded-lg p-4">
              <h3 className="font-medium mb-2">Provider</h3>
              <p className="text-sm text-[var(--text-secondary)]">GitHub</p>
            </div>
            <div className="glass rounded-lg p-4">
              <h3 className="font-medium mb-2">Mã hóa</h3>
              <p className="text-sm text-[var(--text-secondary)]">
                AES-256-GCM (Bật)
              </p>
            </div>
          </div>
        )}
      </main>

      {/* Bottom Navigation */}
      <nav className="flex border-t border-[var(--border)] bg-[var(--bg-secondary)]">
        {[
          { id: "home" as Tab, icon: "🏠", label: "Home" },
          { id: "activity" as Tab, icon: "📊", label: "Activity" },
          { id: "settings" as Tab, icon: "⚙️", label: "Settings" },
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex-1 py-3 flex flex-col items-center gap-1 transition-colors ${
              activeTab === tab.id
                ? "text-[var(--accent)]"
                : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            }`}
          >
            <span className="text-lg">{tab.icon}</span>
            <span className="text-xs">{tab.label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}

export default App;

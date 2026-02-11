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

type EmbeddingPreset = "ollama" | "openai" | "gemini" | "mistral" | "custom";

interface EmbeddingConfig {
  preset: EmbeddingPreset;
  api_base: string;
  api_key: string | null;
  model: string;
}

interface ProviderStatusInfo {
  status: "available" | "model_not_found" | "unavailable";
  dimension: number | null;
  message: string | null;
}

interface PresetInfo {
  label: string;
  description: string;
  api_base: string;
  model: string;
  models: string[];
  requires_key: boolean;
  key_placeholder: string;
}

const PRESET_INFO: Record<EmbeddingPreset, PresetInfo> = {
  ollama: {
    label: "Ollama",
    description: "Local, free, no API key needed",
    api_base: "http://localhost:11434/v1",
    model: "nomic-embed-text",
    models: ["nomic-embed-text", "mxbai-embed-large", "all-minilm"],
    requires_key: false,
    key_placeholder: "",
  },
  openai: {
    label: "OpenAI",
    description: "Cloud, requires API key",
    api_base: "https://api.openai.com/v1",
    model: "text-embedding-3-small",
    models: ["text-embedding-3-small", "text-embedding-3-large", "text-embedding-ada-002"],
    requires_key: true,
    key_placeholder: "sk-...",
  },
  gemini: {
    label: "Google Gemini",
    description: "Cloud, requires API key",
    api_base: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "text-embedding-004",
    models: ["text-embedding-004"],
    requires_key: true,
    key_placeholder: "AIza...",
  },
  mistral: {
    label: "Mistral AI",
    description: "Cloud, requires API key",
    api_base: "https://api.mistral.ai/v1",
    model: "mistral-embed",
    models: ["mistral-embed"],
    requires_key: true,
    key_placeholder: "...",
  },
  custom: {
    label: "Custom (OpenAI-compatible)",
    description: "LiteLLM, vLLM, TGI, or any compatible endpoint",
    api_base: "http://localhost:8000/v1",
    model: "nomic-embed-text",
    models: [],
    requires_key: false,
    key_placeholder: "...",
  },
};

function SettingsOverlay({ onClose }: { onClose: () => void }) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [autoLaunch, setAutoLaunch] = useState(false);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Embedding config state
  const [embeddingConfig, setEmbeddingConfig] = useState<EmbeddingConfig>({
    preset: "ollama",
    api_base: "http://localhost:11434/v1",
    api_key: null,
    model: "nomic-embed-text",
  });
  const [providerStatus, setProviderStatus] = useState<ProviderStatusInfo | null>(null);
  const [isTesting, setIsTesting] = useState(false);
  const [isSavingConfig, setIsSavingConfig] = useState(false);
  const [configDirty, setConfigDirty] = useState(false);
  const [ollamaAvailable, setOllamaAvailable] = useState<boolean | null>(null);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const [info, autostart, embConfig, ollamaCheck] = await Promise.all([
          invoke<AppInfo>("get_app_info"),
          invoke<boolean>("get_autostart_status"),
          invoke<EmbeddingConfig>("get_embedding_config"),
          invoke<{ available: boolean; models: string[] }>("check_ollama"),
        ]);
        setAppInfo(info);
        setAutoLaunch(autostart);
        setEmbeddingConfig(embConfig);
        setOllamaAvailable(ollamaCheck.available);
        setOllamaModels(ollamaCheck.models || []);
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
        const confirmed = window.confirm(
          `Update available: v${result.new_version}\n\nDo you want to download and install now?`
        );
        if (confirmed) {
          toast.info("Downloading update...");
          try {
            await invoke("install_update");
            toast.success("Update installed! App will restart.");
          } catch (installErr) {
            toast.error(`Install failed: ${String(installErr)}`);
          }
        }
      } else {
        toast.info("You are on the latest version");
      }
    } catch (err) {
      // Show actual error for debugging
      const errorMsg = String(err);
      if (errorMsg.includes("not running from an installed app") || errorMsg.includes("dev")) {
        toast.info("Update check not available in development builds");
      } else {
        toast.error(`Update check failed: ${errorMsg}`);
      }
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

  const handlePresetChange = (preset: EmbeddingPreset) => {
    const info = PRESET_INFO[preset];
    setEmbeddingConfig((prev) => ({
      ...prev,
      preset,
      api_base: info.api_base,
      model: info.model,
      api_key: info.requires_key ? prev.api_key : null,
    }));
    setConfigDirty(true);
    setProviderStatus(null);
  };

  const handleSaveEmbeddingConfig = async () => {
    setIsSavingConfig(true);
    try {
      await invoke("save_embedding_config", {
        request: {
          preset: embeddingConfig.preset,
          api_base: embeddingConfig.api_base,
          api_key: embeddingConfig.api_key || null,
          model: embeddingConfig.model,
        },
      });
      setConfigDirty(false);
      toast.success("Embedding config saved");

      // Auto-test connection after save
      setIsTesting(true);
      setProviderStatus(null);
      try {
        const result = await invoke<ProviderStatusInfo>("test_embedding_connection");
        setProviderStatus(result);
        if (result.status === "available") {
          toast.success(`Connected (dim=${result.dimension})`);
        } else {
          toast.error(result.message || "Connection failed");
        }
      } catch (testErr) {
        toast.error(`Test failed: ${String(testErr)}`);
      } finally {
        setIsTesting(false);
      }
    } catch (err) {
      toast.error(`Failed to save config: ${String(err)}`);
    } finally {
      setIsSavingConfig(false);
    }
  };

  const handleTestConnection = async () => {
    // Save first if dirty
    if (configDirty) {
      await handleSaveEmbeddingConfig();
    }
    setIsTesting(true);
    setProviderStatus(null);
    try {
      const result = await invoke<ProviderStatusInfo>("test_embedding_connection");
      setProviderStatus(result);
      if (result.status === "available") {
        toast.success(`Connected (dim=${result.dimension})`);
      } else {
        toast.error(result.message || "Connection failed");
      }
    } catch (err) {
      toast.error(`Test failed: ${String(err)}`);
    } finally {
      setIsTesting(false);
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

            {/* Embedding Provider Section */}
            <div className="mb-4">
              <h3 className="mb-2 text-xs font-medium uppercase text-[var(--text-secondary)]">
                Embedding Provider
              </h3>
              <div className="space-y-3 rounded-lg bg-[var(--bg-card)] p-3">
                {/* Provider dropdown */}
                <div>
                  <label className="mb-1.5 block text-xs text-[var(--text-secondary)]">
                    Provider
                  </label>
                  <select
                    value={embeddingConfig.preset}
                    onChange={(e) => handlePresetChange(e.target.value as EmbeddingPreset)}
                    className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                  >
                    {(Object.keys(PRESET_INFO) as EmbeddingPreset[]).map((p) => (
                      <option key={p} value={p}>
                        {PRESET_INFO[p].label}
                      </option>
                    ))}
                  </select>
                  <p className="mt-1 text-xs text-[var(--text-secondary)]">
                    {PRESET_INFO[embeddingConfig.preset].description}
                  </p>
                  {embeddingConfig.preset === "ollama" && ollamaAvailable !== null && (
                    <p
                      className={`mt-0.5 text-xs ${ollamaAvailable ? "text-[var(--success)]" : "text-red-400"}`}
                    >
                      {ollamaAvailable ? "Ollama detected" : "Ollama not running"}
                    </p>
                  )}
                </div>

                {/* API Base - only show for Custom */}
                {embeddingConfig.preset === "custom" && (
                  <div>
                    <label className="mb-1 block text-xs text-[var(--text-secondary)]">
                      API Base URL
                    </label>
                    <input
                      type="text"
                      value={embeddingConfig.api_base}
                      onChange={(e) => {
                        setEmbeddingConfig((prev) => ({ ...prev, api_base: e.target.value }));
                        setConfigDirty(true);
                      }}
                      placeholder="http://localhost:8000/v1"
                      className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                    />
                  </div>
                )}

                {/* Model */}
                <div>
                  <label className="mb-1 block text-xs text-[var(--text-secondary)]">Model</label>
                  {embeddingConfig.preset === "ollama" &&
                  ollamaAvailable &&
                  ollamaModels.length > 0 ? (
                    <div className="space-y-1.5">
                      <select
                        value={
                          ollamaModels.some(
                            (m) =>
                              m === embeddingConfig.model ||
                              m.startsWith(`${embeddingConfig.model}:`)
                          )
                            ? embeddingConfig.model
                            : "__custom__"
                        }
                        onChange={(e) => {
                          if (e.target.value !== "__custom__") {
                            setEmbeddingConfig((prev) => ({
                              ...prev,
                              model: e.target.value,
                            }));
                            setConfigDirty(true);
                          }
                        }}
                        className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                      >
                        {ollamaModels.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                        {!ollamaModels.some(
                          (m) =>
                            m === embeddingConfig.model || m.startsWith(`${embeddingConfig.model}:`)
                        ) && (
                          <option value="__custom__">
                            {embeddingConfig.model} (not installed)
                          </option>
                        )}
                      </select>
                      {!ollamaModels.some(
                        (m) =>
                          m === embeddingConfig.model || m.startsWith(`${embeddingConfig.model}:`)
                      ) && (
                        <p className="text-xs text-amber-400">
                          Run: ollama pull {embeddingConfig.model}
                        </p>
                      )}
                    </div>
                  ) : PRESET_INFO[embeddingConfig.preset].models.length > 0 ? (
                    <select
                      value={
                        PRESET_INFO[embeddingConfig.preset].models.includes(embeddingConfig.model)
                          ? embeddingConfig.model
                          : "__custom__"
                      }
                      onChange={(e) => {
                        if (e.target.value !== "__custom__") {
                          setEmbeddingConfig((prev) => ({ ...prev, model: e.target.value }));
                          setConfigDirty(true);
                        }
                      }}
                      className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                    >
                      {PRESET_INFO[embeddingConfig.preset].models.map((m) => (
                        <option key={m} value={m}>
                          {m}
                        </option>
                      ))}
                      {!PRESET_INFO[embeddingConfig.preset].models.includes(
                        embeddingConfig.model
                      ) && <option value="__custom__">{embeddingConfig.model} (custom)</option>}
                    </select>
                  ) : (
                    <input
                      type="text"
                      value={embeddingConfig.model}
                      onChange={(e) => {
                        setEmbeddingConfig((prev) => ({ ...prev, model: e.target.value }));
                        setConfigDirty(true);
                      }}
                      placeholder="model-name"
                      className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                    />
                  )}
                </div>

                {/* API Key */}
                {PRESET_INFO[embeddingConfig.preset].requires_key && (
                  <div>
                    <label className="mb-1 block text-xs text-[var(--text-secondary)]">
                      API Key
                    </label>
                    <input
                      type="password"
                      value={embeddingConfig.api_key || ""}
                      onChange={(e) => {
                        setEmbeddingConfig((prev) => ({
                          ...prev,
                          api_key: e.target.value || null,
                        }));
                        setConfigDirty(true);
                      }}
                      placeholder={PRESET_INFO[embeddingConfig.preset].key_placeholder}
                      className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                    />
                  </div>
                )}

                {/* Optional API key for custom */}
                {embeddingConfig.preset === "custom" && (
                  <div>
                    <label className="mb-1 block text-xs text-[var(--text-secondary)]">
                      API Key (optional)
                    </label>
                    <input
                      type="password"
                      value={embeddingConfig.api_key || ""}
                      onChange={(e) => {
                        setEmbeddingConfig((prev) => ({
                          ...prev,
                          api_key: e.target.value || null,
                        }));
                        setConfigDirty(true);
                      }}
                      placeholder="..."
                      className="w-full rounded-md border border-[var(--border)] bg-[var(--bg-primary)] px-2.5 py-1.5 text-xs focus:border-[var(--accent)] focus:outline-none"
                    />
                  </div>
                )}

                {/* Status indicator */}
                {providerStatus && (
                  <div
                    className={`flex items-center gap-1.5 text-xs ${
                      providerStatus.status === "available"
                        ? "text-[var(--success)]"
                        : "text-red-400"
                    }`}
                  >
                    <div
                      className={`h-2 w-2 rounded-full ${
                        providerStatus.status === "available" ? "bg-[var(--success)]" : "bg-red-400"
                      }`}
                    />
                    {providerStatus.status === "available"
                      ? `Connected (dim=${providerStatus.dimension})`
                      : providerStatus.message || "Connection failed"}
                  </div>
                )}

                {/* Action buttons */}
                <div className="flex gap-2">
                  {configDirty ? (
                    <button
                      type="button"
                      onClick={handleSaveEmbeddingConfig}
                      disabled={isSavingConfig || isTesting}
                      className="flex-1 rounded-md bg-[var(--accent)] py-1.5 text-xs font-medium text-white disabled:opacity-50"
                    >
                      {isSavingConfig ? "Saving..." : isTesting ? "Testing..." : "Save & Test"}
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleTestConnection}
                      disabled={isTesting}
                      className="flex-1 rounded-md border border-[var(--border)] py-1.5 text-xs font-medium hover:bg-[var(--bg-primary)] disabled:opacity-50"
                    >
                      {isTesting ? "Testing..." : "Test Connection"}
                    </button>
                  )}
                </div>
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

// Tabs
type MainTab = "sessions" | "search";

// Search result type matching Tauri SearchResultResponse
interface SearchResult {
  session_id: string;
  source: string;
  title: string | null;
  chunk_content: string;
  score: number;
}

interface EmbedResponse {
  sessions_processed: number;
  chunks_created: number;
  sessions_skipped: number;
  errors: number;
}

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

  // Tab & search state
  const [activeTab, setActiveTab] = useState<MainTab>("sessions");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [isEmbedding, setIsEmbedding] = useState(false);
  const [embedStats, setEmbedStats] = useState<{
    total_chunks: number;
    total_sessions: number;
  } | null>(null);

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

  // biome-ignore lint/correctness/useExhaustiveDependencies: init effect runs once on mount
  useEffect(() => {
    const initialize = async () => {
      try {
        // Config is still loaded to check setup, but not saved to state because UI has removed Settings
        await invoke<AppConfig>("get_config");
        await loadSessions();
        loadEmbedStats();
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

  // Search handler
  const handleSearch = async () => {
    const q = searchQuery.trim();
    if (!q) return;
    setIsSearching(true);
    try {
      const results = await invoke<SearchResult[]>("search_semantic", {
        query: q,
        limit: 20,
      });
      setSearchResults(results);
      if (results.length === 0) {
        toast.info("No results found. Try embedding sessions first.");
      }
    } catch (err) {
      toast.error(`Search failed: ${String(err)}`);
    } finally {
      setIsSearching(false);
    }
  };

  // Embed handler
  const handleEmbed = async () => {
    if (isEmbedding) return;
    setIsEmbedding(true);
    toast.info("Embedding sessions... This may take a while.");
    try {
      const result = await invoke<EmbedResponse>("embed_sessions");
      toast.success(
        `Embedded ${result.sessions_processed} sessions (${result.chunks_created} chunks)`
      );
      // Refresh stats
      loadEmbedStats();
    } catch (err) {
      toast.error(`Embedding failed: ${String(err)}`);
    } finally {
      setIsEmbedding(false);
    }
  };

  // Load embedding stats
  const loadEmbedStats = async () => {
    try {
      const stats = await invoke<{ total_chunks: number; total_sessions: number }>(
        "embedding_stats"
      );
      setEmbedStats(stats);
    } catch {
      // Ignore - stats are optional
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
    // Initial sync after 10s (give app time to fully load)
    const timeout = setTimeout(() => syncRef.current(), 10000);
    // Periodic sync every 5 minutes (300000ms) to reduce RAM/CPU usage
    const interval = setInterval(() => syncRef.current(), 300000);

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
      <div className="border-b border-[var(--border)]">
        <div className="flex items-center justify-between px-4 py-3">
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

        {/* Tabs */}
        <div className="flex gap-0 px-4">
          <button
            type="button"
            onClick={() => setActiveTab("sessions")}
            className={`border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === "sessions"
                ? "border-[var(--accent)] text-[var(--accent)]"
                : "border-transparent text-[var(--text-secondary)] hover:text-white"
            }`}
          >
            Sessions
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("search")}
            className={`border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === "search"
                ? "border-[var(--accent)] text-[var(--accent)]"
                : "border-transparent text-[var(--text-secondary)] hover:text-white"
            }`}
          >
            Search
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {/* ===== Sessions Tab ===== */}
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

        {/* ===== Search Tab ===== */}
        {activeTab === "search" && (
          <div className="space-y-4">
            {/* Search Input */}
            <div className="flex gap-2">
              <div className="relative flex-1">
                <svg
                  className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[var(--text-secondary)]"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <title>Search</title>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                  />
                </svg>
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                  placeholder="Semantic search across all conversations..."
                  className="w-full rounded-lg border border-[var(--border)] bg-[var(--bg-card)] py-2 pl-9 pr-3 text-sm placeholder:text-[var(--text-secondary)] focus:border-[var(--accent)] focus:outline-none"
                />
              </div>
              <button
                type="button"
                onClick={handleSearch}
                disabled={isSearching || !searchQuery.trim()}
                className="rounded-lg bg-[var(--accent)] px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
              >
                {isSearching ? (
                  <div className="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
                ) : (
                  "Search"
                )}
              </button>
            </div>

            {/* Embed Controls */}
            <div className="glass rounded-xl px-4 py-3">
              <div className="flex items-center justify-between">
                <div className="text-sm">
                  <span className="text-[var(--text-secondary)]">Embedding index: </span>
                  {embedStats ? (
                    <span>
                      {embedStats.total_chunks} chunks from {embedStats.total_sessions} sessions
                    </span>
                  ) : (
                    <span className="text-[var(--text-secondary)]">Not indexed</span>
                  )}
                </div>
                <button
                  type="button"
                  onClick={handleEmbed}
                  disabled={isEmbedding}
                  className="rounded-lg border border-[var(--border)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--bg-card)] disabled:opacity-50"
                >
                  {isEmbedding ? (
                    <span className="flex items-center gap-1.5">
                      <div className="h-3 w-3 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
                      Embedding...
                    </span>
                  ) : (
                    "Build Index"
                  )}
                </button>
              </div>
              {!embedStats && !isEmbedding && (
                <p className="mt-2 text-xs text-[var(--text-secondary)]">
                  Configure embedding provider in Settings, then click Build Index.
                </p>
              )}
            </div>

            {/* Search Results */}
            {searchResults.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs text-[var(--text-secondary)]">
                  {searchResults.length} result{searchResults.length !== 1 ? "s" : ""}
                </p>
                {searchResults.map((result, index) => (
                  <div
                    key={`${result.session_id}-${index}`}
                    className="glass cursor-default rounded-xl p-4"
                  >
                    <div className="mb-2 flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium">
                          {result.title || result.session_id}
                        </p>
                        <p className="text-xs text-[var(--text-secondary)]">{result.source}</p>
                      </div>
                      <span className="shrink-0 rounded bg-[var(--accent)]/20 px-1.5 py-0.5 text-xs text-[var(--accent)]">
                        {(result.score * 100).toFixed(0)}%
                      </span>
                    </div>
                    <p className="line-clamp-4 text-xs leading-relaxed text-[var(--text-secondary)]">
                      {result.chunk_content}
                    </p>
                  </div>
                ))}
              </div>
            )}

            {/* Empty state */}
            {!isSearching && searchResults.length === 0 && searchQuery.trim() === "" && (
              <div className="py-8 text-center text-[var(--text-secondary)]">
                <svg
                  className="mx-auto mb-3 h-12 w-12 opacity-30"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <title>Search</title>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={1.5}
                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                  />
                </svg>
                {embedStats && embedStats.total_chunks > 0 ? (
                  <>
                    <p className="text-sm">Search your AI conversation history</p>
                    <p className="mt-1 text-xs">
                      Type a query above to search semantically across {embedStats.total_sessions}{" "}
                      sessions.
                    </p>
                  </>
                ) : (
                  <div className="space-y-3 text-left">
                    <p className="text-center text-sm font-medium text-[var(--text-primary)]">
                      Get started with semantic search
                    </p>
                    <div className="mx-auto max-w-xs space-y-2">
                      <div className="flex items-start gap-2 rounded-lg bg-[var(--bg-card)] p-2.5">
                        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-xs font-bold text-white">
                          1
                        </span>
                        <div>
                          <p className="text-xs font-medium text-[var(--text-primary)]">
                            Configure Embedding
                          </p>
                          <p className="text-xs">
                            Open Settings and choose a provider (Ollama, OpenAI, Gemini, Mistral, or
                            custom).
                          </p>
                        </div>
                      </div>
                      <div className="flex items-start gap-2 rounded-lg bg-[var(--bg-card)] p-2.5">
                        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-xs font-bold text-white">
                          2
                        </span>
                        <div>
                          <p className="text-xs font-medium text-[var(--text-primary)]">
                            Test Connection
                          </p>
                          <p className="text-xs">Verify the provider is reachable and working.</p>
                        </div>
                      </div>
                      <div className="flex items-start gap-2 rounded-lg bg-[var(--bg-card)] p-2.5">
                        <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-xs font-bold text-white">
                          3
                        </span>
                        <div>
                          <p className="text-xs font-medium text-[var(--text-primary)]">
                            Build Index
                          </p>
                          <p className="text-xs">
                            Click the button above to embed all parsed sessions.
                          </p>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
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

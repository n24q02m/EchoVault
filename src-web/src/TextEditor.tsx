import { json } from "@codemirror/lang-json";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import CodeMirror from "@uiw/react-codemirror";
import { useEffect, useState } from "react";

interface TextEditorProps {
  path: string;
  title?: string;
  onClose: () => void;
}

// Custom theme để làm to fold gutter
const foldGutterTheme = EditorView.theme({
  ".cm-foldGutter": {
    width: "16px",
  },
  ".cm-foldGutter .cm-gutterElement": {
    fontSize: "16px",
    padding: "0 2px",
    cursor: "pointer",
  },
});

/**
 * Text Editor sử dụng CodeMirror.
 * Hỗ trợ JSON syntax highlighting, virtualized rendering sẵn.
 */
export function TextEditor({ path, title, onClose }: TextEditorProps) {
  const [content, setContent] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Load file content
  useEffect(() => {
    const loadContent = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const text = await invoke<string>("read_file_content", { path });
        setContent(text);
      } catch (err) {
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    };
    loadContent();
  }, [path]);

  // Detect file type for extensions
  const isJson = path.endsWith(".json");
  const extensions = [foldGutterTheme];
  if (isJson) {
    extensions.push(json());
  }

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-[var(--bg-primary)]">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate font-semibold">{title || path.split("/").pop()}</h2>
          <p className="truncate text-xs text-[var(--text-secondary)]">{path}</p>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="ml-4 rounded-lg bg-[var(--bg-card)] p-2 hover:bg-[var(--border)]"
          aria-label="Close"
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

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {isLoading && (
          <div className="flex h-full items-center justify-center">
            <div className="h-8 w-8 animate-spin rounded-full border-2 border-[var(--accent)] border-t-transparent" />
          </div>
        )}

        {error && (
          <div className="flex h-full flex-col items-center justify-center p-4">
            <p className="text-red-400">{error}</p>
            <button
              type="button"
              onClick={onClose}
              className="mt-4 rounded-lg bg-[var(--accent)] px-4 py-2 text-white"
            >
              Close
            </button>
          </div>
        )}

        {!isLoading && !error && (
          <CodeMirror
            value={content}
            height="100%"
            theme={oneDark}
            extensions={extensions}
            editable={false}
            basicSetup={{
              lineNumbers: true,
              highlightActiveLineGutter: false,
              highlightActiveLine: false,
              foldGutter: true,
            }}
            className="h-full overflow-auto"
          />
        )}
      </div>

      {/* Footer */}
      <div className="border-t border-[var(--border)] px-4 py-2 text-xs text-[var(--text-secondary)]">
        {content && (
          <span>
            {content.split("\n").length} lines | {content.length} characters
          </span>
        )}
      </div>
    </div>
  );
}

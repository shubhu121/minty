import { useState, useEffect, useCallback } from "react";
import {
  checkOllamaStatus,
  setOllamaModel,
  setOllamaCompletionModel,
  OllamaStatus,
  importVault,
  ImportResult
} from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

interface AiSettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function AiSettingsPanel({
  isOpen,
  onClose,
}: AiSettingsPanelProps) {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);

  const fetchStatus = useCallback(async () => {
    setIsLoading(true);
    try {
      const s = await checkOllamaStatus();
      setStatus(s);
    } catch (err) {
      console.error("[AiSettings]", err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) fetchStatus();
  }, [isOpen, fetchStatus]);

  const handleModelChange = async (model: string) => {
    setIsSwitching(true);
    try {
      await setOllamaModel(model);
      setStatus((prev) =>
        prev ? { ...prev, active_model: model } : prev
      );
    } catch (err) {
      console.error("[AiSettings] Failed to set model:", err);
    } finally {
      setIsSwitching(false);
    }
  };

  const handleCompletionModelChange = async (model: string) => {
    setIsSwitching(true);
    try {
      await setOllamaCompletionModel(model);
      setStatus((prev) =>
        prev ? { ...prev, completion_model: model } : prev
      );
    } catch (err) {
      console.error("[AiSettings] Failed to set completion model:", err);
    } finally {
      setIsSwitching(false);
    }
  };

  const handleImport = async (type: "obsidian" | "folder") => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Select ${type === "obsidian" ? "Obsidian Vault" : "Markdown Folder"}`,
      });

      if (!selected) return;

      setIsImporting(true);
      setImportResult(null);

      const result = await importVault(selected as string, type);
      setImportResult(result);
    } catch (err) {
      console.error("[AiSettings] Failed to import vault:", err);
      // fallback error
      setImportResult({ imported: 0, skipped: 0, errors: [String(err)] });
    } finally {
      setIsImporting(false);
    }
  };


  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)", backdropFilter: "blur(4px)" }}
    >
      <div
        className="w-full max-w-[420px] rounded-2xl overflow-hidden shadow-2xl"
        style={{
          background: "var(--background)",
          border: "1px solid var(--border)",
          animation: "smartSearchAppear 0.2s ease-out",
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-5 py-4"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          <div className="flex items-center gap-2">
            <span className="text-lg">⚙️</span>
            <span className="font-semibold text-sm">Settings</span>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg transition-colors cursor-pointer"
            style={{ color: "var(--muted-foreground)" }}
            onMouseEnter={(e) =>
              (e.currentTarget.style.background = "var(--accent)")
            }
            onMouseLeave={(e) =>
              (e.currentTarget.style.background = "transparent")
            }
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M18 6L6 18" />
              <path d="M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="px-5 py-4 space-y-5">
          
          {/* Import Vault */}
          <div className="pt-1">
            <label className="text-xs font-medium mb-3 block" style={{ color: "var(--muted-foreground)" }}>
              IMPORT VAULT
            </label>
            <div className="flex gap-2">
              <button
                onClick={() => handleImport("obsidian")}
                disabled={isImporting}
                className="flex-1 py-2 rounded-xl text-sm font-medium transition-all cursor-pointer bg-[var(--accent)] hover:bg-[var(--border)] text-[var(--text-color)] disabled:opacity-50"
              >
                {isImporting ? "Importing..." : "Import Obsidian"}
              </button>
              <button
                onClick={() => handleImport("folder")}
                disabled={isImporting}
                className="flex-1 py-2 rounded-xl text-sm font-medium transition-all cursor-pointer bg-[var(--accent)] hover:bg-[var(--border)] text-[var(--text-color)] disabled:opacity-50"
              >
                {isImporting ? "Importing..." : "Import Folder"}
              </button>
            </div>

            {importResult && (
              <div className="mt-3 p-3 rounded-xl bg-[var(--accent)] border border-[var(--border)] text-sm space-y-1">
                <p className="font-medium text-[var(--text-color)]">Import Complete</p>
                <p className="text-[var(--muted-foreground)]">✅ {importResult.imported} notes imported and queued for AI indexing.</p>
                {importResult.skipped > 0 && (
                  <p className="text-[var(--muted-foreground)]">⏭️ {importResult.skipped} notes skipped (already exist).</p>
                )}
                {importResult.errors.length > 0 && (
                  <div className="mt-2 pt-2 border-t border-red-500/20 text-red-500 max-h-24 overflow-y-auto">
                    <p className="text-xs font-bold mb-1">Errors</p>
                    {importResult.errors.map((e, i) => (
                      <p key={i} className="text-[10px] leading-tight mb-1">{e}</p>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="border-t pt-4" style={{ borderColor: 'var(--border)' }}></div>

          {/* Ollama Status */}
          <div>
            <label
              className="text-xs font-medium mb-2 block"
              style={{ color: "var(--muted-foreground)" }}
            >
              OLLAMA STATUS
            </label>
            <div
              className="flex items-center gap-3 p-3 rounded-xl"
              style={{ background: "var(--accent)" }}
            >
              {isLoading ? (
                <>
                  <div
                    className="w-3 h-3 rounded-full border-[1.5px] animate-spin"
                    style={{
                      borderColor: "var(--muted-foreground)",
                      borderTopColor: "transparent",
                    }}
                  />
                  <span className="text-sm">Checking...</span>
                </>
              ) : status?.available ? (
                <>
                  <span
                    className="relative flex h-3 w-3 items-center justify-center"
                    title="Ollama is running"
                  >
                    <span
                      className="absolute inline-flex h-3 w-3 rounded-full opacity-40 animate-ping"
                      style={{
                        background: "#22c55e",
                        animationDuration: "2s",
                      }}
                    />
                    <span
                      className="relative inline-flex rounded-full h-2.5 w-2.5"
                      style={{ background: "#22c55e" }}
                    />
                  </span>
                  <div>
                    <span className="text-sm font-medium">Connected</span>
                    <span
                      className="text-xs ml-2"
                      style={{ color: "var(--muted-foreground)" }}
                    >
                      {status.models.length} model
                      {status.models.length !== 1 ? "s" : ""} available
                    </span>
                  </div>
                </>
              ) : (
                <>
                  <span
                    className="inline-flex rounded-full h-2.5 w-2.5"
                    style={{ background: "#ef4444" }}
                  />
                  <div className="flex-1">
                    <span className="text-sm font-medium">Not connected</span>
                    <p
                      className="text-xs mt-0.5"
                      style={{ color: "var(--muted-foreground)" }}
                    >
                      Install and run{" "}
                      <a
                         href="https://ollama.ai"
                         target="_blank"
                         rel="noopener noreferrer"
                         className="underline"
                         style={{ color: "#3b82f6" }}
                      >
                         ollama.ai
                      </a>{" "}
                      for AI features
                    </p>
                  </div>
                </>
              )}
              <button
                onClick={fetchStatus}
                className="ml-auto p-1.5 rounded-lg transition-colors cursor-pointer"
                style={{ color: "var(--muted-foreground)" }}
                title="Refresh"
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  className={isLoading ? "animate-spin" : ""}
                >
                  <path d="M21 12a9 9 0 11-6.219-8.56" />
                  <polyline points="21 3 21 9 15 9" />
                </svg>
              </button>
            </div>
          </div>

          {/* Model Selector */}
          {status?.available && status.models.length > 0 && (
            <div className="space-y-4">
              <div>
                <label
                  className="text-xs font-medium mb-2 block"
                  style={{ color: "var(--muted-foreground)" }}
                >
                  CHAT MODEL (RAG)
                </label>
                <div className="space-y-1.5">
                  {status.models.map((model) => (
                  <button
                    key={model}
                    onClick={() => handleModelChange(model)}
                    disabled={isSwitching}
                    className="w-full text-left px-3 py-2.5 rounded-xl text-sm transition-all duration-150 cursor-pointer flex items-center justify-between"
                    style={{
                      background:
                        model === status.active_model
                          ? "var(--primary)"
                          : "var(--accent)",
                      color:
                        model === status.active_model
                          ? "var(--primary-foreground)"
                          : "var(--text-color)",
                      border:
                        model === status.active_model
                          ? "1px solid var(--primary)"
                          : "1px solid transparent",
                      opacity: isSwitching ? 0.6 : 1,
                    }}
                  >
                    <span className="font-mono text-xs">{model}</span>
                    {model === status.active_model && (
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="3"
                      >
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    )}
                  </button>
                ))}
              </div>
              </div>
              
              <div>
                <label
                  className="text-xs font-medium mb-2 block"
                  style={{ color: "var(--muted-foreground)" }}
                >
                  COMPLETION MODEL (GHOST TEXT)
                </label>
                <div className="space-y-1.5">
                  {status.models.map((model) => (
                    <button
                      key={`completion-${model}`}
                      onClick={() => handleCompletionModelChange(model)}
                      disabled={isSwitching}
                      className="w-full text-left px-3 py-2.5 rounded-xl text-sm transition-all duration-150 cursor-pointer flex items-center justify-between"
                      style={{
                        background: model === status.completion_model ? "var(--primary)" : "var(--accent)",
                        color: model === status.completion_model ? "var(--primary-foreground)" : "var(--text-color)",
                        border: model === status.completion_model ? "1px solid var(--primary)" : "1px solid transparent",
                        opacity: isSwitching ? 0.6 : 1,
                      }}
                    >
                      <span className="font-mono text-xs">{model}</span>
                      {model === status.completion_model && (
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                      )}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Info */}
          <p
            className="text-[11px] leading-relaxed"
            style={{ color: "var(--muted-foreground)", opacity: 0.7 }}
          >
            Smart Notes uses local AI via Ollama for privacy-first
            intelligent features. No data ever leaves your machine.
          </p>
        </div>
      </div>
    </div>
  );
}

import { useState, useEffect, useRef, useCallback } from "react";
import {
  searchNotes,
  RetrievedChunk,
  SearchMode,
} from "../lib/tauri";

interface SmartSearchProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenNote: (noteId: string) => void;
}

const MODE_LABELS: { mode: SearchMode; label: string; icon: string }[] = [
  { mode: "hybrid", label: "Hybrid", icon: "⚡" },
  { mode: "semantic", label: "Semantic", icon: "🧠" },
  { mode: "keyword", label: "Keyword", icon: "🔤" },
];

export default function SmartSearch({
  isOpen,
  onClose,
  onOpenNote,
}: SmartSearchProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<RetrievedChunk[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searchMode, setSearchMode] = useState<SearchMode>("hybrid");
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<number | null>(null);

  // Focus input when opened
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
      setQuery("");
      setResults([]);
      setSelectedIndex(0);
    }
  }, [isOpen]);

  // Debounced search
  const doSearch = useCallback(
    async (q: string, mode: SearchMode) => {
      if (q.trim().length < 2) {
        setResults([]);
        setIsSearching(false);
        return;
      }
      setIsSearching(true);
      try {
        const res = await searchNotes(q, 8, mode);
        setResults(res);
        setSelectedIndex(0);
      } catch (err) {
        console.error("[SmartSearch] Error:", err);
        setResults([]);
      } finally {
        setIsSearching(false);
      }
    },
    []
  );

  const handleInputChange = (value: string) => {
    setQuery(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(
      () => doSearch(value, searchMode),
      300
    );
  };

  // Re-search when mode changes (if there's a query)
  const handleModeChange = (mode: SearchMode) => {
    setSearchMode(mode);
    if (query.trim().length >= 2) {
      doSearch(query, mode);
    }
  };

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.min(prev + 1, results.length - 1));
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.max(prev - 1, 0));
    }
    if (e.key === "Enter" && results[selectedIndex]) {
      onOpenNote(results[selectedIndex].note_id);
      onClose();
    }
    // Tab switches mode
    if (e.key === "Tab") {
      e.preventDefault();
      const idx = MODE_LABELS.findIndex((m) => m.mode === searchMode);
      const next = MODE_LABELS[(idx + 1) % MODE_LABELS.length];
      handleModeChange(next.mode);
    }
  };

  // Close on outside click
  const backdropRef = useRef<HTMLDivElement>(null);
  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === backdropRef.current) onClose();
  };

  if (!isOpen) return null;

  return (
    <div
      ref={backdropRef}
      onClick={handleBackdropClick}
      className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]"
      style={{ background: "rgba(0,0,0,0.4)", backdropFilter: "blur(4px)" }}
    >
      <div
        className="w-full max-w-[600px] rounded-2xl overflow-hidden shadow-2xl"
        style={{
          background: "var(--background)",
          border: "1px solid var(--border)",
          animation: "smartSearchAppear 0.2s ease-out",
        }}
      >
        {/* Search input */}
        <div
          className="flex items-center gap-3 px-5 py-4"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ opacity: 0.5, flexShrink: 0 }}
          >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.35-4.35" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => handleInputChange(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search your notes..."
            className="flex-1 text-base outline-none"
            style={{
              background: "transparent",
              color: "var(--text-color)",
              caretColor: "var(--primary)",
            }}
            spellCheck={false}
          />
          {isSearching && (
            <div
              className="w-4 h-4 rounded-full border-2 animate-spin"
              style={{
                borderColor: "var(--muted-foreground)",
                borderTopColor: "transparent",
              }}
            />
          )}
          <kbd
            className="text-xs px-1.5 py-0.5 rounded"
            style={{
              background: "var(--muted)",
              color: "var(--muted-foreground)",
              fontSize: "11px",
            }}
          >
            ESC
          </kbd>
        </div>

        {/* Mode toggle */}
        <div
          className="flex items-center gap-1 px-5 py-2"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          {MODE_LABELS.map((m) => (
            <button
              key={m.mode}
              onClick={() => handleModeChange(m.mode)}
              className="px-3 py-1 rounded-lg text-xs font-medium transition-all duration-150 cursor-pointer"
              style={{
                background:
                  searchMode === m.mode ? "var(--accent)" : "transparent",
                color:
                  searchMode === m.mode
                    ? "var(--accent-foreground)"
                    : "var(--muted-foreground)",
                border:
                  searchMode === m.mode
                    ? "1px solid var(--border)"
                    : "1px solid transparent",
              }}
            >
              {m.icon} {m.label}
            </button>
          ))}
          <span
            className="ml-auto text-[10px]"
            style={{ color: "var(--muted-foreground)", opacity: 0.6 }}
          >
            Tab to switch
          </span>
        </div>

        {/* Results */}
        <div
          className="max-h-[400px] overflow-y-auto"
          style={{ scrollbarWidth: "thin" }}
        >
          {results.length === 0 && query.trim().length >= 2 && !isSearching && (
            <div
              className="px-5 py-8 text-center text-sm"
              style={{ color: "var(--muted-foreground)" }}
            >
              No matching notes found
            </div>
          )}
          {results.length === 0 && query.trim().length < 2 && (
            <div
              className="px-5 py-8 text-center text-sm"
              style={{ color: "var(--muted-foreground)" }}
            >
              Type at least 2 characters to search...
            </div>
          )}
          {results.map((result, i) => {
            const score = result.rrf_score;
            return (
              <button
                key={`${result.note_id}-${result.chunk_id}-${i}`}
                onClick={() => {
                  onOpenNote(result.note_id);
                  onClose();
                }}
                onMouseEnter={() => setSelectedIndex(i)}
                className="w-full text-left px-5 py-3 transition-colors duration-100 cursor-pointer"
                style={{
                  background:
                    i === selectedIndex ? "var(--accent)" : "transparent",
                  borderBottom: "1px solid var(--border)",
                }}
              >
                <div className="flex items-center justify-between mb-1">
                  <span
                    className="font-medium text-sm"
                    style={{ color: "var(--text-color)" }}
                  >
                    {result.note_title}
                  </span>
                  <div className="flex items-center gap-1.5">
                    {result.vector_score > 0 && (
                      <span
                        className="text-[10px] px-1.5 py-0.5 rounded-full"
                        style={{
                          background: "rgba(139,92,246,0.15)",
                          color: "#8b5cf6",
                        }}
                        title="Vector similarity"
                      >
                        🧠 {Math.round(result.vector_score * 100)}%
                      </span>
                    )}
                    {result.bm25_score > 0 && (
                      <span
                        className="text-[10px] px-1.5 py-0.5 rounded-full"
                        style={{
                          background: "rgba(59,130,246,0.15)",
                          color: "#3b82f6",
                        }}
                        title="Keyword match"
                      >
                        🔤 {result.bm25_score.toFixed(1)}
                      </span>
                    )}
                    <span
                      className="text-[10px] px-1.5 py-0.5 rounded-full"
                      style={{
                        background:
                          score > 0.025
                            ? "rgba(34,197,94,0.15)"
                            : "rgba(107,114,128,0.15)",
                        color:
                          score > 0.025
                            ? "#22c55e"
                            : "var(--muted-foreground)",
                      }}
                    >
                      RRF {score.toFixed(3)}
                    </span>
                  </div>
                </div>
                {result.heading_path && (
                  <div
                    className="text-xs mb-1"
                    style={{ color: "var(--muted-foreground)" }}
                  >
                    📍 {result.heading_path}
                  </div>
                )}
                <p
                  className="text-xs line-clamp-2 leading-relaxed"
                  style={{ color: "var(--muted-foreground)", opacity: 0.8 }}
                >
                  {result.text.slice(0, 200)}
                  {result.text.length > 200 ? "..." : ""}
                </p>
              </button>
            );
          })}
        </div>

        {/* Footer hint */}
        <div
          className="flex items-center gap-4 px-5 py-2.5 text-xs"
          style={{
            borderTop: "1px solid var(--border)",
            color: "var(--muted-foreground)",
          }}
        >
          <span>
            <kbd
              className="px-1 py-0.5 rounded mr-1"
              style={{ background: "var(--muted)" }}
            >
              ↑↓
            </kbd>
            navigate
          </span>
          <span>
            <kbd
              className="px-1 py-0.5 rounded mr-1"
              style={{ background: "var(--muted)" }}
            >
              ↵
            </kbd>
            open
          </span>
          <span>
            <kbd
              className="px-1 py-0.5 rounded mr-1"
              style={{ background: "var(--muted)" }}
            >
              Tab
            </kbd>
            mode
          </span>
          <span className="ml-auto">
            {searchMode === "hybrid"
              ? "Vector + BM25 fusion"
              : searchMode === "semantic"
                ? "Vector similarity only"
                : "Keyword match only"}
          </span>
        </div>
      </div>
    </div>
  );
}

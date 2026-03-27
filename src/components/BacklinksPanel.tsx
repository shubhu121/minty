import { useState, useEffect } from "react";
import { getBacklinks, Backlink } from "../lib/tauri";

interface BacklinksPanelProps {
  noteId: string | null;
  isOpen: boolean;
  onClose: () => void;
  onOpenNote: (noteId: string) => void;
}

export default function BacklinksPanel({
  noteId,
  isOpen,
  onClose,
  onOpenNote,
}: BacklinksPanelProps) {
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    if (!isOpen || !noteId) {
      setBacklinks([]);
      return;
    }
    setIsLoading(true);
    getBacklinks(noteId)
      .then(setBacklinks)
      .catch((err) => console.error("[Backlinks]", err))
      .finally(() => setIsLoading(false));
  }, [noteId, isOpen]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed right-0 top-0 h-full w-[320px] z-40 flex flex-col"
      style={{
        background: "var(--background)",
        borderLeft: "1px solid var(--border)",
        boxShadow: "-4px 0 24px rgba(0,0,0,0.08)",
        animation: "slideInRight 0.25s ease-out",
      }}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between px-5 py-4"
        style={{ borderBottom: "1px solid var(--border)" }}
      >
        <div className="flex items-center gap-2">
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            style={{ opacity: 0.6 }}
          >
            <path d="M9 17H7A5 5 0 017 7h2" />
            <path d="M15 7h2a5 5 0 010 10h-2" />
            <line x1="8" y1="12" x2="16" y2="12" />
          </svg>
          <span className="font-semibold text-sm">
            Backlinks
          </span>
          <span
            className="text-xs px-1.5 py-0.5 rounded-full"
            style={{
              background: "var(--accent)",
              color: "var(--accent-foreground)",
            }}
          >
            {backlinks.length}
          </span>
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded-md transition-colors cursor-pointer"
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

      {/* Body */}
      <div
        className="flex-1 overflow-y-auto p-4"
        style={{ scrollbarWidth: "thin" }}
      >
        {isLoading && (
          <div
            className="flex items-center justify-center py-8"
            style={{ color: "var(--muted-foreground)" }}
          >
            <div
              className="w-5 h-5 rounded-full border-2 animate-spin mr-2"
              style={{
                borderColor: "var(--muted-foreground)",
                borderTopColor: "transparent",
              }}
            />
            Loading...
          </div>
        )}

        {!isLoading && backlinks.length === 0 && (
          <div
            className="text-center py-8 text-sm"
            style={{ color: "var(--muted-foreground)" }}
          >
            <div className="text-2xl mb-2">🔗</div>
            No backlinks yet
            <p className="text-xs mt-1 opacity-70">
              Link to this note from others using [[wikilinks]]
            </p>
          </div>
        )}

        {backlinks.map((bl, i) => (
          <button
            key={`${bl.source_id}-${i}`}
            onClick={() => onOpenNote(bl.source_id)}
            className="w-full text-left p-3 rounded-xl mb-2 transition-all duration-150 cursor-pointer"
            style={{
              background: "var(--accent)",
              border: "1px solid transparent",
            }}
            onMouseEnter={(e) =>
              (e.currentTarget.style.borderColor = "var(--border)")
            }
            onMouseLeave={(e) =>
              (e.currentTarget.style.borderColor = "transparent")
            }
          >
            <div className="flex items-center gap-2 mb-1">
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                style={{ opacity: 0.5, flexShrink: 0 }}
              >
                <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
                <polyline points="14,2 14,8 20,8" />
              </svg>
              <span className="font-medium text-sm truncate">
                {bl.source_title}
              </span>
            </div>
            <div
              className="text-xs ml-[22px]"
              style={{ color: "var(--muted-foreground)" }}
            >
              {bl.anchor_text && (
                <span>
                  via "<em>{bl.anchor_text}</em>"
                </span>
              )}
              {!bl.anchor_text && (
                <span className="opacity-60">{bl.source_path}</span>
              )}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

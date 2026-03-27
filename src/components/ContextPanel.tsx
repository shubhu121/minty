import { useState, useEffect } from "react";
import { getBacklinks, Backlink, getRelatedNotes, RelatedNote } from "../lib/tauri";

interface ContextPanelProps {
  noteId: string | null;
  isOpen: boolean;
  onClose: () => void;
  onOpenNote: (noteId: string) => void;
}

export default function ContextPanel({
  noteId,
  isOpen,
  onClose,
  onOpenNote,
}: ContextPanelProps) {
  const [activeTab, setActiveTab] = useState<"related" | "backlinks">("related");
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const [relatedNotes, setRelatedNotes] = useState<RelatedNote[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    if (!isOpen || !noteId) {
      setBacklinks([]);
      setRelatedNotes([]);
      return;
    }
    
    let isCancelled = false;

    const fetchData = async () => {
      setIsLoading(true);
      try {
        const bls = await getBacklinks(noteId);
        if (!isCancelled) setBacklinks(bls);

        const related = await getRelatedNotes(noteId, 5);
        if (!isCancelled) setRelatedNotes(related);
      } catch (err) {
        console.error("[ContextPanel]", err);
      } finally {
        if (!isCancelled) setIsLoading(false);
      }
    };

    // Minor debounce because note saves might trigger rapid changes, though normally fetching happens on open/change
    const timer = setTimeout(fetchData, 800);
    return () => {
      isCancelled = true;
      clearTimeout(timer);
    };
  }, [noteId, isOpen]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed right-0 top-0 h-full w-[380px] z-40 flex flex-col"
      style={{
        background: "var(--background)",
        borderLeft: "1px solid var(--border)",
        boxShadow: "-4px 0 24px rgba(0,0,0,0.08)",
        animation: "slideInRight 0.25s ease-out",
      }}
    >
      <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)] shrink-0">
        <h2 className="font-semibold text-sm text-[var(--text-color)]">Context</h2>
        <button
          onClick={onClose}
          className="p-1.5 rounded-lg transition-colors cursor-pointer hover:bg-[var(--accent)] text-[var(--muted-foreground)]"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18" />
            <path d="M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div className="flex border-b border-[var(--border)] px-2 pt-2 shrink-0">
        <button
          onClick={() => setActiveTab("related")}
          className={`flex-1 pb-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "related"
              ? "border-[var(--primary)] text-[var(--primary)]"
              : "border-transparent text-[var(--muted-foreground)] hover:text-[var(--text-color)]"
          }`}
        >
          Related Notes
        </button>
        <button
          onClick={() => setActiveTab("backlinks")}
          className={`flex-1 pb-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "backlinks"
              ? "border-[var(--primary)] text-[var(--primary)]"
              : "border-transparent text-[var(--muted-foreground)] hover:text-[var(--text-color)]"
          }`}
        >
          Backlinks ({backlinks.length})
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-3" style={{ scrollbarWidth: "thin" }}>
        {isLoading && (
          <div className="flex justify-center py-6 text-[var(--muted-foreground)]">
             <div className="w-5 h-5 rounded-full border-2 animate-spin mr-2" style={{ borderColor: "var(--muted-foreground)", borderTopColor: "transparent" }} />
             <span>Loading context...</span>
          </div>
        )}

        {!isLoading && activeTab === "related" && relatedNotes.length === 0 && (
          <div className="text-center py-8 text-sm text-[var(--muted-foreground)]">
            <div className="text-2xl mb-2">🧠</div>
            No related notes found
            <p className="text-xs mt-1 opacity-70">
              Needs more indexable notes to map semantic concepts.
            </p>
          </div>
        )}

        {!isLoading && activeTab === "related" && relatedNotes.map((note) => (
          <div
            key={note.note_id}
            onClick={() => onOpenNote(note.note_id)}
            className="p-3 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] cursor-pointer transition-colors"
          >
            <div className="flex justify-between items-start mb-1">
              <h4 className="text-[13px] font-semibold text-[var(--text-color)]">{note.title}</h4>
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--accent)] font-medium" style={{ color: "var(--primary)" }}>
                {(note.similarity_score * 100).toFixed(0)}%
              </span>
            </div>
            <p className="text-[12px] text-[var(--muted-foreground)] line-clamp-2 leading-relaxed mt-1">
              {note.preview}
            </p>
          </div>
        ))}

        {!isLoading && activeTab === "backlinks" && backlinks.length === 0 && (
          <div className="text-center py-8 text-sm text-[var(--muted-foreground)]">
            <div className="text-2xl mb-2">🔗</div>
            No backlinks yet
            <p className="text-xs mt-1 opacity-70">
              Link to this note from others using [[wikilinks]]
            </p>
          </div>
        )}

        {!isLoading && activeTab === "backlinks" && backlinks.map((bl, i) => (
          <div
            key={`${bl.source_id}-${i}`}
            onClick={() => onOpenNote(bl.source_id)}
            className="w-full text-left p-3 rounded-lg border border-[var(--border)] transition-all cursor-pointer hover:bg-[var(--accent)]"
          >
            <div className="flex items-center gap-2 mb-1">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="text-[var(--primary)]">
                <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
                <polyline points="14,2 14,8 20,8" />
              </svg>
              <span className="font-medium text-[13px] truncate text-[var(--text-color)]">
                {bl.source_title}
              </span>
            </div>
            <div className="text-xs mt-1.5 text-[var(--muted-foreground)]">
              {bl.anchor_text ? (
                <>via "<em className="text-[var(--text-color)]">{bl.anchor_text}</em>"</>
              ) : (
                <span className="opacity-60">{bl.source_path}</span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

import { Citation } from "../../store/rag";

interface CitationPillProps {
  citation: Citation;
  onOpenNote?: (noteId: string) => void;
}

export default function CitationPill({ citation, onOpenNote }: CitationPillProps) {
  return (
    <button
      onClick={() => onOpenNote?.(citation.note_id)}
      className="inline-flex items-center gap-1 px-2 py-0.5 mx-0.5 rounded-full text-xs font-medium cursor-pointer transition-colors"
      style={{
        background: "var(--accent)",
        border: "1px solid var(--border)",
        color: "var(--primary)",
      }}
      title={citation.note_title}
    >
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
      </svg>
      {citation.label}
    </button>
  );
}

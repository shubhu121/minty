import { SourceInfo } from "../../store/rag";

interface SourcePanelProps {
  sources: SourceInfo[];
  onOpenNote?: (id: string) => void;
}

export default function SourcePanel({ sources, onOpenNote }: SourcePanelProps) {
  if (sources.length === 0) return null;

  return (
    <div className="w-64 border-l flex flex-col h-full bg-[var(--background)]" style={{ borderColor: 'var(--border)' }}>
      <div className="px-4 py-3 border-b border-[var(--border)]">
        <h3 className="text-xs font-semibold text-[var(--muted-foreground)]">SOURCES USED</h3>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {sources.map((src, i) => (
          <div
            key={`${src.note_id}-${i}`}
            onClick={() => onOpenNote?.(src.note_id)}
            className="p-3 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] cursor-pointer transition-colors"
          >
            <div className="flex items-center gap-2 mb-1">
              <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-[var(--accent)] text-[var(--primary)]">
                {src.label}
              </span>
            </div>
            <h4 className="text-sm font-medium line-clamp-1 text-[var(--text-color)]">{src.note_title}</h4>
            {src.heading_path && (
              <p className="text-xs text-[var(--muted-foreground)] line-clamp-1 mt-0.5">
                {src.heading_path}
              </p>
            )}
            <div className="mt-2 text-[10px] text-[var(--muted-foreground)] opacity-70">
              Score: {src.rrf_score.toFixed(4)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

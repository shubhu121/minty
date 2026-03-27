import { Message } from "../../store/rag";
import CitationPill from "./CitationPill";

interface StreamingMessageProps {
  message: Message;
  onOpenNote?: (id: string) => void;
}

export default function StreamingMessage({ message, onOpenNote }: StreamingMessageProps) {
  const isUser = message.role === "user";

  return (
    <div
      className={`flex w-full mb-6 ${
        isUser ? "justify-end" : "justify-start"
      }`}
    >
      {!isUser && (
        <div className="w-8 h-8 rounded bg-[var(--accent)] text-[var(--text-color)] flex items-center justify-center mr-3 mt-1 shrink-0">
          🤖
        </div>
      )}
      
      <div
        className={`max-w-[85%] rounded-2xl px-5 py-3.5 leading-relaxed overflow-hidden text-sm ${
          isUser
            ? "bg-[var(--primary)] text-[var(--primary-foreground)] rounded-br-sm"
            : "bg-[var(--accent)] text-[var(--text-color)] rounded-tl-sm border border-[var(--border)]"
        }`}
        style={{ wordBreak: "break-word" }}
      >
        <div className="whitespace-pre-wrap font-sans space-y-2">
          {message.parts.map((p, i) => {
            if (p.type === "text") {
              return <span key={i}>{p.text}</span>;
            } else if (p.type === "citation") {
              return (
                <CitationPill
                  key={i}
                  citation={p.citation}
                  onOpenNote={onOpenNote}
                />
              );
            }
            return null;
          })}
        </div>
      </div>

      {isUser && (
        <div className="w-8 h-8 rounded bg-[var(--primary)] text-[var(--primary-foreground)] flex items-center justify-center ml-3 mt-1 shrink-0 opacity-80">
          U
        </div>
      )}
    </div>
  );
}

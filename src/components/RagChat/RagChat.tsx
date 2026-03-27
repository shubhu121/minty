import { useState } from "react";
import MessageList from "./MessageList";
import SourcePanel from "./SourcePanel";
import { useRagStream } from "../../hooks/useRagStream";
import { useRagStore } from "../../store/rag";

interface RagChatProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenNote: (id: string) => void;
}

export default function RagChat({ isOpen, onClose, onOpenNote }: RagChatProps) {
  const [input, setInput] = useState("");
  const { ask } = useRagStream();
  const isStreaming = useRagStore((state) => state.streaming);

  const activeId = useRagStore((state) => state.activeConversationId);
  const conv = useRagStore((state) =>
    activeId ? state.conversations.get(activeId) : null
  );

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isStreaming) return;
    ask(input);
    setInput("");
  };

  const handleOpenNote = (id: string) => {
    onOpenNote(id);
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)", backdropFilter: "blur(6px)" }}
    >
      <div
        className="w-[90vw] h-[85vh] max-w-6xl rounded-2xl overflow-hidden shadow-2xl flex flex-col relative"
        style={{
          background: "var(--background)",
          border: "1px solid var(--border)",
          animation: "smartSearchAppear 0.2s cubic-bezier(0.16, 1, 0.3, 1)",
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between px-6 py-4 border-b shrink-0"
          style={{ borderColor: "var(--border)" }}
        >
          <div className="flex items-center gap-3">
            <span className="text-xl">📚</span>
            <div>
              <h2 className="font-semibold text-lg text-[var(--text-color)] leading-tight">
                Ask Your Notes
              </h2>
              <span className="text-xs text-[var(--muted-foreground)]">
                Local AI RAG Pipeline
              </span>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-lg cursor-pointer hover:bg-[var(--accent)] transition-colors text-[var(--muted-foreground)]"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18" />
              <path d="M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Layout: Chat / Sources split */}
        <div className="flex flex-1 overflow-hidden">
          {/* Main Chat Area */}
          <div className="flex-1 flex flex-col relative bg-[var(--background)]">
            <MessageList onOpenNote={handleOpenNote} />

            {/* Input form fixed at bottom */}
            <div
              className="px-6 py-4 shrink-0 bg-[var(--background)] border-t"
              style={{ borderColor: "var(--border)" }}
            >
              <form onSubmit={handleSubmit} className="max-w-3xl mx-auto relative flex items-center shadow-lg rounded-2xl">
                <input
                  autoFocus
                  type="text"
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  disabled={isStreaming}
                  placeholder="What do I know about..."
                  className="w-full pl-5 pr-14 py-4 rounded-2xl text-base focus:outline-none transition-all disabled:opacity-50"
                  style={{
                    background: "var(--accent)",
                    color: "var(--text-color)",
                    border: "1px solid var(--border)",
                  }}
                />
                <button
                  type="submit"
                  disabled={isStreaming || !input.trim()}
                  className="absolute right-2 p-2.5 rounded-xl transition-all cursor-pointer shadow-sm text-white disabled:opacity-50 disabled:cursor-not-allowed"
                  style={{ background: "var(--primary)" }}
                >
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mr-0.5 ml-0.5">
                    <line x1="22" y1="2" x2="11" y2="13" />
                    <polygon points="22 2 15 22 11 13 2 9 22 2" />
                  </svg>
                </button>
              </form>
            </div>
          </div>

          {/* Right Sidebar: Sources */}
          {conv && conv.sources.length > 0 && (
            <SourcePanel sources={conv.sources} onOpenNote={handleOpenNote} />
          )}
        </div>
      </div>
    </div>
  );
}

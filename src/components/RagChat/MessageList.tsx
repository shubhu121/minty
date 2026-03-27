import { useEffect, useRef } from "react";
import { useRagStore } from "../../store/rag";
import StreamingMessage from "./StreamingMessage";

interface MessageListProps {
  onOpenNote?: (id: string) => void;
}

export default function MessageList({ onOpenNote }: MessageListProps) {
  const conversations = useRagStore((state) => state.conversations);
  const activeId = useRagStore((state) => state.activeConversationId);
  const isStreaming = useRagStore((state) => state.streaming);
  
  const bottomRef = useRef<HTMLDivElement>(null);

  const currentConv = activeId ? conversations.get(activeId) : null;
  const messages = currentConv?.messages || [];

  // Scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isStreaming]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center text-[var(--muted-foreground)] opacity-70">
        <span className="text-4xl mb-4">💬</span>
        <h3 className="text-lg font-medium text-[var(--text-color)] mb-2">Ask Your Notes</h3>
        <p className="max-w-md text-sm">
          Type a question below. Smart Notes uses AI locally on your device to read your notes and synthesize an answer, including inline citations.
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-8 space-y-2">
      <div className="max-w-3xl mx-auto">
        {messages.map((msg) => (
          <StreamingMessage
            key={msg.id}
            message={msg}
            onOpenNote={onOpenNote}
          />
        ))}
        {isStreaming && (
          <div className="flex w-full mb-6 justify-start animate-pulse">
            <div className="w-8 h-8 rounded bg-[var(--accent)] text-[var(--text-color)] flex items-center justify-center mr-3 mt-1 shrink-0">
               ~
            </div>
            <div className="h-4 w-1 bg-[var(--primary)] animate-ping" />
          </div>
        )}
        <div ref={bottomRef} className="h-4 w-full" />
      </div>
    </div>
  );
}

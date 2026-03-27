import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useRagStore, SourceInfo, Citation } from "../store/rag";

export type StreamEvent =
  | { type: "token"; data: string }
  | { type: "citation"; data: Citation }
  | { type: "done" }
  | { type: "error"; data: string };

export function useRagStream() {
  const store = useRagStore();

  useEffect(() => {
    // Listen for sources (emitted before generation)
    const unlistenSources = listen<{ conversation_id: string; sources: SourceInfo[] }>(
      "rag_sources",
      (event) => {
        useRagStore.getState().setSources(event.payload.conversation_id, event.payload.sources);
      }
    );

    // Listen for streaming tokens
    const unlistenStream = listen<{ conversation_id: string; event: StreamEvent }>(
      "rag_stream",
      (event) => {
        const payload = event.payload;
        const state = useRagStore.getState();
        
        switch (payload.event.type) {
          case "token":
            state.updateLastMessageText(payload.conversation_id, payload.event.data as string);
            break;
          case "citation":
            state.addCitationToLastMessage(payload.conversation_id, payload.event.data as Citation);
            break;
          case "done":
            state.setStreaming(false);
            break;
          case "error":
            state.updateLastMessageText(payload.conversation_id, `\n\n[Error: ${payload.event.data}]`);
            state.setStreaming(false);
            break;
        }
      }
    );

    return () => {
      unlistenSources.then((f) => f());
      unlistenStream.then((f) => f());
    };
  }, []);

  const ask = async (question: string) => {
    let convId = store.activeConversationId;
    if (!convId) {
      convId = `conv-${Date.now()}`;
      store.createConversation(convId);
    }

    // Add user message
    store.addMessage(convId, {
      id: `msg-${Date.now()}-user`,
      role: "user",
      parts: [{ type: "text", text: question }],
    });

    // Add empty assistant message placeholder
    store.addMessage(convId, {
      id: `msg-${Date.now()}-ast`,
      role: "assistant",
      parts: [],
    });

    store.setStreaming(true);

    try {
      await invoke("ask_notes", {
        question,
        conversationId: convId,
      });
    } catch (err) {
      console.error("[useRagStream] ask_notes error:", err);
      store.updateLastMessageText(convId, `\n\n[Error: ${err}]`);
      store.setStreaming(false);
    }
  };

  return { ask };
}

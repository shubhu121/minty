import { create } from "zustand";

export interface SourceInfo {
  label: string;
  note_id: string;
  note_title: string;
  heading_path: string;
  rrf_score: number;
}

export interface Citation {
  label: string;
  note_id: string;
  note_title: string;
}

export type MessagePart = 
  | { type: "text"; text: string }
  | { type: "citation"; citation: Citation };

export interface Message {
  id: string;
  role: "user" | "assistant";
  parts: MessagePart[];
}

export interface Conversation {
  id: string;
  messages: Message[];
  sources: SourceInfo[];
}

interface RagStore {
  conversations: Map<string, Conversation>;
  activeConversationId: string | null;
  streaming: boolean;
  
  // Actions
  setActiveConversation: (id: string) => void;
  createConversation: (id: string) => void;
  addMessage: (convId: string, msg: Message) => void;
  updateLastMessageText: (convId: string, textChunk: string) => void;
  addCitationToLastMessage: (convId: string, citation: Citation) => void;
  setSources: (convId: string, sources: SourceInfo[]) => void;
  setStreaming: (isStreaming: boolean) => void;
  clearAll: () => void;
}

export const useRagStore = create<RagStore>((set) => ({
  conversations: new Map(),
  activeConversationId: null,
  streaming: false,

  setActiveConversation: (id) => set({ activeConversationId: id }),
  
  createConversation: (id) =>
    set((state) => {
      const newMap = new Map(state.conversations);
      newMap.set(id, { id, messages: [], sources: [] });
      return { conversations: newMap, activeConversationId: id };
    }),

  addMessage: (convId, msg) =>
    set((state) => {
      const newMap = new Map(state.conversations);
      const conv = newMap.get(convId);
      if (conv) {
        newMap.set(convId, { ...conv, messages: [...conv.messages, msg] });
      }
      return { conversations: newMap };
    }),

  updateLastMessageText: (convId, textChunk) =>
    set((state) => {
      const newMap = new Map(state.conversations);
      const conv = newMap.get(convId);
      if (conv && conv.messages.length > 0) {
        const msgs = [...conv.messages];
        const lastMsg = { ...msgs[msgs.length - 1] };
        const parts = [...lastMsg.parts];
        
        if (parts.length > 0 && parts[parts.length - 1].type === "text") {
          const lastPart = parts[parts.length - 1];
          if (lastPart.type === "text") {
             parts[parts.length - 1] = { ...lastPart, text: lastPart.text + textChunk };
          }
        } else {
          parts.push({ type: "text", text: textChunk });
        }
        
        lastMsg.parts = parts;
        msgs[msgs.length - 1] = lastMsg;
        newMap.set(convId, { ...conv, messages: msgs });
      }
      return { conversations: newMap };
    }),

  addCitationToLastMessage: (convId, citation) =>
    set((state) => {
      const newMap = new Map(state.conversations);
      const conv = newMap.get(convId);
      if (conv && conv.messages.length > 0) {
        const msgs = [...conv.messages];
        const lastMsg = { ...msgs[msgs.length - 1] };
        lastMsg.parts = [...lastMsg.parts, { type: "citation", citation }];
        msgs[msgs.length - 1] = lastMsg;
        newMap.set(convId, { ...conv, messages: msgs });
      }
      return { conversations: newMap };
    }),

  setSources: (convId, sources) =>
    set((state) => {
      const newMap = new Map(state.conversations);
      const conv = newMap.get(convId);
      if (conv) {
        newMap.set(convId, { ...conv, sources });
      }
      return { conversations: newMap };
    }),

  setStreaming: (streaming) => set({ streaming }),

  clearAll: () => set({ conversations: new Map(), activeConversationId: null }),
}));

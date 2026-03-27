import { useEffect, useState, useRef, useCallback } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  KEY_TAB_COMMAND,
  COMMAND_PRIORITY_HIGH,
  $getRoot,
} from "lexical";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

interface SuggestionToken {
  token: string;
  request_id: number;
  done: boolean;
}

export default function InlineSuggestionPlugin({ noteId }: { noteId?: string }) {
  const [editor] = useLexicalComposerContext();
  const [suggestion, setSuggestion] = useState("");
  const [coords, setCoords] = useState<{ x: number; y: number } | null>(null);
  
  const activeRequestId = useRef<number>(0);
  const debounceTimer = useRef<NodeJS.Timeout | null>(null);
  const currentPrefix = useRef<string>("");

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    listen<SuggestionToken>("inline_suggestion_token", (event) => {
      const { token, request_id, done } = event.payload;
      
      if (request_id !== activeRequestId.current) {
        return; // Stale request
      }

      if (done) {
        return;
      }

      setSuggestion((prev) => prev + token);
    }).then(_unlisten => {
      unlisten = _unlisten;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const requestSuggestion = useCallback(async (prefix: string) => {
    activeRequestId.current += 1;
    const reqId = activeRequestId.current;
    setSuggestion("");
    
    try {
      await invoke("get_inline_suggestion", {
        noteId: noteId || "scratch",
        prefix,
        suffix: "",
        cursorPos: prefix.length,
        requestId: reqId
      });
    } catch (e) {
      console.error("Failed to request suggestion:", e);
    }
  }, [noteId]);

  useEffect(() => {
    const removeUpdateListener = editor.registerUpdateListener(({ editorState, tags }) => {
      // Don't trigger on internal state updates, only user changes
      if (tags.has('historic') || tags.has('collaboration')) return;

      editorState.read(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
          setSuggestion("");
          setCoords(null);
          activeRequestId.current += 1; // invalidate
          return;
        }

        const anchor = selection.anchor;
        const root = $getRoot();
        const fullText = root.getTextContent();
        
        // Find text before cursor
        const anchorNode = anchor.getNode();
        
        // Get cursor coordinates for overlay
        const domSelection = window.getSelection();
        if (domSelection && domSelection.rangeCount > 0) {
          const range = domSelection.getRangeAt(0);
          const rect = range.getBoundingClientRect();
          if (rect.width === 0 && rect.height === 0 && rect.top === 0) {
            setCoords(null);
          } else {
             // Let's adjust for relative editor container if needed, but absolute fixed to viewport is easier
             setCoords({ x: rect.right, y: rect.top });
          }
        }

        const prefix = fullText; // For RAG context, we can pass full text as prefix if cursor is at the very end
        currentPrefix.current = prefix;
        
        // Clear previous timeouts
        if (debounceTimer.current) clearTimeout(debounceTimer.current);
        setSuggestion("");
        activeRequestId.current += 1; // invalidate previous requests

        // Only suggest if at the end of the text
        if (anchor.offset === anchorNode.getTextContent().length && anchorNode.getNextSibling() === null) {
          debounceTimer.current = setTimeout(() => {
             requestSuggestion(prefix);
          }, 800); // 800ms debounce
        } else {
             setCoords(null);
        }
      });
    });

    return () => {
      removeUpdateListener();
      if (debounceTimer.current) clearTimeout(debounceTimer.current);
    };
  }, [editor, requestSuggestion]);

  // Handle Tab to accept
  useEffect(() => {
    return editor.registerCommand(
      KEY_TAB_COMMAND,
      (event: KeyboardEvent) => {
        if (suggestion.trim().length > 0) {
          event.preventDefault();
          editor.update(() => {
            const selection = $getSelection();
            if ($isRangeSelection(selection)) {
              selection.insertText(suggestion);
            }
          });
          setSuggestion("");
          activeRequestId.current += 1;
          return true; // handled
        }
        return false;
      },
      COMMAND_PRIORITY_HIGH 
    );
  }, [editor, suggestion]);

  if (!suggestion || !coords) return null;

  return (
    <div
      style={{
        position: "fixed",
        top: coords.y,
        left: coords.x,
        zIndex: 50,
        pointerEvents: "none",
        color: "rgba(156, 163, 175, 0.7)",
        whiteSpace: "pre-wrap",
        fontFamily: "inherit",
        fontSize: "inherit",
        lineHeight: "1.6",
      }}
    >
      {suggestion}
    </div>
  );
}

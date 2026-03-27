import React, { useState, useRef, useEffect } from "react";
import MusicPlayer from "./MusicPlayer";

import { SlEarphones } from "react-icons/sl";
import { IoSettingsOutline } from "react-icons/io5";
import { invoke } from "@tauri-apps/api/core";
import { getNextUntitledName } from "../utils";
import SettingsPanel from "./SettingsPanel";
import IndexingIndicator from "./IndexingIndicator";

interface NavbarProps {
  theme: string;
  setTheme: (theme: string) => void;
  onSave: () => void;
  onPrint: () => void;
  currentFileName: string | null;
  isSaved: boolean;
  onRename: (newName: string) => void;
  autoSave: boolean;
  onToggleAutoSave: () => void;
  onOpenSearch: () => void;
  onToggleContext: () => void;
  onOpenAiSettings: () => void;
  onOpenRagChat: () => void;
}

const Navbar: React.FC<NavbarProps> = ({
  theme,
  setTheme,
  onSave,
  onPrint,
  currentFileName,
  isSaved,
  onRename,
  autoSave,
  onToggleAutoSave,
  onOpenSearch,
  onToggleContext,
  onOpenAiSettings,
  onOpenRagChat,
}) => {
  const [showMusicPlayer, setShowMusicPlayer] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [editName, setEditName] = useState(() => {
    return currentFileName || "";
  });

  useEffect(() => {
    if (currentFileName) {
      setEditName(currentFileName);
      return;
    }
    (async () => {
      const files = await invoke<string[]>("list_files");
      const newName = getNextUntitledName(files);
      setEditName(newName);
      onRename(newName.trim());
    })();
  }, [currentFileName]);

  const inputRef = useRef<HTMLInputElement>(null);

  const toggleMusicPlayer = () => setShowMusicPlayer(!showMusicPlayer);

  // Focus input when editing starts
  useEffect(() => {
    if (isEditing && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isEditing]);

  const handleNameSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (editName.trim() && editName !== currentFileName) {
      onRename(editName.trim());
    }
    setIsEditing(false);
  };

  const handleInputBlur = () => {
    if (editName.trim() && editName !== currentFileName) {
      onRename(editName.trim());
    }
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setEditName(currentFileName || "");
      setIsEditing(false);
    }
    if (e.key === "Enter") {
      handleNameSubmit(e);
    }
  };

  return (
    <>
      <nav className="flex justify-between items-center px-8 py-4 select-none relative">
        <div
          onClick={() => setShowSettings(!showSettings)}
          className="cursor-pointer hover:opacity-50 transition-opacity flex items-center gap-2"
        >
          <IoSettingsOutline size={19} />
        </div>


        {/* Center: Status dot + File name input */}
        <div className="flex items-center gap-2 absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-10">
          {/* Blinking / Pulsing Status Dot */}
          <span className="relative flex h-3 w-3 items-center justify-center">
            {!isSaved && (
              <span
                className="absolute inline-flex h-3 w-3 rounded-full !bg-red-400 opacity-60 animate-ping"
                style={{ animationDuration: "1.5s" }}
              ></span>
            )}
            <span
              className={`relative inline-flex rounded-full h-2 w-2 ${isSaved ? "!bg-green-500" : "!bg-red-500"
                } border border-white`}
            ></span>
          </span>

          {/* Input always rendered for fixed position */}
          <form onSubmit={handleNameSubmit} className="flex items-center">
            <input
              ref={inputRef}
              type="text"
              placeholder="None"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              onBlur={handleInputBlur}
              onKeyDown={handleKeyDown}
              onClick={() => setIsEditing(true)}
              className="font-semibold text-[var(--text-color)] bg-transparent outline-none px-1 min-w-[100px] cursor-text"
              spellCheck={false}
            />
          </form>
        </div>

        {/* Right: Actions */}
        <div className="flex items-center gap-6">
          <div className="flex items-center gap-4">
            {/* Search button */}
            <button
              onClick={onOpenSearch}
              className="cursor-pointer hover:opacity-50 transition-opacity flex items-center gap-1.5"
              title="Search notes (Ctrl+K)"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="8"/>
                <path d="m21 21-4.35-4.35"/>
              </svg>
              <kbd className="text-[10px] px-1 py-0.5 rounded" style={{ background: 'var(--muted)', color: 'var(--muted-foreground)', opacity: 0.7 }}>⌘K</kbd>
            </button>

            {/* Backlinks button */}
            <button
              onClick={onToggleContext}
              className="cursor-pointer hover:opacity-50 transition-opacity"
              title="Context & Backlinks"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M9 17H7A5 5 0 017 7h2"/>
                <path d="M15 7h2a5 5 0 010 10h-2"/>
                <line x1="8" y1="12" x2="16" y2="12"/>
              </svg>
            </button>

            <button
              onClick={() => onSave()}
              className="cursor-pointer hover:opacity-50 transition-opacity px-3 py-1 border-[var(--text-color)]"
            >
              Save
            </button>
          </div>

          <IndexingIndicator />

          {/* Ask AI button */}
          <button
            onClick={onOpenRagChat}
            className="cursor-pointer hover:opacity-50 transition-opacity flex items-center gap-1.5 text-xs font-semibold px-2.5 py-1.5 rounded-lg border"
            style={{
              background: 'var(--accent)',
              borderColor: 'var(--border)',
              color: 'var(--text-color)'
            }}
            title="Ask your notes (Ctrl+J)"
          >
            <span>💬</span> Ask
          </button>

          <div className="flex gap-2.5 sm:gap-4 items-center pl-4 border-l" style={{ borderColor: 'var(--border)' }}>
            <button
              onClick={onToggleContext}
              className="p-2 rounded-lg transition-colors cursor-pointer hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--text-color)]"
              title="Context & Backlinks"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71" />
                <path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71" />
              </svg>
            </button>

            {/* AI Settings button */}
            <button
              onClick={onOpenAiSettings}
              className="cursor-pointer hover:opacity-50 transition-opacity text-sm"
              title="AI Settings"
            >
              🤖
            </button>
          </div>


          <div className="relative">
            <button
              onClick={toggleMusicPlayer}
              className={`cursor-pointer transition-all duration-300 p-2 rounded-full ${showMusicPlayer
                ? "bg-[var(--accent-color)] text-white"
                : "hover:bg-[var(--hover-bg)] hover:opacity-70"
                }`}
            >
              <SlEarphones size={17} />
            </button>
          </div>
        </div>
      </nav>

      {/* Music Player Widget */}
      {/* Music Player Widget - Always mounted for background play, controlled via visibility */}
      <div className={showMusicPlayer ? "block" : "hidden"}>
        <MusicPlayer onClose={() => setShowMusicPlayer(false)} />
      </div>

      {showSettings && (
        <SettingsPanel
          theme={theme}
          setTheme={setTheme}
          autosave={autoSave}
          setAutosave={onToggleAutoSave}
          onPrint={onPrint}
          onClose={() => setShowSettings(false)}
        />
      )}

    </>
  );
};

export default Navbar;
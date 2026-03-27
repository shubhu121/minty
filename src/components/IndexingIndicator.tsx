import { useState, useEffect, useRef } from "react";
import { getIndexingStatus, IndexingStatus } from "../lib/tauri";

export default function IndexingIndicator() {
  const [status, setStatus] = useState<IndexingStatus | null>(null);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    // Poll every 5 seconds
    const poll = async () => {
      try {
        const s = await getIndexingStatus();
        setStatus(s);
      } catch (err) {
        console.error("[IndexingIndicator]", err);
      }
    };
    poll();
    intervalRef.current = window.setInterval(poll, 5000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  if (!status) return null;

  const isComplete = status.total > 0 && status.indexed >= status.total;
  const progress =
    status.total > 0 ? Math.round((status.indexed / status.total) * 100) : 0;

  return (
    <div
      className="flex items-center gap-1.5 text-xs select-none"
      style={{ color: "var(--muted-foreground)" }}
      title={`${status.indexed} / ${status.total} notes indexed`}
    >
      {isComplete ? (
        <>
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            style={{ color: "#22c55e" }}
          >
            <circle cx="12" cy="12" r="10" />
            <path d="M9 12l2 2 4-4" />
          </svg>
          <span style={{ opacity: 0.7 }}>Indexed</span>
        </>
      ) : (
        <>
          <div
            className="w-3 h-3 rounded-full border-[1.5px] animate-spin"
            style={{
              borderColor: "var(--muted-foreground)",
              borderTopColor: "transparent",
            }}
          />
          <span>{progress}%</span>
        </>
      )}
    </div>
  );
}

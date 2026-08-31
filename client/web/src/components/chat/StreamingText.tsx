// StreamingText — typewriter rendering of a single text string.
// EP-0026-rev-fix: no per-char delay; the worker reveals the full
// text on the next animation frame after each update.

import { useEffect, useRef, useState, useTransition, memo } from "react";

export const StreamingText = memo(function StreamingText({ text }: { text: string }) {
  const [, startTransition] = useTransition();
  const [revealedLen, setRevealedLen] = useState(0);
  const workerRef = useRef<Worker | null>(null);

  useEffect(() => {
    const worker = new Worker(
      new URL("../typewriter.worker.ts", import.meta.url),
      { type: "module" },
    );
    worker.onmessage = (e: MessageEvent) => {
      if (e.data?.type === "reveal") {
        startTransition(() => setRevealedLen(e.data.revealed));
      }
    };
    workerRef.current = worker;
    return () => {
      worker.terminate();
      workerRef.current = null;
    };
  }, [startTransition]);

  useEffect(() => {
    workerRef.current?.postMessage({ type: "update", text });
  }, [text]);

  return (
    <div className="chat__streaming-text">
      {text.slice(0, revealedLen)}
    </div>
  );
});

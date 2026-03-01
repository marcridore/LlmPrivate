import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function TitleBar() {
  const appWindow = getCurrentWindow();
  const [isFullscreen, setIsFullscreen] = useState(false);

  useEffect(() => {
    // Sync fullscreen state on mount
    appWindow.isFullscreen().then(setIsFullscreen);

    // Listen for F11 to toggle fullscreen
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "F11") {
        e.preventDefault();
        appWindow.isFullscreen().then((fs) => {
          appWindow.setFullscreen(!fs);
          setIsFullscreen(!fs);
        });
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [appWindow]);

  const toggleFullscreen = async () => {
    const fs = await appWindow.isFullscreen();
    await appWindow.setFullscreen(!fs);
    setIsFullscreen(!fs);
  };

  // Hide title bar in fullscreen — show a thin hover strip at top to exit
  if (isFullscreen) {
    return (
      <div
        className="h-1 hover:h-8 group bg-transparent hover:bg-background hover:border-b hover:border-border transition-all duration-200 select-none flex items-center justify-end"
      >
        <div className="hidden group-hover:flex">
          <button
            onClick={toggleFullscreen}
            title="Exit Fullscreen (F11)"
            className="h-8 w-12 flex items-center justify-center hover:bg-muted transition-colors"
          >
            <svg width="12" height="12" viewBox="0 0 12 12" className="stroke-foreground fill-none" strokeWidth="1.2">
              <polyline points="4,1 1,1 1,4" />
              <polyline points="8,1 11,1 11,4" />
              <polyline points="4,11 1,11 1,8" />
              <polyline points="8,11 11,11 11,8" />
            </svg>
          </button>
          <button
            onClick={() => appWindow.close()}
            className="h-8 w-12 flex items-center justify-center hover:bg-destructive hover:text-destructive-foreground transition-colors"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" className="stroke-current">
              <line x1="0" y1="0" x2="10" y2="10" strokeWidth="1" />
              <line x1="10" y1="0" x2="0" y2="10" strokeWidth="1" />
            </svg>
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      data-tauri-drag-region
      className="h-8 flex items-center justify-between bg-background border-b border-border select-none"
    >
      <div data-tauri-drag-region className="flex items-center gap-2 px-3">
        <span className="text-xs font-semibold text-muted-foreground">
          LlmPrivate
        </span>
      </div>

      <div className="flex">
        <button
          onClick={() => appWindow.minimize()}
          className="h-8 w-12 flex items-center justify-center hover:bg-muted transition-colors"
        >
          <svg width="10" height="1" viewBox="0 0 10 1" className="fill-foreground">
            <rect width="10" height="1" />
          </svg>
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="h-8 w-12 flex items-center justify-center hover:bg-muted transition-colors"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" className="fill-none stroke-foreground">
            <rect x="0.5" y="0.5" width="9" height="9" strokeWidth="1" />
          </svg>
        </button>
        <button
          onClick={toggleFullscreen}
          title="Fullscreen (F11)"
          className="h-8 w-12 flex items-center justify-center hover:bg-muted transition-colors"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" className="stroke-foreground fill-none" strokeWidth="1.2">
            <polyline points="1,4 1,1 4,1" />
            <polyline points="11,4 11,1 8,1" />
            <polyline points="1,8 1,11 4,11" />
            <polyline points="11,8 11,11 8,11" />
          </svg>
        </button>
        <button
          onClick={() => appWindow.close()}
          className="h-8 w-12 flex items-center justify-center hover:bg-destructive hover:text-destructive-foreground transition-colors"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" className="stroke-current">
            <line x1="0" y1="0" x2="10" y2="10" strokeWidth="1" />
            <line x1="10" y1="0" x2="0" y2="10" strokeWidth="1" />
          </svg>
        </button>
      </div>
    </div>
  );
}

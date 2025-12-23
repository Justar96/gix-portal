import { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";

const appWindow = getCurrentWindow();

export function Titlebar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const checkMaximized = async () => {
      setIsMaximized(await appWindow.isMaximized());
    };
    
    checkMaximized();
    
    const unlisten = appWindow.onResized(async () => {
      setIsMaximized(await appWindow.isMaximized());
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleDrag = useCallback((e: React.MouseEvent) => {
    // Prevent dragging when clicking on buttons
    if ((e.target as HTMLElement).closest('button')) return;
    appWindow.startDragging();
  }, []);

  const handleMinimize = () => appWindow.minimize();
  const handleMaximize = () => appWindow.toggleMaximize();
  const handleClose = () => appWindow.close();

  return (
    <div className="titlebar" onMouseDown={handleDrag}>
      <div className="titlebar-title">
        Gix
      </div>
      <div className="titlebar-controls">
        <button
          className="titlebar-btn titlebar-minimize"
          onClick={handleMinimize}
          aria-label="Minimize"
        >
          <Minus size={12} strokeWidth={2} />
        </button>
        <button
          className="titlebar-btn titlebar-maximize"
          onClick={handleMaximize}
          aria-label={isMaximized ? "Restore" : "Maximize"}
        >
          {isMaximized ? <Copy size={10} strokeWidth={2} /> : <Square size={10} strokeWidth={2} />}
        </button>
        <button
          className="titlebar-btn titlebar-close"
          onClick={handleClose}
          aria-label="Close"
        >
          <X size={14} strokeWidth={2} />
        </button>
      </div>
    </div>
  );
}

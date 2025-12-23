import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HardDrive, Pencil, Trash2 } from "lucide-react";
import type { DriveInfo } from "../types";
import { formatBytes } from "../types";

interface DriveListProps {
  drives: DriveInfo[];
  onSelect: (drive: DriveInfo) => void;
  onUpdate: () => void;
  selectedId: string | null;
}

interface ContextMenuState {
  x: number;
  y: number;
  drive: DriveInfo;
}

export function DriveList({
  drives,
  onSelect,
  onUpdate,
  selectedId,
}: DriveListProps) {
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameName, setRenameName] = useState("");

  const handleContextMenu = (e: React.MouseEvent, drive: DriveInfo) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, drive });
  };

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  const handleDelete = async (drive: DriveInfo) => {
    closeContextMenu();
    if (!confirm(`Delete drive "${drive.name}"? This will not delete your files.`)) {
      return;
    }

    try {
      await invoke("delete_drive", { driveId: drive.id });
      onUpdate();
    } catch (e) {
      console.error("Failed to delete drive:", e);
      alert(`Failed to delete drive: ${e}`);
    }
  };

  const startRename = (drive: DriveInfo) => {
    closeContextMenu();
    setRenameId(drive.id);
    setRenameName(drive.name);
  };

  const handleRename = async (driveId: string) => {
    if (!renameName.trim()) {
      setRenameId(null);
      return;
    }

    try {
      await invoke("rename_drive", { driveId, newName: renameName });
      onUpdate();
    } catch (e) {
      console.error("Failed to rename drive:", e);
      alert(`Failed to rename drive: ${e}`);
    } finally {
      setRenameId(null);
    }
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent, driveId: string) => {
    if (e.key === "Enter") {
      handleRename(driveId);
    } else if (e.key === "Escape") {
      setRenameId(null);
    }
  };

  if (drives.length === 0) {
    return (
      <div className="drive-list empty">
        <p>No drives yet</p>
        <p className="hint">Create one to get started</p>
      </div>
    );
  }

  return (
    <>
      <div className="drive-list" onClick={closeContextMenu}>
        {drives.map((drive) => (
          <div
            key={drive.id}
            className={`drive-item ${selectedId === drive.id ? "selected" : ""}`}
            onClick={() => onSelect(drive)}
            onContextMenu={(e) => handleContextMenu(e, drive)}
          >
            <div className="drive-icon">
              <HardDrive size={16} />
            </div>
            <div className="drive-info">
              {renameId === drive.id ? (
                <input
                  className="rename-input"
                  value={renameName}
                  onChange={(e) => setRenameName(e.target.value)}
                  onBlur={() => handleRename(drive.id)}
                  onKeyDown={(e) => handleRenameKeyDown(e, drive.id)}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <div className="drive-name">{drive.name}</div>
              )}
              <div className="drive-stats">
                {drive.file_count} files &middot; {formatBytes(drive.total_size)}
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div className="context-overlay" onClick={closeContextMenu} />
          <div
            className="context-menu"
            style={{ top: contextMenu.y, left: contextMenu.x }}
          >
            <button onClick={() => startRename(contextMenu.drive)}>
              <Pencil size={14} />
              Rename
            </button>
            <button
              className="danger"
              onClick={() => handleDelete(contextMenu.drive)}
            >
              <Trash2 size={14} />
              Delete
            </button>
          </div>
        </>
      )}
    </>
  );
}

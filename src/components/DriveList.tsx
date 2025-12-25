import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HardDrive, Pencil, Trash2, Share2, FolderSync, MoreVertical } from "lucide-react";
import type { DriveInfo } from "../types";
import { formatBytes } from "../types";
import { ShareDriveModal } from "./ShareDriveModal";
import { ConfirmDialog } from "./ConfirmDialog";
import { useToast } from "./Toast";

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

interface DeleteConfirmState {
  isOpen: boolean;
  drive: DriveInfo | null;
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
  const [shareDrive, setShareDrive] = useState<DriveInfo | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState>({ isOpen: false, drive: null });
  const [isDeleting, setIsDeleting] = useState(false);

  // Toast notifications
  let toast: ReturnType<typeof useToast> | null = null;
  try {
    toast = useToast();
  } catch {
    // Toast provider not available
  }

  const handleContextMenu = (e: React.MouseEvent, drive: DriveInfo) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, drive });
  };

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  const handleDeleteRequest = (drive: DriveInfo) => {
    closeContextMenu();
    setDeleteConfirm({ isOpen: true, drive });
  };

  const handleDeleteConfirm = async () => {
    const driveToDelete = deleteConfirm.drive;
    if (!driveToDelete) return;

    setIsDeleting(true);
    try {
      await invoke("delete_drive", { driveId: driveToDelete.id });
      setDeleteConfirm({ isOpen: false, drive: null });
      toast?.showSuccess(`Drive "${driveToDelete.name}" removed`);
      onUpdate();
    } catch (e) {
      console.error("Failed to delete drive:", e);
      toast?.showError(`Failed to delete drive: ${e}`);
    } finally {
      setIsDeleting(false);
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
        <div className="empty-icon">
          <FolderSync size={32} />
        </div>
        <p className="empty-title">No drives yet</p>
        <p className="hint">Create your first drive to start syncing</p>
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
              <HardDrive size={18} />
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
                <>
                  <span className="drive-name">{drive.name}</span>
                  <div className="drive-meta">
                    <span>• {drive.file_count} files</span>
                    <span>• {formatBytes(drive.total_size)}</span>
                  </div>
                </>
              )}
            </div>
            <button
              className="drive-menu-btn"
              onClick={(e) => {
                e.stopPropagation();
                handleContextMenu(e, drive);
              }}
              title="More options"
            >
              <MoreVertical size={14} />
            </button>
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
            <button
              onClick={() => {
                setShareDrive(contextMenu.drive);
                closeContextMenu();
              }}
            >
              <Share2 size={14} />
              Share
            </button>
            <button onClick={() => startRename(contextMenu.drive)}>
              <Pencil size={14} />
              Rename
            </button>
            <button
              className="danger"
              onClick={() => handleDeleteRequest(contextMenu.drive)}
            >
              <Trash2 size={14} />
              Delete
            </button>
          </div>
        </>
      )}

      {/* Share Modal */}
      {shareDrive && (
        <ShareDriveModal
          drive={shareDrive}
          onClose={() => setShareDrive(null)}
        />
      )}

      {/* Delete Confirmation Dialog */}
      <ConfirmDialog
        isOpen={deleteConfirm.isOpen}
        title={`Delete "${deleteConfirm.drive?.name}"?`}
        message="This will remove the drive from Gix. Your local files will not be deleted."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        onConfirm={handleDeleteConfirm}
        onCancel={() => setDeleteConfirm({ isOpen: false, drive: null })}
        isLoading={isDeleting}
      />
    </>
  );
}


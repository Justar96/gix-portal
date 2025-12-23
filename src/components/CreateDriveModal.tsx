import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { X, FolderOpen, Loader2 } from "lucide-react";
import type { DriveInfo } from "../types";

interface CreateDriveModalProps {
  onClose: () => void;
  onCreated: (drive: DriveInfo) => void;
}

export function CreateDriveModal({ onClose, onCreated }: CreateDriveModalProps) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select folder to share",
      });
      if (selected && typeof selected === "string") {
        setPath(selected);
        // Auto-fill name from folder name if empty
        if (!name) {
          const folderName = selected.split(/[/\\]/).pop() || "New Drive";
          setName(folderName);
        }
      }
    } catch (e) {
      console.error("Failed to open folder dialog:", e);
    }
  };

  const handleCreate = async () => {
    if (!name.trim()) {
      setError("Please provide a name");
      return;
    }
    if (!path.trim()) {
      setError("Please select a folder");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const drive = await invoke<DriveInfo>("create_drive", {
        name: name.trim(),
        path: path.trim(),
      });
      onCreated(drive);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !loading) {
      handleCreate();
    }
    if (e.key === "Escape") {
      onClose();
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="modal-header">
          <h2>Create Drive</h2>
          <button className="btn-close" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label htmlFor="drive-name">Name</label>
            <input
              id="drive-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Shared Drive"
              autoFocus
            />
          </div>

          <div className="form-group">
            <label htmlFor="drive-path">Folder</label>
            <div className="path-input">
              <input
                id="drive-path"
                type="text"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="Select a folder..."
                readOnly
              />
              <button onClick={selectFolder} disabled={loading}>
                <FolderOpen size={16} />
                Browse
              </button>
            </div>
          </div>

          {error && <div className="error-message">{error}</div>}
        </div>

        <div className="modal-footer">
          <button className="btn-secondary" onClick={onClose} disabled={loading}>
            Cancel
          </button>
          <button
            className="btn-primary"
            onClick={handleCreate}
            disabled={loading || !name.trim() || !path.trim()}
          >
            {loading ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                Creating...
              </>
            ) : (
              "Create Drive"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

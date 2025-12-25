import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { X, FolderOpen, Loader2, FolderPlus, Check, AlertCircle } from "lucide-react";
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
        setError(null);
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
    if (e.key === "Enter" && !loading && name.trim() && path.trim()) {
      handleCreate();
    }
    if (e.key === "Escape") {
      onClose();
    }
  };

  const isValid = name.trim() && path.trim();
  const folderName = path ? path.split(/[/\\]/).pop() : null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal create-drive-modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <button className="modal-close" onClick={onClose}>
          <X size={16} />
        </button>

        <div className="modal-header centered">
          <div className="modal-icon">
            <FolderPlus size={24} strokeWidth={1.5} />
          </div>
          <h2>Create Drive</h2>
          <p className="modal-subtitle">Select a folder to share peer-to-peer</p>
        </div>

        <div className="modal-body">
          {/* Folder Selection - Primary Action */}
          <button
            type="button"
            className={`folder-select-btn ${path ? "selected" : ""}`}
            onClick={selectFolder}
            disabled={loading}
          >
            <div className="folder-select-icon">
              {path ? <Check size={20} strokeWidth={2} /> : <FolderOpen size={20} />}
            </div>
            <div className="folder-select-content">
              {path ? (
                <>
                  <span className="folder-name">{folderName}</span>
                  <span className="folder-path">{path}</span>
                </>
              ) : (
                <>
                  <span className="folder-name">Select Folder</span>
                  <span className="folder-path">Choose a folder from your computer</span>
                </>
              )}
            </div>
          </button>

          {/* Drive Name Input */}
          <div className="form-group compact">
            <label htmlFor="drive-name">Drive Name</label>
            <input
              id="drive-name"
              type="text"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setError(null);
              }}
              placeholder="Enter a name for this drive"
              disabled={loading}
            />
          </div>

          {/* Error Display */}
          {error && (
            <div className="form-error">
              <AlertCircle size={14} />
              <span>{error}</span>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn-secondary" onClick={onClose} disabled={loading}>
            Cancel
          </button>
          <button
            className="btn-primary"
            onClick={handleCreate}
            disabled={loading || !isValid}
          >
            {loading ? (
              <>
                <Loader2 size={16} className="spinning" />
                Creating...
              </>
            ) : (
              <>
                <FolderPlus size={16} />
                Create Drive
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

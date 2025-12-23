import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronUp,
  RefreshCw,
  Folder,
  FolderOpen,
  File,
  FileText,
  Image,
  Film,
  Music,
  Code,
  Archive,
  Database,
} from "lucide-react";
import type { FileEntry, DriveInfo, FileCategory } from "../types";
import { formatBytes, formatDate, getFileCategory } from "../types";

interface FileBrowserProps {
  drive: DriveInfo;
}

/** Get Lucide icon component for file category */
function getFileIconComponent(entry: FileEntry) {
  if (entry.is_dir) {
    return <Folder size={16} />;
  }

  const category = getFileCategory(entry.name);
  const iconMap: Record<FileCategory, React.ReactNode> = {
    folder: <Folder size={16} />,
    document: <FileText size={16} />,
    image: <Image size={16} />,
    video: <Film size={16} />,
    audio: <Music size={16} />,
    code: <Code size={16} />,
    archive: <Archive size={16} />,
    data: <Database size={16} />,
    unknown: <File size={16} />,
  };

  return iconMap[category];
}

export function FileBrowser({ drive }: FileBrowserProps) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [currentPath, setCurrentPath] = useState("/");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);

  const loadFiles = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      setSelectedIndex(-1);
      try {
        const entries = await invoke<FileEntry[]>("list_files", {
          driveId: drive.id,
          path,
        });
        setFiles(entries);
        setCurrentPath(path);
      } catch (e) {
        console.error("Failed to list files:", e);
        setError(String(e));
        setFiles([]);
      } finally {
        setLoading(false);
      }
    },
    [drive.id]
  );

  useEffect(() => {
    loadFiles("/");
  }, [loadFiles]);

  const navigateTo = (entry: FileEntry) => {
    if (entry.is_dir) {
      loadFiles(entry.path);
    }
  };

  const navigateUp = () => {
    if (currentPath === "/" || currentPath === "") return;
    const parts = currentPath.split(/[/\\]/).filter(Boolean);
    parts.pop();
    const parent = parts.length > 0 ? parts.join("/") : "/";
    loadFiles(parent);
  };

  const getBreadcrumbs = () => {
    const parts = currentPath.split(/[/\\]/).filter(Boolean);
    const crumbs = [{ name: drive.name, path: "/" }];
    let accumulated = "";
    for (const part of parts) {
      accumulated += "/" + part;
      crumbs.push({ name: part, path: accumulated });
    }
    return crumbs;
  };

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (files.length === 0) return;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, files.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(prev - 1, 0));
        break;
      case "Enter":
        if (selectedIndex >= 0 && selectedIndex < files.length) {
          navigateTo(files[selectedIndex]);
        }
        break;
      case "Backspace":
        navigateUp();
        break;
    }
  };

  return (
    <div className="file-browser" tabIndex={0} onKeyDown={handleKeyDown}>
      <div className="browser-header">
        <button
          className="btn-icon"
          onClick={navigateUp}
          disabled={currentPath === "/" || currentPath === "" || loading}
          title="Go up"
        >
          <ChevronUp size={16} />
        </button>
        <div className="breadcrumbs">
          {getBreadcrumbs().map((crumb, i) => (
            <span key={crumb.path}>
              {i > 0 && <span className="separator">/</span>}
              <button
                className="breadcrumb"
                onClick={() => loadFiles(crumb.path)}
                disabled={loading}
              >
                {crumb.name}
              </button>
            </span>
          ))}
        </div>
        <button
          className="btn-icon"
          onClick={() => loadFiles(currentPath)}
          disabled={loading}
          title="Refresh"
        >
          <RefreshCw size={16} />
        </button>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {loading ? (
        <div className="loading-state">
          <div className="loading-spinner" />
          <span>Loading files...</span>
        </div>
      ) : files.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">
            <FolderOpen size={32} />
          </div>
          <p>This folder is empty</p>
        </div>
      ) : (
        <div className="file-table-container">
          <table className="file-table">
            <thead>
              <tr>
                <th className="col-name">Name</th>
                <th className="col-size">Size</th>
                <th className="col-modified">Modified</th>
              </tr>
            </thead>
            <tbody>
              {files.map((file, index) => (
                <tr
                  key={file.path}
                  className={`${file.is_dir ? "directory" : "file"} ${index === selectedIndex ? "selected" : ""
                    }`}
                  onClick={() => setSelectedIndex(index)}
                  onDoubleClick={() => navigateTo(file)}
                >
                  <td className="col-name">
                    <div className="file-cell">
                      <span className="file-icon">{getFileIconComponent(file)}</span>
                      <span className="file-name">{file.name}</span>
                    </div>
                  </td>
                  <td className="col-size">
                    {file.is_dir ? "-" : formatBytes(file.size)}
                  </td>
                  <td className="col-modified">{formatDate(file.modified_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

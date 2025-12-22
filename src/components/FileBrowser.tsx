import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified_at: string;
}

interface DriveInfo {
  id: string;
  name: string;
  local_path: string;
}

interface FileBrowserProps {
  drive: DriveInfo;
}

export function FileBrowser({ drive }: FileBrowserProps) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [currentPath, setCurrentPath] = useState("/");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadFiles = async (path: string) => {
    setLoading(true);
    setError(null);
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
  };

  useEffect(() => {
    loadFiles("/");
  }, [drive.id]);

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

  const formatSize = (bytes: number): string => {
    if (bytes === 0) return "-";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return "-";
    }
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

  return (
    <div className="file-browser">
      <div className="browser-header">
        <button
          className="btn-icon"
          onClick={navigateUp}
          disabled={currentPath === "/" || currentPath === "" || loading}
          title="Go up"
        >
          ↑
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
          ↻
        </button>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {loading ? (
        <div className="loading-state">Loading files...</div>
      ) : files.length === 0 ? (
        <div className="empty-state">
          <p>This folder is empty</p>
        </div>
      ) : (
        <table className="file-table">
          <thead>
            <tr>
              <th className="col-name">Name</th>
              <th className="col-size">Size</th>
              <th className="col-modified">Modified</th>
            </tr>
          </thead>
          <tbody>
            {files.map((file) => (
              <tr
                key={file.path}
                className={file.is_dir ? "directory" : "file"}
                onDoubleClick={() => navigateTo(file)}
              >
                <td className="col-name">
                  <span className="file-icon">{file.is_dir ? "📁" : "📄"}</span>
                  <span className="file-name">{file.name}</span>
                </td>
                <td className="col-size">
                  {file.is_dir ? "-" : formatSize(file.size)}
                </td>
                <td className="col-modified">{formatDate(file.modified_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

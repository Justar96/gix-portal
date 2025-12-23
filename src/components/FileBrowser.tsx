import { useEffect, useState, useCallback, useTransition, useDeferredValue, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
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
  Lock,
  LockOpen,
  Unlock,
} from "lucide-react";
import type { FileEntry, DriveInfo, FileCategory } from "../types";
import { formatBytes, formatDate, getFileCategory, shortNodeId, formatLockExpiry } from "../types";
import { useLocking } from "../hooks";

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
  const [showLockMenu, setShowLockMenu] = useState<string | null>(null);

  // React 18 concurrent features for smoother UI
  const [isPending, startTransition] = useTransition();
  const deferredFiles = useDeferredValue(files);
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Virtual scrolling for large file lists (1000+ files)
  const rowVirtualizer = useVirtualizer({
    count: deferredFiles.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => 44, // Estimated row height in pixels
    overscan: 10, // Render 10 extra rows above/below viewport
  });

  // File locking
  const {
    getLockStatus,
    isLockedByOther,
    isLockedByMe,
    acquireLock,
    releaseLock,
  } = useLocking({ driveId: drive.id });

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
        // Use startTransition to mark state updates as non-urgent
        // This keeps the UI responsive during large file list updates
        startTransition(() => {
          setFiles(entries);
          setCurrentPath(path);
        });
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

  // Show stale indicator when deferred value differs from current
  const isStale = deferredFiles !== files;

  return (
    <div className="file-browser" tabIndex={0} onKeyDown={handleKeyDown} ref={containerRef}>
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
          {/* Show pending indicator during transitions */}
          {isPending && <span className="pending-indicator">...</span>}
        </div>
        <button
          className="btn-icon"
          onClick={() => loadFiles(currentPath)}
          disabled={loading}
          title="Refresh"
        >
          <RefreshCw size={16} className={isPending ? "spinning" : ""} />
        </button>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {loading ? (
        <div className="loading-state">
          <div className="loading-spinner" />
          <span>Loading files...</span>
        </div>
      ) : deferredFiles.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">
            <FolderOpen size={32} />
          </div>
          <p>This folder is empty</p>
        </div>
      ) : (
        <div className={`file-table-container ${isStale ? "stale" : ""}`}>
          {/* Table header */}
          <div className="file-table-header">
            <div className="col-name">Name</div>
            <div className="col-size">Size</div>
            <div className="col-modified">Modified</div>
            <div className="col-actions"></div>
          </div>

          {/* Virtualized scroll container */}
          <div
            ref={scrollContainerRef}
            className="file-table-body"
          >
            {/* Total height container for proper scrollbar */}
            <div
              style={{
                height: `${rowVirtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {/* Only render visible rows */}
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const index = virtualRow.index;
                const file = deferredFiles[index];
                const lock = getLockStatus(file.path);
                const lockedByOther = isLockedByOther(file.path);
                const lockedByMe = isLockedByMe(file.path);

                return (
                  <div
                    key={file.path}
                    className={`file-row ${file.is_dir ? "directory" : "file"} ${index === selectedIndex ? "selected" : ""} ${lockedByOther ? "locked-other" : ""} ${lockedByMe ? "locked-mine" : ""}`}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: `${virtualRow.size}px`,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                    onClick={() => setSelectedIndex(index)}
                    onDoubleClick={() => navigateTo(file)}
                  >
                    <div className="col-name">
                      <div className="file-cell">
                        <span className="file-icon">{getFileIconComponent(file)}</span>
                        <span className="file-name">{file.name}</span>
                        {lock && !file.is_dir && (
                          <span
                            className={`lock-indicator ${lockedByMe ? "mine" : "other"}`}
                            title={
                              lockedByMe
                                ? `Locked by you (${formatLockExpiry(lock.expires_at)})`
                                : `Locked by ${shortNodeId(lock.holder)} (${lock.lock_type})`
                            }
                          >
                            {lockedByMe ? (
                              <LockOpen size={12} className="lock-icon mine" />
                            ) : (
                              <Lock size={12} className="lock-icon other" />
                            )}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="col-size">
                      {file.is_dir ? "-" : formatBytes(file.size)}
                    </div>
                    <div className="col-modified">{formatDate(file.modified_at)}</div>
                    <div className="col-actions">
                      {!file.is_dir && (
                        <div className="action-buttons">
                          {lockedByMe ? (
                            <button
                              className="btn-icon btn-unlock"
                              onClick={(e) => {
                                e.stopPropagation();
                                releaseLock(file.path);
                              }}
                              title="Release lock"
                            >
                              <Unlock size={14} />
                            </button>
                          ) : !lockedByOther ? (
                            <button
                              className="btn-icon btn-lock"
                              onClick={(e) => {
                                e.stopPropagation();
                                setShowLockMenu(showLockMenu === file.path ? null : file.path);
                              }}
                              title="Lock file"
                            >
                              <Lock size={14} />
                            </button>
                          ) : null}
                          {showLockMenu === file.path && (
                            <div className="lock-menu">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  acquireLock(file.path, "advisory");
                                  setShowLockMenu(null);
                                }}
                              >
                                Advisory Lock
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  acquireLock(file.path, "exclusive");
                                  setShowLockMenu(null);
                                }}
                              >
                                Exclusive Lock
                              </button>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

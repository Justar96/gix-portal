import { useEffect, useState, useCallback, useTransition, useMemo, useRef, memo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronUp,
  ChevronDown,
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
  Download,
  Trash2,
  Pencil,
  Copy,
  MoreHorizontal,
  Search,
  X,
  Grid,
  List,
} from "lucide-react";
import type { FileEntry, DriveInfo, FileCategory } from "../types";
import { formatBytes, formatDate, getFileCategory, shortNodeId, formatLockExpiry } from "../types";
import { useLocking } from "../hooks";

interface FileBrowserProps {
  drive: DriveInfo;
}

type SortField = "name" | "size" | "modified";
type SortDirection = "asc" | "desc";
type ViewMode = "list" | "grid";

interface ContextMenuState {
  x: number;
  y: number;
  file: FileEntry;
}

// Menu dimensions for viewport boundary calculations
const CONTEXT_MENU_WIDTH = 200;
const CONTEXT_MENU_HEIGHT = 280;

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

// Memoized file row component to prevent unnecessary re-renders
interface FileRowProps {
  file: FileEntry;
  index: number;
  isSelected: boolean;
  isRenaming: boolean;
  renameName: string;
  lock: ReturnType<ReturnType<typeof useLocking>["getLockStatus"]>;
  lockedByOther: boolean;
  lockedByMe: boolean;
  virtualRow: { size: number; start: number };
  onRowClick: (e: React.MouseEvent, index: number) => void;
  onDoubleClick: (file: FileEntry) => void;
  onContextMenu: (e: React.MouseEvent, file: FileEntry, index: number) => void;
  onRenameChange: (value: string) => void;
  onRenameSubmit: (file: FileEntry) => void;
  onRenameCancel: () => void;
  renameInputRef: React.RefObject<HTMLInputElement>;
}

const FileRow = memo(function FileRow({
  file,
  index,
  isSelected,
  isRenaming,
  renameName,
  lock,
  lockedByOther,
  lockedByMe,
  virtualRow,
  onRowClick,
  onDoubleClick,
  onContextMenu,
  onRenameChange,
  onRenameSubmit,
  onRenameCancel,
  renameInputRef,
}: FileRowProps) {
  return (
    <div
      className={`file-row ${file.is_dir ? "directory" : "file"} ${isSelected ? "selected" : ""} ${lockedByOther ? "locked-other" : ""} ${lockedByMe ? "locked-mine" : ""}`}
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: "100%",
        height: `${virtualRow.size}px`,
        transform: `translateY(${virtualRow.start}px)`,
      }}
      onClick={(e) => onRowClick(e, index)}
      onDoubleClick={() => onDoubleClick(file)}
      onContextMenu={(e) => onContextMenu(e, file, index)}
    >
      <div className="col-checkbox">
        <input
          type="checkbox"
          checked={isSelected}
          onChange={() => {}}
          onClick={(e) => e.stopPropagation()}
        />
      </div>
      <div className="col-name">
        <div className="file-cell">
          <span className="file-icon">{getFileIconComponent(file)}</span>
          {isRenaming ? (
            <input
              ref={renameInputRef}
              className="rename-input"
              value={renameName}
              onChange={(e) => onRenameChange(e.target.value)}
              onBlur={() => onRenameSubmit(file)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onRenameSubmit(file);
                if (e.key === "Escape") onRenameCancel();
                e.stopPropagation();
              }}
              onClick={(e) => e.stopPropagation()}
            />
          ) : (
            <span className="file-name">{file.name}</span>
          )}
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
        <button
          className="btn-icon btn-more"
          onClick={(e) => {
            e.stopPropagation();
            onContextMenu(e, file, index);
          }}
          title="More actions"
        >
          <MoreHorizontal size={14} />
        </button>
      </div>
    </div>
  );
});

export function FileBrowser({ drive }: FileBrowserProps) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [currentPath, setCurrentPath] = useState("/");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedIndices, setSelectedIndices] = useState<Set<number>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number>(-1);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renameFile, setRenameFile] = useState<string | null>(null);
  const [renameName, setRenameName] = useState("");

  // Search and sort state
  const [searchQuery, setSearchQuery] = useState("");
  const [sortField, setSortField] = useState<SortField>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const [viewMode, setViewMode] = useState<ViewMode>("list");

  // React 18 concurrent features for smoother UI
  const [isPending, startTransition] = useTransition();
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);

  // File locking
  const {
    getLockStatus,
    isLockedByOther,
    isLockedByMe,
    acquireLock,
    releaseLock,
  } = useLocking({ driveId: drive.id });

  // Memoized filter and sort - only recalculate when dependencies change
  const displayFiles = useMemo(() => {
    let result = [...files];

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter((f) => f.name.toLowerCase().includes(query));
    }

    // Sort files (directories first, then by field)
    result.sort((a, b) => {
      // Directories always come first
      if (a.is_dir && !b.is_dir) return -1;
      if (!a.is_dir && b.is_dir) return 1;

      let cmp = 0;
      switch (sortField) {
        case "name":
          cmp = a.name.localeCompare(b.name);
          break;
        case "size":
          cmp = a.size - b.size;
          break;
        case "modified":
          cmp = new Date(a.modified_at).getTime() - new Date(b.modified_at).getTime();
          break;
      }
      return sortDirection === "asc" ? cmp : -cmp;
    });

    return result;
  }, [files, searchQuery, sortField, sortDirection]);

  // Virtualizer for list view
  const rowVirtualizer = useVirtualizer({
    count: displayFiles.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => viewMode === "grid" ? 100 : 36,
    overscan: 10,
  });

  const loadFiles = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      setSelectedIndices(new Set());
      setLastSelectedIndex(-1);
      setSearchQuery("");
      setContextMenu(null);
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

  // Multi-select click handler
  const handleRowClick = (e: React.MouseEvent, index: number) => {
    if (e.ctrlKey || e.metaKey) {
      // Ctrl+click: Toggle selection
      setSelectedIndices((prev) => {
        const next = new Set(prev);
        if (next.has(index)) {
          next.delete(index);
        } else {
          next.add(index);
        }
        return next;
      });
      setLastSelectedIndex(index);
    } else if (e.shiftKey && lastSelectedIndex >= 0) {
      // Shift+click: Range selection
      const start = Math.min(lastSelectedIndex, index);
      const end = Math.max(lastSelectedIndex, index);
      const range = new Set<number>();
      for (let i = start; i <= end; i++) {
        range.add(i);
      }
      setSelectedIndices(range);
    } else {
      // Normal click: Toggle if already selected alone, otherwise select
      if (selectedIndices.has(index) && selectedIndices.size === 1) {
        // Clicking on the only selected item - deselect it
        setSelectedIndices(new Set());
        setLastSelectedIndex(-1);
      } else {
        // Select this item only
        setSelectedIndices(new Set([index]));
        setLastSelectedIndex(index);
      }
    }
  };

  // Context menu handler with viewport boundary detection
  const handleContextMenu = (e: React.MouseEvent, file: FileEntry, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    
    // If right-clicked file is not in selection, select only it
    if (!selectedIndices.has(index)) {
      setSelectedIndices(new Set([index]));
      setLastSelectedIndex(index);
    }
    
    // Calculate position with viewport boundary checks
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    
    let x = e.clientX;
    let y = e.clientY;
    
    // Adjust X if menu would overflow right edge
    if (x + CONTEXT_MENU_WIDTH > viewportWidth) {
      x = viewportWidth - CONTEXT_MENU_WIDTH - 8;
    }
    
    // Adjust Y if menu would overflow bottom edge
    if (y + CONTEXT_MENU_HEIGHT > viewportHeight) {
      y = viewportHeight - CONTEXT_MENU_HEIGHT - 8;
    }
    
    // Ensure minimum position
    x = Math.max(8, x);
    y = Math.max(8, y);
    
    setContextMenu({ x, y, file });
  };

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  // Copy path to clipboard
  const handleCopyPath = async (file: FileEntry) => {
    try {
      await navigator.clipboard.writeText(file.path);
      closeContextMenu();
    } catch (err) {
      console.error("Failed to copy path:", err);
    }
  };

  // Start rename
  const handleStartRename = (file: FileEntry) => {
    closeContextMenu();
    setRenameFile(file.path);
    setRenameName(file.name);
    setTimeout(() => renameInputRef.current?.select(), 0);
  };

  // Cancel rename
  const handleCancelRename = () => {
    setRenameFile(null);
    setRenameName("");
  };

  // Submit rename (placeholder - needs backend)
  const handleSubmitRename = async (file: FileEntry) => {
    if (!renameName.trim() || renameName === file.name) {
      handleCancelRename();
      return;
    }
    // TODO: Implement rename via backend
    console.log("Rename:", file.path, "->", renameName);
    handleCancelRename();
    loadFiles(currentPath);
  };

  // Delete files (placeholder - needs backend)
  const handleDelete = async () => {
    closeContextMenu();
    const selected = Array.from(selectedIndices).map((i) => displayFiles[i]);
    if (selected.length === 0) return;
    
    const names = selected.map((f) => f.name).join(", ");
    if (!confirm(`Delete ${selected.length} item(s)?\n${names}`)) return;
    
    // TODO: Implement delete via backend
    console.log("Delete:", selected.map((f) => f.path));
    loadFiles(currentPath);
  };

  // Download file (placeholder - needs backend)
  const handleDownload = async (file: FileEntry) => {
    closeContextMenu();
    // TODO: Implement download via backend
    console.log("Download:", file.path);
  };

  // Sort toggle
  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDirection((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortDirection("asc");
    }
  };

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Close context menu on any key
    if (contextMenu) {
      closeContextMenu();
    }

    if (displayFiles.length === 0) return;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndices(() => {
          const current = lastSelectedIndex >= 0 ? lastSelectedIndex : -1;
          const next = Math.min(current + 1, displayFiles.length - 1);
          setLastSelectedIndex(next);
          return new Set([next]);
        });
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndices(() => {
          const current = lastSelectedIndex >= 0 ? lastSelectedIndex : displayFiles.length;
          const next = Math.max(current - 1, 0);
          setLastSelectedIndex(next);
          return new Set([next]);
        });
        break;
      case "Enter":
        if (lastSelectedIndex >= 0 && lastSelectedIndex < displayFiles.length) {
          navigateTo(displayFiles[lastSelectedIndex]);
        }
        break;
      case "Backspace":
        if (!renameFile) {
          navigateUp();
        }
        break;
      case "Escape":
        setSelectedIndices(new Set());
        setLastSelectedIndex(-1);
        handleCancelRename();
        break;
      case "Delete":
        if (selectedIndices.size > 0) {
          handleDelete();
        }
        break;
      case "F2":
        if (selectedIndices.size === 1) {
          const idx = Array.from(selectedIndices)[0];
          handleStartRename(displayFiles[idx]);
        }
        break;
      case "a":
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          const all = new Set<number>();
          for (let i = 0; i < displayFiles.length; i++) all.add(i);
          setSelectedIndices(all);
        }
        break;
    }
  };

  // Close context menu on click outside - use ref to avoid re-subscribing
  const contextMenuRef = useRef<ContextMenuState | null>(null);
  contextMenuRef.current = contextMenu;
  
  useEffect(() => {
    const handleClickOutside = () => {
      if (contextMenuRef.current) {
        setContextMenu(null);
      }
    };
    document.addEventListener("click", handleClickOutside);
    return () => document.removeEventListener("click", handleClickOutside);
  }, []);

  // Focus rename input
  useEffect(() => {
    if (renameFile && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renameFile]);

  return (
    <div className="file-browser" tabIndex={0} onKeyDown={handleKeyDown} ref={containerRef} onClick={closeContextMenu}>
      {/* Toolbar */}
      <div className="browser-toolbar">
        <div className="toolbar-left">
          <button
            className="btn-icon"
            onClick={navigateUp}
            disabled={currentPath === "/" || currentPath === "" || loading}
            title="Go up (Backspace)"
          >
            <ChevronUp size={16} />
          </button>
          <button
            className="btn-icon"
            onClick={() => loadFiles(currentPath)}
            disabled={loading}
            title="Refresh"
          >
            <RefreshCw size={16} className={isPending ? "spinning" : ""} />
          </button>
          <div className="toolbar-divider" />
          {selectedIndices.size > 0 && (
            <>
              <span className="selection-count">
                {selectedIndices.size} selected
              </span>
              <button
                className="btn-icon btn-danger"
                onClick={handleDelete}
                title="Delete (Del)"
              >
                <Trash2 size={14} />
              </button>
            </>
          )}
        </div>
        <div className="toolbar-right">
          <div className="search-box">
            <Search size={14} className="search-icon" />
            <input
              type="text"
              placeholder="Filter files..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="search-input"
            />
            {searchQuery && (
              <button
                className="btn-icon btn-clear"
                onClick={() => setSearchQuery("")}
                title="Clear search"
              >
                <X size={12} />
              </button>
            )}
          </div>
          <div className="toolbar-divider" />
          <div className="view-toggle">
            <button
              className={`btn-icon ${viewMode === "list" ? "active" : ""}`}
              onClick={() => setViewMode("list")}
              title="List view"
            >
              <List size={14} />
            </button>
            <button
              className={`btn-icon ${viewMode === "grid" ? "active" : ""}`}
              onClick={() => setViewMode("grid")}
              title="Grid view"
            >
              <Grid size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Breadcrumbs */}
      <div className="browser-header">
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
          {isPending && <span className="pending-indicator">...</span>}
        </div>
        {displayFiles.length > 0 && (
          <span className="file-count">
            {displayFiles.length} item{displayFiles.length !== 1 ? "s" : ""}
            {searchQuery && ` (filtered)`}
          </span>
        )}
      </div>

      {error && <div className="error-banner">{error}</div>}

      {loading ? (
        <div className="loading-state">
          <div className="loading-spinner" />
          <span>Loading files...</span>
        </div>
      ) : displayFiles.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">
            <FolderOpen size={32} />
          </div>
          <p>{searchQuery ? "No matching files" : "This folder is empty"}</p>
          {searchQuery && (
            <button className="btn-secondary" onClick={() => setSearchQuery("")}>
              Clear filter
            </button>
          )}
        </div>
      ) : viewMode === "list" ? (
        <div className={`file-table-container ${isPending ? "stale" : ""}`}>
          {/* Sortable table header */}
          <div className="file-table-header">
            <div className="col-checkbox">
              <input
                type="checkbox"
                checked={selectedIndices.size === displayFiles.length && displayFiles.length > 0}
                onChange={(e) => {
                  if (e.target.checked) {
                    const all = new Set<number>();
                    for (let i = 0; i < displayFiles.length; i++) all.add(i);
                    setSelectedIndices(all);
                  } else {
                    setSelectedIndices(new Set());
                  }
                }}
                title="Select all (Ctrl+A)"
              />
            </div>
            <div
              className={`col-name sortable ${sortField === "name" ? "sorted" : ""}`}
              onClick={() => handleSort("name")}
            >
              Name
              {sortField === "name" && (
                <ChevronDown size={12} className={`sort-icon ${sortDirection}`} />
              )}
            </div>
            <div
              className={`col-size sortable ${sortField === "size" ? "sorted" : ""}`}
              onClick={() => handleSort("size")}
            >
              Size
              {sortField === "size" && (
                <ChevronDown size={12} className={`sort-icon ${sortDirection}`} />
              )}
            </div>
            <div
              className={`col-modified sortable ${sortField === "modified" ? "sorted" : ""}`}
              onClick={() => handleSort("modified")}
            >
              Modified
              {sortField === "modified" && (
                <ChevronDown size={12} className={`sort-icon ${sortDirection}`} />
              )}
            </div>
            <div className="col-actions"></div>
          </div>

          {/* Virtualized scroll container */}
          <div ref={scrollContainerRef} className="file-table-body">
            <div
              style={{
                height: `${rowVirtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const index = virtualRow.index;
                const file = displayFiles[index];
                const lock = getLockStatus(file.path);
                const lockedByOther = isLockedByOther(file.path);
                const lockedByMe = isLockedByMe(file.path);
                const isSelected = selectedIndices.has(index);
                const isRenaming = renameFile === file.path;

                return (
                  <FileRow
                    key={file.path}
                    file={file}
                    index={index}
                    isSelected={isSelected}
                    isRenaming={isRenaming}
                    renameName={renameName}
                    lock={lock}
                    lockedByOther={lockedByOther}
                    lockedByMe={lockedByMe}
                    virtualRow={virtualRow}
                    onRowClick={handleRowClick}
                    onDoubleClick={navigateTo}
                    onContextMenu={handleContextMenu}
                    onRenameChange={setRenameName}
                    onRenameSubmit={handleSubmitRename}
                    onRenameCancel={handleCancelRename}
                    renameInputRef={renameInputRef}
                  />
                );
              })}
            </div>
          </div>
        </div>
      ) : (
        // Grid view
        <div className={`file-grid-container ${isPending ? "stale" : ""}`}>
          <div ref={scrollContainerRef} className="file-grid">
            {displayFiles.map((file, index) => {
              const isSelected = selectedIndices.has(index);
              const lockedByOther = isLockedByOther(file.path);
              const lockedByMe = isLockedByMe(file.path);

              return (
                <div
                  key={file.path}
                  className={`file-grid-item ${file.is_dir ? "directory" : "file"} ${isSelected ? "selected" : ""} ${lockedByOther ? "locked-other" : ""} ${lockedByMe ? "locked-mine" : ""}`}
                  onClick={(e) => handleRowClick(e, index)}
                  onDoubleClick={() => navigateTo(file)}
                  onContextMenu={(e) => handleContextMenu(e, file, index)}
                >
                  <div className="grid-item-icon">
                    {getFileIconComponent(file)}
                  </div>
                  <span className="grid-item-name" title={file.name}>
                    {file.name}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div className="context-overlay" onClick={closeContextMenu} />
          <div
            className="file-context-menu"
            style={{
              top: contextMenu.y,
              left: contextMenu.x,
              maxHeight: `calc(100vh - ${contextMenu.y + 16}px)`,
            }}
            onClick={(e) => e.stopPropagation()}
          >
            {contextMenu.file.is_dir ? (
              <>
                <button onClick={() => { navigateTo(contextMenu.file); closeContextMenu(); }}>
                  <FolderOpen size={14} />
                  Open
                </button>
                <div className="context-divider" />
              </>
            ) : (
              <>
                <button onClick={() => handleDownload(contextMenu.file)}>
                  <Download size={14} />
                  Download
                </button>
                <div className="context-divider" />
                {isLockedByMe(contextMenu.file.path) ? (
                  <button onClick={() => { releaseLock(contextMenu.file.path); closeContextMenu(); }}>
                    <Unlock size={14} />
                    Release Lock
                  </button>
                ) : !isLockedByOther(contextMenu.file.path) ? (
                  <>
                    <button onClick={() => { acquireLock(contextMenu.file.path, "advisory"); closeContextMenu(); }}>
                      <Lock size={14} />
                      Advisory Lock
                    </button>
                    <button onClick={() => { acquireLock(contextMenu.file.path, "exclusive"); closeContextMenu(); }}>
                      <Lock size={14} />
                      Exclusive Lock
                    </button>
                  </>
                ) : null}
                <div className="context-divider" />
              </>
            )}
            <button onClick={() => handleStartRename(contextMenu.file)}>
              <Pencil size={14} />
              Rename
              <span className="shortcut">F2</span>
            </button>
            <button onClick={() => handleCopyPath(contextMenu.file)}>
              <Copy size={14} />
              Copy Path
            </button>
            <div className="context-divider" />
            <button className="danger" onClick={handleDelete}>
              <Trash2 size={14} />
              Delete
              <span className="shortcut">Del</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}

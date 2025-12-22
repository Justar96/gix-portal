interface DriveInfo {
  id: string;
  name: string;
  local_path: string;
  total_size: number;
  file_count: number;
}

interface DriveListProps {
  drives: DriveInfo[];
  onSelect: (drive: DriveInfo) => void;
  selectedId: string | null;
}

export function DriveList({ drives, onSelect, selectedId }: DriveListProps) {
  const formatSize = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
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
    <div className="drive-list">
      {drives.map((drive) => (
        <div
          key={drive.id}
          className={`drive-item ${selectedId === drive.id ? "selected" : ""}`}
          onClick={() => onSelect(drive)}
        >
          <div className="drive-icon">📁</div>
          <div className="drive-info">
            <div className="drive-name">{drive.name}</div>
            <div className="drive-stats">
              {drive.file_count} files &middot; {formatSize(drive.total_size)}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

// Shared TypeScript types for the P2P Drive application

/** Node identity information */
export interface IdentityInfo {
    node_id: string;
    short_id: string;
}

/** P2P connection status */
export interface ConnectionInfo {
    is_online: boolean;
    node_id: string | null;
    relay_url: string | null;
    peer_count: number;
}

/** Shared drive information */
export interface DriveInfo {
    id: string;
    name: string;
    local_path: string;
    owner: string;
    created_at: string;
    total_size: number;
    file_count: number;
}

/** File or directory entry */
export interface FileEntry {
    name: string;
    path: string;
    is_dir: boolean;
    size: number;
    modified_at: string;
}

/** File type categories for icon mapping */
export type FileCategory =
    | "folder"
    | "document"
    | "image"
    | "video"
    | "audio"
    | "code"
    | "archive"
    | "data"
    | "unknown";

/**
 * Get file category from file extension
 */
export function getFileCategory(filename: string): FileCategory {
    const ext = filename.split(".").pop()?.toLowerCase() || "";

    const categories: Record<string, FileCategory> = {
        // Documents
        pdf: "document",
        doc: "document",
        docx: "document",
        txt: "document",
        rtf: "document",
        odt: "document",
        md: "document",

        // Images
        jpg: "image",
        jpeg: "image",
        png: "image",
        gif: "image",
        svg: "image",
        webp: "image",
        bmp: "image",
        ico: "image",

        // Video
        mp4: "video",
        mkv: "video",
        avi: "video",
        mov: "video",
        wmv: "video",
        webm: "video",

        // Audio
        mp3: "audio",
        wav: "audio",
        flac: "audio",
        ogg: "audio",
        aac: "audio",
        m4a: "audio",

        // Code
        js: "code",
        ts: "code",
        jsx: "code",
        tsx: "code",
        py: "code",
        rs: "code",
        go: "code",
        java: "code",
        c: "code",
        cpp: "code",
        h: "code",
        css: "code",
        html: "code",
        json: "code",
        yaml: "code",
        yml: "code",
        toml: "code",
        xml: "code",

        // Archives
        zip: "archive",
        rar: "archive",
        "7z": "archive",
        tar: "archive",
        gz: "archive",

        // Data
        csv: "data",
        xls: "data",
        xlsx: "data",
        db: "data",
        sql: "data",
    };

    return categories[ext] || "unknown";
}

/**
 * Get emoji icon for file category
 */
export function getFileIcon(entry: FileEntry): string {
    if (entry.is_dir) return "📁";

    const category = getFileCategory(entry.name);
    const icons: Record<FileCategory, string> = {
        folder: "📁",
        document: "📄",
        image: "🖼️",
        video: "🎬",
        audio: "🎵",
        code: "💻",
        archive: "📦",
        data: "📊",
        unknown: "📄",
    };

    return icons[category];
}

/**
 * Format bytes to human-readable string
 */
export function formatBytes(bytes: number): string {
    if (bytes === 0) return "-";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const value = bytes / Math.pow(k, i);
    // Show more precision for small values
    const decimals = value < 10 ? 2 : value < 100 ? 1 : 0;
    return `${value.toFixed(decimals)} ${sizes[i]}`;
}

// ============================================
// Phase 2: Sync Types
// ============================================

/** Sync status for a drive */
export interface SyncStatus {
    is_syncing: boolean;
    connected_peers: number;
    last_sync: string | null;
}

/** Drive event types from backend */
export type DriveEventType =
    | "FileChanged"
    | "FileDeleted"
    | "FileEditStarted"
    | "FileEditEnded"
    | "UserJoined"
    | "UserLeft"
    | "SyncProgress"
    | "SyncComplete";

/** Base event with common fields */
interface BaseEvent {
    drive_id: string;
    event_type: DriveEventType;
    timestamp: string;
}

/** File changed event data */
export interface FileChangedEvent extends BaseEvent {
    event_type: "FileChanged";
    path: string;
    hash: string;
    size: number;
    modified_by: string;
}

/** File deleted event data */
export interface FileDeletedEvent extends BaseEvent {
    event_type: "FileDeleted";
    path: string;
    deleted_by: string;
}

/** File edit started event data */
export interface FileEditStartedEvent extends BaseEvent {
    event_type: "FileEditStarted";
    path: string;
    editor: string;
}

/** File edit ended event data */
export interface FileEditEndedEvent extends BaseEvent {
    event_type: "FileEditEnded";
    path: string;
    editor: string;
}

/** User joined event data */
export interface UserJoinedEvent extends BaseEvent {
    event_type: "UserJoined";
    user: string;
}

/** User left event data */
export interface UserLeftEvent extends BaseEvent {
    event_type: "UserLeft";
    user: string;
}

/** Sync progress event data */
export interface SyncProgressEvent extends BaseEvent {
    event_type: "SyncProgress";
    path: string;
    bytes_transferred: number;
    total_bytes: number;
}

/** Sync complete event data */
export interface SyncCompleteEvent extends BaseEvent {
    event_type: "SyncComplete";
    path: string;
    hash: string;
}

/** Union type of all drive events */
export type DriveEvent =
    | FileChangedEvent
    | FileDeletedEvent
    | FileEditStartedEvent
    | FileEditEndedEvent
    | UserJoinedEvent
    | UserLeftEvent
    | SyncProgressEvent
    | SyncCompleteEvent;

// ============================================
// Phase 2.4: File Transfer Types
// ============================================

/** Transfer direction */
export type TransferDirection = "Upload" | "Download";

/** Transfer status */
export type TransferStatus =
    | "Pending"
    | "InProgress"
    | "Completed"
    | "Failed"
    | "Cancelled";

/** Transfer state for tracking active transfers */
export interface TransferState {
    /** Unique transfer ID */
    id: string;
    /** Drive this transfer belongs to */
    drive_id: string;
    /** File path (relative to drive root) */
    path: string;
    /** Transfer direction */
    direction: TransferDirection;
    /** Current state */
    status: TransferStatus;
    /** Bytes transferred so far */
    bytes_transferred: number;
    /** Total bytes to transfer */
    total_bytes: number;
    /** BLAKE3 hash of the content */
    hash: string | null;
    /** Error message if failed */
    error: string | null;
}

/** Progress event for transfers */
export interface TransferProgress {
    transfer_id: string;
    drive_id: string;
    path: string;
    direction: TransferDirection;
    bytes_transferred: number;
    total_bytes: number;
    status: TransferStatus;
}

/**
 * Calculate transfer progress percentage
 */
export function getTransferProgress(transfer: TransferState | TransferProgress): number {
    if (transfer.total_bytes === 0) return 0;
    return Math.round((transfer.bytes_transferred / transfer.total_bytes) * 100);
}

/**
 * Format transfer speed (bytes per second)
 */
export function formatSpeed(bytesPerSecond: number): string {
    return `${formatBytes(bytesPerSecond)}/s`;
}

/**
 * Format date string to localized display
 */
export function formatDate(dateStr: string): string {
    try {
        const date = new Date(dateStr);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

        // If today, show time
        if (diffDays === 0) {
            return date.toLocaleTimeString(undefined, {
                hour: "2-digit",
                minute: "2-digit",
            });
        }

        // If yesterday
        if (diffDays === 1) {
            return "Yesterday";
        }

        // If within a week
        if (diffDays < 7) {
            return date.toLocaleDateString(undefined, { weekday: "long" });
        }

        // Otherwise show date
        return date.toLocaleDateString();
    } catch {
        return "-";
    }
}

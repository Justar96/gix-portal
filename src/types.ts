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

// ============================================
// Phase 3: Security & Permission Types
// ============================================

/** Permission level for drive access */
export type PermissionLevel = "read" | "write" | "manage" | "admin";

/** Permission display names */
export const PERMISSION_LABELS: Record<PermissionLevel, string> = {
    read: "Read",
    write: "Write",
    manage: "Manage",
    admin: "Admin",
};

/** Permission descriptions */
export const PERMISSION_DESCRIPTIONS: Record<PermissionLevel, string> = {
    read: "Can view and download files",
    write: "Can upload, modify, and delete files",
    manage: "Can manage users and permissions",
    admin: "Full control including key rotation",
};

/** User permission info from backend */
export interface UserPermission {
    node_id: string;
    permission: PermissionLevel;
    granted_by: string;
    granted_at: string;
    expires_at: string | null;
    is_owner: boolean;
}

/** Request to create an invite token */
export interface CreateInviteRequest {
    drive_id: string;
    permission: PermissionLevel;
    validity_hours?: number;
    note?: string;
    single_use?: boolean;
}

/** Generated invite token info */
export interface InviteInfo {
    token: string;
    drive_id: string;
    permission: PermissionLevel;
    expires_at: string;
    note: string | null;
    single_use: boolean;
}

/** Invite verification result */
export interface InviteVerification {
    valid: boolean;
    drive_id: string | null;
    permission: PermissionLevel | null;
    inviter: string | null;
    expires_at: string | null;
    error: string | null;
}

/**
 * Get short node ID for display (first 8 + last 4 chars)
 */
export function shortNodeId(nodeId: string): string {
    if (nodeId.length <= 16) return nodeId;
    return `${nodeId.slice(0, 8)}...${nodeId.slice(-4)}`;
}

// ============================================
// Phase 4: Collaboration Types
// ============================================

/** Lock type for files */
export type LockType = "advisory" | "exclusive";

/** Lock type display names */
export const LOCK_TYPE_LABELS: Record<LockType, string> = {
    advisory: "Advisory",
    exclusive: "Exclusive",
};

/** Lock type descriptions */
export const LOCK_TYPE_DESCRIPTIONS: Record<LockType, string> = {
    advisory: "Warns others but doesn't prevent access",
    exclusive: "Prevents others from editing the file",
};

/** File lock information */
export interface FileLockInfo {
    path: string;
    holder: string;
    lock_type: LockType;
    acquired_at: string;
    expires_at: string;
    reason: string | null;
    is_mine: boolean;
}

/** Lock acquisition result */
export interface AcquireLockResult {
    success: boolean;
    lock: FileLockInfo | null;
    error: string | null;
    warning: string | null;
}

/** Lock event types */
export type LockEventType = "FileLockAcquired" | "FileLockReleased";

/** File lock acquired event */
export interface FileLockAcquiredEvent {
    drive_id: string;
    event_type: "FileLockAcquired";
    timestamp: string;
    payload: {
        path: string;
        holder: string;
        lock_type: string;
        expires_at: string;
    };
}

/** File lock released event */
export interface FileLockReleasedEvent {
    drive_id: string;
    event_type: "FileLockReleased";
    timestamp: string;
    payload: {
        path: string;
        holder: string;
    };
}

/** Update DriveEventType to include lock events */
export type DriveEventTypeExtended =
    | DriveEventType
    | LockEventType;

/**
 * Format lock expiration time
 */
export function formatLockExpiry(expiresAt: string): string {
    try {
        const date = new Date(expiresAt);
        const now = new Date();
        const diffMs = date.getTime() - now.getTime();
        
        if (diffMs < 0) return "Expired";
        
        const diffMins = Math.floor(diffMs / (1000 * 60));
        if (diffMins < 60) return `${diffMins}m remaining`;
        
        const diffHours = Math.floor(diffMins / 60);
        return `${diffHours}h ${diffMins % 60}m remaining`;
    } catch {
        return "-";
    }
}

// ============================================
// Phase 4.2: Conflict Resolution Types
// ============================================

/** Resolution strategy for conflicts */
export type ResolutionStrategy = "KeepLocal" | "KeepRemote" | "KeepBoth" | "ManualMerge";

/** Resolution strategy labels */
export const RESOLUTION_LABELS: Record<ResolutionStrategy, string> = {
    KeepLocal: "Keep Local",
    KeepRemote: "Keep Remote",
    KeepBoth: "Keep Both",
    ManualMerge: "Manual Merge",
};

/** Resolution strategy descriptions */
export const RESOLUTION_DESCRIPTIONS: Record<ResolutionStrategy, string> = {
    KeepLocal: "Discard remote changes and keep your local version",
    KeepRemote: "Accept remote changes and replace your local version",
    KeepBoth: "Keep both versions (creates a copy with conflict suffix)",
    ManualMerge: "Manually merge the differences (for text files)",
};

/** File conflict information */
export interface FileConflictInfo {
    id: string;
    path: string;
    detected_at: string;
    local_hash: string;
    local_size: number;
    local_modified_at: string;
    local_modified_by: string;
    local_preview: string | null;
    remote_hash: string;
    remote_size: number;
    remote_modified_at: string;
    remote_modified_by: string;
    remote_preview: string | null;
    is_text_file: boolean;
    suggested_resolution: string;
    resolved: boolean;
}

/**
 * Get available resolution options for a conflict
 */
export function getResolutionOptions(conflict: FileConflictInfo): ResolutionStrategy[] {
    const options: ResolutionStrategy[] = ["KeepLocal", "KeepRemote", "KeepBoth"];
    if (conflict.is_text_file) {
        options.push("ManualMerge");
    }
    return options;
}

// ============================================
// Phase 4.3: Presence & Activity Types
// ============================================

/** User presence status */
export type PresenceStatus = "online" | "away" | "offline";

/** User presence information */
export interface UserPresenceInfo {
    node_id: string;
    short_id: string;
    status: PresenceStatus;
    joined_at: string;
    last_seen: string;
    current_activity: string | null;
    is_self: boolean;
}

/** Activity type */
export type ActivityType =
    | "FileCreated"
    | "FileModified"
    | "FileDeleted"
    | "FileRenamed"
    | "UserJoined"
    | "UserLeft"
    | "LockAcquired"
    | "LockReleased"
    | "ConflictDetected"
    | "ConflictResolved";

/** Activity entry */
export interface ActivityEntryInfo {
    id: string;
    activity_type: ActivityType;
    user_id: string;
    user_short: string;
    path: string | null;
    timestamp: string;
    details: string | null;
    is_self: boolean;
}

/** Activity type icons/labels */
export const ACTIVITY_LABELS: Record<ActivityType, string> = {
    FileCreated: "Created",
    FileModified: "Modified",
    FileDeleted: "Deleted",
    FileRenamed: "Renamed",
    UserJoined: "Joined",
    UserLeft: "Left",
    LockAcquired: "Locked",
    LockReleased: "Unlocked",
    ConflictDetected: "Conflict",
    ConflictResolved: "Resolved",
};

/**
 * Get status color class
 */
export function getStatusColor(status: PresenceStatus): string {
    switch (status) {
        case "online":
            return "status-online";
        case "away":
            return "status-away";
        case "offline":
            return "status-offline";
    }
}

/**
 * Format relative time for activity
 */
export function formatRelativeTime(timestamp: string): string {
    try {
        const date = new Date(timestamp);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffSecs = Math.floor(diffMs / 1000);

        if (diffSecs < 60) return "just now";
        if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
        if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
        return formatDate(timestamp);
    } catch {
        return "-";
    }
}


import type { DriveInfo, SyncStatus, TransferState, FileLockInfo, FileConflictInfo, UserPresenceInfo, ActivityEntryInfo } from '../types';

// Mock drive data
export const mockDrive: DriveInfo = {
    id: 'abc123def456abc123def456abc123def456abc123def456abc123def456abc1',
    name: 'Test Drive',
    local_path: '/home/user/test-drive',
    owner: 'node123',
    created_at: '2024-01-01T00:00:00Z',
    total_size: 1024000,
    file_count: 10,
};

// Mock sync status
export const mockSyncStatus: SyncStatus = {
    is_syncing: true,
    connected_peers: 3,
    last_sync: '2024-01-01T12:00:00Z',
};

// Mock transfer state
export const mockTransfer: TransferState = {
    id: 'transfer-1',
    drive_id: mockDrive.id,
    path: '/documents/file.txt',
    direction: 'Upload',
    status: 'InProgress',
    bytes_transferred: 512000,
    total_bytes: 1024000,
    hash: 'abc123',
    error: null,
};

// Mock file lock
export const mockFileLock: FileLockInfo = {
    path: '/documents/file.txt',
    holder: 'node456',
    lock_type: 'exclusive',
    acquired_at: '2024-01-01T10:00:00Z',
    expires_at: '2024-01-01T11:00:00Z',
    reason: null,
    is_mine: false,
};

// Mock conflict
export const mockConflict: FileConflictInfo = {
    id: 'conflict-1',
    path: '/documents/conflict.txt',
    detected_at: '2024-01-01T10:00:00Z',
    local_hash: 'local123',
    local_size: 1024,
    local_modified_at: '2024-01-01T09:00:00Z',
    local_modified_by: 'node123',
    local_preview: 'Local content preview...',
    remote_hash: 'remote456',
    remote_size: 2048,
    remote_modified_at: '2024-01-01T09:30:00Z',
    remote_modified_by: 'node456',
    remote_preview: 'Remote content preview...',
    is_text_file: true,
    suggested_resolution: 'KeepRemote',
    resolved: false,
};

// Mock user presence
export const mockUserPresence: UserPresenceInfo = {
    node_id: 'node123abc456def789',
    short_id: 'node123a...f789',
    status: 'online',
    joined_at: '2024-01-01T08:00:00Z',
    last_seen: '2024-01-01T12:00:00Z',
    current_activity: 'Editing file.txt',
    is_self: false,
};

// Mock activity entry
export const mockActivity: ActivityEntryInfo = {
    id: 'activity-1',
    activity_type: 'FileModified',
    user_id: 'node123',
    user_short: 'node123a',
    path: '/documents/file.txt',
    timestamp: '2024-01-01T11:30:00Z',
    details: null,
    is_self: false,
};

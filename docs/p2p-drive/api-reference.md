# API Reference

## Tauri Commands

### Drive Management

#### `create_drive`

Create and share a new drive.

```rust
#[tauri::command]
async fn create_drive(
    name: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<SharedDrive, String>
```

**Parameters:**
- `name` - Human-readable drive name
- `path` - Local filesystem path to share

**Returns:** `SharedDrive` object with drive metadata

---

#### `join_drive`

Join an existing drive via invite link.

```rust
#[tauri::command]
async fn join_drive(
    invite_link: String,
    mount_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<JoinedDrive, String>
```

**Parameters:**
- `invite_link` - Invite link (p2pshare://invite/...)
- `mount_path` - Optional local path to mount drive

**Returns:** `JoinedDrive` object

---

#### `list_drives`

List all drives (owned and joined).

```rust
#[tauri::command]
async fn list_drives(state: State<'_, AppState>) -> Result<DriveList, String>
```

**Returns:**
```rust
struct DriveList {
    owned: Vec<SharedDrive>,
    joined: Vec<JoinedDrive>,
}
```

---

#### `create_invite`

Generate invite link for a drive.

```rust
#[tauri::command]
async fn create_invite(
    drive_id: String,
    permission: Permission,
    expires_hours: Option<u32>,
    max_uses: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String>
```

**Parameters:**
- `drive_id` - Drive to share
- `permission` - Permission level (Read, Write, Manage, Admin)
- `expires_hours` - Optional expiration in hours
- `max_uses` - Optional maximum uses

**Returns:** Invite link string

---

### File Operations

#### `list_files`

List files in a drive path.

```rust
#[tauri::command]
async fn list_files(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, String>
```

**Returns:**
```rust
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified_at: DateTime<Utc>,
    // Lock status
    locked_by: Option<NodeId>,
    // Edit status
    being_edited_by: Option<NodeId>,
}
```

---

#### `read_file`

Read file content.

```rust
#[tauri::command]
async fn read_file(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String>
```

---

#### `write_file`

Write file content.

```rust
#[tauri::command]
async fn write_file(
    drive_id: String,
    path: String,
    content: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<(), String>
```

---

#### `delete_file`

Delete a file.

```rust
#[tauri::command]
async fn delete_file(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String>
```

---

### Realtime Events

#### `subscribe_drive_events`

Subscribe to drive events (files, users, etc.).

```rust
#[tauri::command]
async fn subscribe_drive_events(
    drive_id: String,
    window: Window,
    state: State<'_, AppState>,
) -> Result<(), String>
```

**Emitted Events:**

Events are emitted to the frontend via `window.emit("drive-event", payload)`:

```typescript
interface DriveEventPayload {
  drive_id: string;
  event: DriveEvent;
}

type DriveEvent =
  | { type: 'FileChanged'; path: string; hash: string; size: number; modified_by: string; timestamp: string }
  | { type: 'FileDeleted'; path: string; deleted_by: string; timestamp: string }
  | { type: 'FileEditStarted'; path: string; editor: string }
  | { type: 'FileEditEnded'; path: string; editor: string }
  | { type: 'UserJoined'; user: string; permission: Permission }
  | { type: 'UserLeft'; user: string }
  | { type: 'PermissionChanged'; user: string; old?: Permission; new?: Permission; changed_by: string };
```

---

## Frontend Integration

### TypeScript Types

```typescript
// Permission levels
type Permission = 'Read' | 'Write' | 'Manage' | 'Admin';

// Drive types
interface SharedDrive {
  id: string;
  name: string;
  local_path: string;
  owner: string;
  created_at: string;
  connected_users: number;
  total_size: number;
}

interface JoinedDrive {
  id: string;
  name: string;
  owner: string;
  permission: Permission;
  mount_path?: string;
  connected: boolean;
}

// File entry
interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified_at: string;
  locked_by?: string;
  being_edited_by?: string;
}
```

### React Hooks Example

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';

// Hook to list drives
function useDrives() {
  const [drives, setDrives] = useState<DriveList | null>(null);
  
  useEffect(() => {
    invoke<DriveList>('list_drives').then(setDrives);
  }, []);
  
  return drives;
}

// Hook to subscribe to drive events
function useDriveEvents(driveId: string) {
  const [events, setEvents] = useState<DriveEvent[]>([]);
  
  useEffect(() => {
    // Subscribe to events
    invoke('subscribe_drive_events', { driveId });
    
    // Listen for events
    const unlisten = listen<DriveEventPayload>('drive-event', (event) => {
      if (event.payload.drive_id === driveId) {
        setEvents(prev => [...prev, event.payload.event]);
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, [driveId]);
  
  return events;
}

// Hook to list files with realtime updates
function useFiles(driveId: string, path: string) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const events = useDriveEvents(driveId);
  
  // Initial load
  useEffect(() => {
    invoke<FileEntry[]>('list_files', { driveId, path }).then(setFiles);
  }, [driveId, path]);
  
  // Update on events
  useEffect(() => {
    const lastEvent = events[events.length - 1];
    if (!lastEvent) return;
    
    if (lastEvent.type === 'FileChanged' || lastEvent.type === 'FileDeleted') {
      // Refresh file list
      invoke<FileEntry[]>('list_files', { driveId, path }).then(setFiles);
    }
  }, [events, driveId, path]);
  
  return files;
}
```

---

## Performance Stats

#### `get_performance_stats`

Get realtime performance metrics.

```rust
#[tauri::command]
async fn get_performance_stats() -> PerformanceStats
```

**Returns:**
```rust
struct PerformanceStats {
    avg_frame_time_ms: f64,
    frame_drops_last_minute: u64,
    network_rtt_ms: f64,
    avg_sync_latency_ms: f64,
}
```

import { useEffect, useState, useCallback, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { FileLockInfo, AcquireLockResult, LockType } from "../types";

/** Options for the useLocking hook */
interface UseLockingOptions {
    /** The drive ID to manage locks for */
    driveId: string;
    /** Callback when a lock changes */
    onLockChange?: (path: string, lock: FileLockInfo | null) => void;
}

/** Return type for the useLocking hook */
interface UseLockingResult {
    /** Current locks for this drive */
    locks: Map<string, FileLockInfo>;
    /** Acquire a lock on a file */
    acquireLock: (path: string, lockType?: LockType) => Promise<AcquireLockResult>;
    /** Release a lock on a file */
    releaseLock: (path: string) => Promise<boolean>;
    /** Get lock status for a specific file */
    getLockStatus: (path: string) => FileLockInfo | undefined;
    /** Check if a file is locked by someone else */
    isLockedByOther: (path: string) => boolean;
    /** Check if a file is locked by me */
    isLockedByMe: (path: string) => boolean;
    /** Extend a lock */
    extendLock: (path: string, durationMins?: number) => Promise<FileLockInfo | null>;
    /** Refresh locks from backend */
    refreshLocks: () => Promise<void>;
    /** Loading state */
    isLoading: boolean;
    /** Error message if any */
    error: string | null;
}

/**
 * Hook for managing file locks in a drive
 *
 * @example
 * ```tsx
 * const { locks, acquireLock, releaseLock, isLockedByOther } = useLocking({
 *   driveId: selectedDrive.id,
 * });
 *
 * // Check if file is locked
 * if (isLockedByOther(file.path)) {
 *   console.log('File is locked by another user');
 * }
 *
 * // Acquire a lock before editing
 * const result = await acquireLock(file.path, 'exclusive');
 * if (!result.success) {
 *   console.error(result.error);
 * }
 * ```
 */
export function useLocking({
    driveId,
    onLockChange,
}: UseLockingOptions): UseLockingResult {
    const [locks, setLocks] = useState<Map<string, FileLockInfo>>(new Map());
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    
    // Use ref for callback to avoid re-subscribing to events
    const onLockChangeRef = useRef(onLockChange);
    onLockChangeRef.current = onLockChange;

    // Fetch initial locks
    const refreshLocks = useCallback(async () => {
        if (!driveId) return;

        setIsLoading(true);
        try {
            const lockList = await invoke<FileLockInfo[]>("list_locks", {
                driveId,
            });

            const lockMap = new Map<string, FileLockInfo>();
            for (const lock of lockList) {
                lockMap.set(lock.path, lock);
            }
            setLocks(lockMap);
            setError(null);
        } catch (err) {
            console.warn("Failed to fetch locks:", err);
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setIsLoading(false);
        }
    }, [driveId]);

    // Subscribe to lock events - only re-subscribe when driveId changes
    useEffect(() => {
        let unlisten: UnlistenFn | null = null;
        let mounted = true;

        const setup = async () => {
            // Listen for drive events (including lock events)
            unlisten = await listen<{
                drive_id: string;
                event_type: string;
                payload: unknown;
            }>("drive-event", (event) => {
                if (!mounted) return;
                
                const { drive_id, event_type, payload } = event.payload;

                // Only process events for our drive
                if (drive_id !== driveId) return;

                if (event_type === "FileLockAcquired") {
                    const data = payload as {
                        FileLockAcquired: {
                            path: string;
                            holder: string;
                            lock_type: string;
                            expires_at: string;
                            timestamp: string;
                        };
                    };
                    const lockData = data.FileLockAcquired;

                    const newLock: FileLockInfo = {
                        path: lockData.path,
                        holder: lockData.holder,
                        lock_type: lockData.lock_type as LockType,
                        acquired_at: lockData.timestamp,
                        expires_at: lockData.expires_at,
                        reason: null,
                        is_mine: false, // Will be updated on next refresh
                    };

                    setLocks((prev) => {
                        const next = new Map(prev);
                        next.set(lockData.path, newLock);
                        return next;
                    });

                    onLockChangeRef.current?.(lockData.path, newLock);
                } else if (event_type === "FileLockReleased") {
                    const data = payload as {
                        FileLockReleased: {
                            path: string;
                            holder: string;
                            timestamp: string;
                        };
                    };
                    const lockData = data.FileLockReleased;

                    setLocks((prev) => {
                        const next = new Map(prev);
                        next.delete(lockData.path);
                        return next;
                    });

                    onLockChangeRef.current?.(lockData.path, null);
                }
            });

            // Initial fetch
            if (mounted) {
                await refreshLocks();
            }
        };

        setup();

        return () => {
            mounted = false;
            unlisten?.();
        };
    }, [driveId, refreshLocks]);

    // Acquire a lock
    const acquireLock = useCallback(
        async (
            path: string,
            lockType: LockType = "advisory"
        ): Promise<AcquireLockResult> => {
            try {
                const result = await invoke<AcquireLockResult>("acquire_lock", {
                    driveId,
                    path,
                    lockType,
                });

                if (result.success && result.lock) {
                    setLocks((prev) => {
                        const next = new Map(prev);
                        next.set(path, result.lock!);
                        return next;
                    });
                }

                return result;
            } catch (err) {
                const message = err instanceof Error ? err.message : String(err);
                return {
                    success: false,
                    lock: null,
                    error: message,
                    warning: null,
                };
            }
        },
        [driveId]
    );

    // Release a lock
    const releaseLock = useCallback(
        async (path: string): Promise<boolean> => {
            try {
                const released = await invoke<boolean>("release_lock", {
                    driveId,
                    path,
                });

                if (released) {
                    setLocks((prev) => {
                        const next = new Map(prev);
                        next.delete(path);
                        return next;
                    });
                }

                return released;
            } catch (err) {
                console.error("Failed to release lock:", err);
                return false;
            }
        },
        [driveId]
    );

    // Get lock status for a path
    const getLockStatus = useCallback(
        (path: string): FileLockInfo | undefined => {
            return locks.get(path);
        },
        [locks]
    );

    // Check if locked by someone else
    const isLockedByOther = useCallback(
        (path: string): boolean => {
            const lock = locks.get(path);
            return lock !== undefined && !lock.is_mine;
        },
        [locks]
    );

    // Check if locked by me
    const isLockedByMe = useCallback(
        (path: string): boolean => {
            const lock = locks.get(path);
            return lock !== undefined && lock.is_mine;
        },
        [locks]
    );

    // Extend a lock
    const extendLock = useCallback(
        async (
            path: string,
            durationMins: number = 30
        ): Promise<FileLockInfo | null> => {
            try {
                const lock = await invoke<FileLockInfo | null>("extend_lock", {
                    driveId,
                    path,
                    durationMins,
                });

                if (lock) {
                    setLocks((prev) => {
                        const next = new Map(prev);
                        next.set(path, lock);
                        return next;
                    });
                }

                return lock;
            } catch (err) {
                console.error("Failed to extend lock:", err);
                return null;
            }
        },
        [driveId]
    );

    return {
        locks,
        acquireLock,
        releaseLock,
        getLockStatus,
        isLockedByOther,
        isLockedByMe,
        extendLock,
        refreshLocks,
        isLoading,
        error,
    };
}

export default useLocking;

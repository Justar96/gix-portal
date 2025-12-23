import { useEffect, useState, useCallback } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { DriveEvent, SyncStatus } from "../types";

/** Options for the useDriveEvents hook */
interface UseDriveEventsOptions {
    /** The drive ID to subscribe to events for */
    driveId: string;
    /** Maximum number of events to keep in history */
    maxEvents?: number;
    /** Callback when a new event is received */
    onEvent?: (event: DriveEvent) => void;
}

/** Return type for the useDriveEvents hook */
interface UseDriveEventsResult {
    /** Recent events for this drive */
    events: DriveEvent[];
    /** Current sync status */
    syncStatus: SyncStatus | null;
    /** Whether sync is active */
    isSyncing: boolean;
    /** Start syncing this drive */
    startSync: () => Promise<void>;
    /** Stop syncing this drive */
    stopSync: () => Promise<void>;
    /** Clear event history */
    clearEvents: () => void;
    /** Error message if any */
    error: string | null;
}

/**
 * Hook for subscribing to real-time drive events
 *
 * @example
 * ```tsx
 * const { events, isSyncing, startSync, stopSync } = useDriveEvents({
 *   driveId: selectedDrive.id,
 *   onEvent: (event) => console.log('New event:', event),
 * });
 * ```
 */
export function useDriveEvents({
    driveId,
    maxEvents = 100,
    onEvent,
}: UseDriveEventsOptions): UseDriveEventsResult {
    const [events, setEvents] = useState<DriveEvent[]>([]);
    const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
    const [error, setError] = useState<string | null>(null);

    // Subscribe to Tauri events
    useEffect(() => {
        let unlisten: UnlistenFn | null = null;

        const setup = async () => {
            // Listen for drive events from backend
            unlisten = await listen<DriveEvent>("drive-event", (event) => {
                const driveEvent = event.payload;

                // Only process events for our drive
                if (driveEvent.drive_id !== driveId) return;

                // Add to event history
                setEvents((prev) => {
                    const newEvents = [driveEvent, ...prev];
                    // Trim to max events
                    return newEvents.slice(0, maxEvents);
                });

                // Call user callback
                onEvent?.(driveEvent);
            });

            // Notify backend that we're subscribed
            try {
                await invoke("subscribe_drive_events", { driveId });
            } catch (err) {
                console.warn("Failed to subscribe to drive events:", err);
            }
        };

        setup();

        return () => {
            unlisten?.();
        };
    }, [driveId, maxEvents, onEvent]);

    // Fetch initial sync status
    useEffect(() => {
        const fetchStatus = async () => {
            try {
                const status = await invoke<SyncStatus>("get_sync_status", {
                    driveId,
                });
                setSyncStatus(status);
                setError(null);
            } catch (err) {
                // Sync might not be initialized yet, which is fine
                console.debug("Sync status not available:", err);
            }
        };

        fetchStatus();

        // Refresh status periodically
        const interval = setInterval(fetchStatus, 5000);
        return () => clearInterval(interval);
    }, [driveId]);

    // Start sync handler
    const startSync = useCallback(async () => {
        try {
            await invoke("start_sync", { driveId });
            // Refresh status
            const status = await invoke<SyncStatus>("get_sync_status", {
                driveId,
            });
            setSyncStatus(status);
            setError(null);
        } catch (err) {
            const message =
                err instanceof Error ? err.message : String(err);
            setError(`Failed to start sync: ${message}`);
        }
    }, [driveId]);

    // Stop sync handler
    const stopSync = useCallback(async () => {
        try {
            await invoke("stop_sync", { driveId });
            // Refresh status
            const status = await invoke<SyncStatus>("get_sync_status", {
                driveId,
            });
            setSyncStatus(status);
            setError(null);
        } catch (err) {
            const message =
                err instanceof Error ? err.message : String(err);
            setError(`Failed to stop sync: ${message}`);
        }
    }, [driveId]);

    // Clear events
    const clearEvents = useCallback(() => {
        setEvents([]);
    }, []);

    return {
        events,
        syncStatus,
        isSyncing: syncStatus?.is_syncing ?? false,
        startSync,
        stopSync,
        clearEvents,
        error,
    };
}

export default useDriveEvents;

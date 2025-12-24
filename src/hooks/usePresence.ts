import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UserPresenceInfo, ActivityEntryInfo } from "../types";

/** Options for the usePresence hook */
interface UsePresenceOptions {
    /** The drive ID to track presence for */
    driveId: string;
    /** How often to send heartbeat (ms) */
    heartbeatInterval?: number;
    /** How often to refresh data (ms) */
    refreshInterval?: number;
}

/** Return type for the usePresence hook */
interface UsePresenceResult {
    /** Online users */
    users: UserPresenceInfo[];
    /** Online user count */
    onlineCount: number;
    /** Recent activities */
    activities: ActivityEntryInfo[];
    /** Refresh all data */
    refresh: () => Promise<void>;
    /** Loading state */
    isLoading: boolean;
    /** Error message if any */
    error: string | null;
}

/**
 * Hook for tracking user presence and activity in a drive
 *
 * @example
 * ```tsx
 * const { users, activities, onlineCount } = usePresence({
 *   driveId: selectedDrive.id,
 * });
 *
 * // Show online users count
 * <span>{onlineCount} online</span>
 * ```
 */
export function usePresence({
    driveId,
    heartbeatInterval = 30000,
    refreshInterval = 5000,
}: UsePresenceOptions): UsePresenceResult {
    const [users, setUsers] = useState<UserPresenceInfo[]>([]);
    const [activities, setActivities] = useState<ActivityEntryInfo[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    
    // Track if component is mounted
    const mountedRef = useRef(true);

    // Fetch users and activities
    const refresh = useCallback(async () => {
        if (!driveId || !mountedRef.current) return;

        try {
            const [usersData, activitiesData] = await Promise.all([
                invoke<UserPresenceInfo[]>("get_online_users", { driveId }),
                invoke<ActivityEntryInfo[]>("get_recent_activity", {
                    driveId,
                    limit: 50,
                }),
            ]);

            if (mountedRef.current) {
                setUsers(usersData);
                setActivities(activitiesData);
                setError(null);
            }
        } catch (err) {
            if (mountedRef.current) {
                console.warn("Failed to fetch presence data:", err);
                setError(err instanceof Error ? err.message : String(err));
            }
        }
    }, [driveId]);

    // Join drive on mount, leave on unmount
    useEffect(() => {
        mountedRef.current = true;
        
        if (!driveId) return;

        const join = async () => {
            setIsLoading(true);
            try {
                await invoke("join_drive_presence", { driveId });
                await refresh();
            } catch (err) {
                console.warn("Failed to join drive presence:", err);
            } finally {
                if (mountedRef.current) {
                    setIsLoading(false);
                }
            }
        };

        join();

        return () => {
            mountedRef.current = false;
            invoke("leave_drive_presence", { driveId }).catch(() => {});
        };
    }, [driveId, refresh]);

    // Periodic heartbeat
    useEffect(() => {
        if (!driveId) return;

        const interval = setInterval(() => {
            invoke("presence_heartbeat", { driveId }).catch(() => {});
        }, heartbeatInterval);

        return () => clearInterval(interval);
    }, [driveId, heartbeatInterval]);

    // Periodic refresh
    useEffect(() => {
        if (!driveId) return;

        const interval = setInterval(refresh, refreshInterval);
        return () => clearInterval(interval);
    }, [driveId, refreshInterval, refresh]);

    return {
        users,
        onlineCount: users.length,
        activities,
        refresh,
        isLoading,
        error,
    };
}

export default usePresence;

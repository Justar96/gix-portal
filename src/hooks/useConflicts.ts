import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FileConflictInfo, ResolutionStrategy } from "../types";

/** Options for the useConflicts hook */
interface UseConflictsOptions {
    /** The drive ID to manage conflicts for */
    driveId: string;
    /** Callback when conflicts change */
    onConflictChange?: (conflicts: FileConflictInfo[]) => void;
}

/** Return type for the useConflicts hook */
interface UseConflictsResult {
    /** Current conflicts for this drive */
    conflicts: FileConflictInfo[];
    /** Conflict count */
    conflictCount: number;
    /** Resolve a conflict */
    resolveConflict: (path: string, strategy: ResolutionStrategy) => Promise<boolean>;
    /** Dismiss a conflict (accepts current state) */
    dismissConflict: (path: string) => Promise<boolean>;
    /** Refresh conflicts from backend */
    refreshConflicts: () => Promise<void>;
    /** Loading state */
    isLoading: boolean;
    /** Error message if any */
    error: string | null;
}

/**
 * Hook for managing file conflicts in a drive
 *
 * @example
 * ```tsx
 * const { conflicts, resolveConflict, conflictCount } = useConflicts({
 *   driveId: selectedDrive.id,
 * });
 *
 * // Show conflict badge
 * {conflictCount > 0 && <Badge>{conflictCount}</Badge>}
 *
 * // Resolve a conflict
 * await resolveConflict(conflict.path, 'KeepLocal');
 * ```
 */
export function useConflicts({
    driveId,
    onConflictChange,
}: UseConflictsOptions): UseConflictsResult {
    const [conflicts, setConflicts] = useState<FileConflictInfo[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Fetch conflicts from backend
    const refreshConflicts = useCallback(async () => {
        if (!driveId) return;

        setIsLoading(true);
        try {
            const conflictList = await invoke<FileConflictInfo[]>("list_conflicts", {
                driveId,
            });

            setConflicts(conflictList);
            setError(null);
            onConflictChange?.(conflictList);
        } catch (err) {
            console.warn("Failed to fetch conflicts:", err);
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setIsLoading(false);
        }
    }, [driveId, onConflictChange]);

    // Initial fetch and periodic refresh
    useEffect(() => {
        refreshConflicts();

        // Refresh every 10 seconds
        const interval = setInterval(refreshConflicts, 10000);
        return () => clearInterval(interval);
    }, [refreshConflicts]);

    // Resolve a conflict
    const resolveConflict = useCallback(
        async (path: string, strategy: ResolutionStrategy): Promise<boolean> => {
            try {
                const resolved = await invoke<FileConflictInfo | null>("resolve_conflict", {
                    driveId,
                    path,
                    strategy: strategy.toLowerCase(),
                });

                if (resolved) {
                    // Remove from local state
                    setConflicts((prev) => prev.filter((c) => c.path !== path));
                    return true;
                }
                return false;
            } catch (err) {
                console.error("Failed to resolve conflict:", err);
                setError(err instanceof Error ? err.message : String(err));
                return false;
            }
        },
        [driveId]
    );

    // Dismiss a conflict
    const dismissConflict = useCallback(
        async (path: string): Promise<boolean> => {
            try {
                const dismissed = await invoke<boolean>("dismiss_conflict", {
                    driveId,
                    path,
                });

                if (dismissed) {
                    setConflicts((prev) => prev.filter((c) => c.path !== path));
                }
                return dismissed;
            } catch (err) {
                console.error("Failed to dismiss conflict:", err);
                return false;
            }
        },
        [driveId]
    );

    return {
        conflicts,
        conflictCount: conflicts.length,
        resolveConflict,
        dismissConflict,
        refreshConflicts,
        isLoading,
        error,
    };
}

export default useConflicts;

import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PermissionLevel, UserPermission } from "../types";

/** Permission check operations */
export type PermissionOperation = 
    | "read" 
    | "write" 
    | "delete" 
    | "manage_users" 
    | "manage_permissions"
    | "admin";

/** Options for the usePermissions hook */
interface UsePermissionsOptions {
    /** The drive ID to check permissions for */
    driveId: string;
    /** Auto-refresh interval in ms (default: 30s) */
    refreshInterval?: number;
}

/** Return type for the usePermissions hook */
interface UsePermissionsResult {
    /** Current user's permission level */
    permissionLevel: PermissionLevel | null;
    /** Whether the user is the owner */
    isOwner: boolean;
    /** All users with permissions (for manage/admin) */
    allPermissions: UserPermission[];
    /** Check if user can perform an operation */
    canPerform: (operation: PermissionOperation) => boolean;
    /** Check if user can access a path (for path-based ACL) */
    canAccessPath: (path: string, operation: PermissionOperation) => Promise<boolean>;
    /** Refresh permissions from backend */
    refresh: () => Promise<void>;
    /** Loading state */
    isLoading: boolean;
    /** Error message if any */
    error: string | null;
}

/** Permission hierarchy for comparison */
const PERMISSION_HIERARCHY: Record<PermissionLevel, number> = {
    read: 1,
    write: 2,
    manage: 3,
    admin: 4,
};

/** Required permission level for each operation */
const OPERATION_REQUIREMENTS: Record<PermissionOperation, PermissionLevel> = {
    read: "read",
    write: "write",
    delete: "write",
    manage_users: "manage",
    manage_permissions: "manage",
    admin: "admin",
};

/**
 * Hook for checking and managing user permissions in a drive.
 * 
 * Provides permission checking, caching, and automatic refresh.
 * 
 * @example
 * ```tsx
 * const { canPerform, permissionLevel, isOwner } = usePermissions({
 *   driveId: selectedDrive.id,
 * });
 * 
 * // Check before showing delete button
 * {canPerform("delete") && <DeleteButton />}
 * 
 * // Check permission level
 * if (permissionLevel === "admin") {
 *   // Show admin features
 * }
 * ```
 */
export function usePermissions({
    driveId,
    refreshInterval = 30000,
}: UsePermissionsOptions): UsePermissionsResult {
    const [permissionLevel, setPermissionLevel] = useState<PermissionLevel | null>(null);
    const [isOwner, setIsOwner] = useState(false);
    const [allPermissions, setAllPermissions] = useState<UserPermission[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    
    const mountedRef = useRef(true);

    // Fetch current user's permission for this drive
    const refresh = useCallback(async () => {
        if (!driveId || !mountedRef.current) return;

        try {
            // Get all permissions for the drive
            const permissions = await invoke<UserPermission[]>("list_permissions", {
                driveId,
            });

            if (!mountedRef.current) return;

            setAllPermissions(permissions);

            // Find current user's permission (the one where we are the holder)
            // The backend should mark our own permission with a flag or we check identity
            const myPermission = permissions.find(p => {
                // Backend marks owner permission
                return p.is_owner || p.node_id === "self";
            });

            if (myPermission) {
                setPermissionLevel(myPermission.permission);
                setIsOwner(myPermission.is_owner);
            } else {
                // Try to get our specific permission
                try {
                    const hasRead = await invoke<boolean>("check_permission", {
                        driveId,
                        operation: "read",
                    });
                    
                    if (hasRead) {
                        // Determine level by checking each operation
                        const hasWrite = await invoke<boolean>("check_permission", {
                            driveId,
                            operation: "write",
                        });
                        const hasManage = await invoke<boolean>("check_permission", {
                            driveId,
                            operation: "manage_users",
                        });
                        const hasAdmin = await invoke<boolean>("check_permission", {
                            driveId,
                            operation: "admin",
                        });

                        if (hasAdmin) setPermissionLevel("admin");
                        else if (hasManage) setPermissionLevel("manage");
                        else if (hasWrite) setPermissionLevel("write");
                        else setPermissionLevel("read");
                    }
                } catch {
                    // If check_permission fails, assume read access since we can see the drive
                    setPermissionLevel("read");
                }
            }

            setError(null);
        } catch (err) {
            if (mountedRef.current) {
                console.warn("Failed to fetch permissions:", err);
                setError(err instanceof Error ? err.message : String(err));
                // Default to read if we can't fetch (drive is visible)
                setPermissionLevel("read");
            }
        } finally {
            if (mountedRef.current) {
                setIsLoading(false);
            }
        }
    }, [driveId]);

    // Initial fetch and periodic refresh
    useEffect(() => {
        mountedRef.current = true;
        setIsLoading(true);
        refresh();

        const interval = setInterval(refresh, refreshInterval);

        return () => {
            mountedRef.current = false;
            clearInterval(interval);
        };
    }, [driveId, refresh, refreshInterval]);

    // Check if user can perform an operation
    const canPerform = useCallback(
        (operation: PermissionOperation): boolean => {
            if (!permissionLevel) return false;
            if (isOwner) return true; // Owner can do everything

            const requiredLevel = OPERATION_REQUIREMENTS[operation];
            const userLevel = PERMISSION_HIERARCHY[permissionLevel];
            const requiredNum = PERMISSION_HIERARCHY[requiredLevel];

            return userLevel >= requiredNum;
        },
        [permissionLevel, isOwner]
    );

    // Check if user can access a specific path
    const canAccessPath = useCallback(
        async (path: string, operation: PermissionOperation): Promise<boolean> => {
            if (!driveId) return false;
            if (isOwner) return true;

            try {
                return await invoke<boolean>("check_permission", {
                    driveId,
                    path,
                    operation,
                });
            } catch {
                // Fall back to level-based check
                return canPerform(operation);
            }
        },
        [driveId, isOwner, canPerform]
    );

    return {
        permissionLevel,
        isOwner,
        allPermissions,
        canPerform,
        canAccessPath,
        refresh,
        isLoading,
        error,
    };
}

export default usePermissions;

import { useState, useEffect } from "react";
import {
    Cloud,
    CloudOff,
    RefreshCw,
    Users,
    Eye,
    EyeOff,
    Loader2,
    CheckCircle,
    AlertCircle,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { DriveInfo } from "../types";
import { useDriveEvents } from "../hooks";

interface SyncStatusBarProps {
    drive: DriveInfo;
    presencePanelOpen?: boolean;
}

export function SyncStatusBar({ drive, presencePanelOpen = true }: SyncStatusBarProps) {
    const { syncStatus, isSyncing, startSync, stopSync, error } = useDriveEvents({
        driveId: drive.id,
    });

    const [isWatching, setIsWatching] = useState(false);
    const [watchLoading, setWatchLoading] = useState(false);
    const [syncLoading, setSyncLoading] = useState(false);

    // Check watching status on mount
    useEffect(() => {
        const checkWatching = async () => {
            try {
                const watching = await invoke<boolean>("is_watching", {
                    driveId: drive.id,
                });
                setIsWatching(watching);
            } catch (err) {
                console.warn("Failed to check watching status:", err);
            }
        };
        checkWatching();
    }, [drive.id]);

    const handleToggleSync = async () => {
        setSyncLoading(true);
        try {
            if (isSyncing) {
                await stopSync();
            } else {
                await startSync();
            }
        } finally {
            setSyncLoading(false);
        }
    };

    const handleToggleWatch = async () => {
        setWatchLoading(true);
        try {
            if (isWatching) {
                await invoke("stop_watching", { driveId: drive.id });
                setIsWatching(false);
            } else {
                await invoke("start_watching", { driveId: drive.id });
                setIsWatching(true);
            }
        } catch (err) {
            console.error("Failed to toggle watching:", err);
        } finally {
            setWatchLoading(false);
        }
    };

    return (
        <div className={`sync-status-bar ${presencePanelOpen ? 'panel-open' : 'panel-closed'}`}>
            <div className="sync-status-left">
                {/* Sync Status Indicator */}
                <div className={`sync-indicator ${isSyncing ? "syncing" : "offline"}`}>
                    {syncLoading ? (
                        <Loader2 size={14} className="spinning" />
                    ) : isSyncing ? (
                        <>
                            <Cloud size={14} />
                            <div className="sync-pulse" />
                        </>
                    ) : (
                        <CloudOff size={14} />
                    )}
                    <span>{isSyncing ? "Syncing" : "Offline"}</span>
                </div>

                {/* Peer Count */}
                {isSyncing && syncStatus && (
                    <div className="peer-count" title="Connected peers">
                        <Users size={12} />
                        <span>{syncStatus.connected_peers}</span>
                    </div>
                )}

                {/* Last Sync */}
                {syncStatus?.last_sync && (
                    <span className="last-sync" title="Last synced">
                        <CheckCircle size={12} />
                        {formatLastSync(syncStatus.last_sync)}
                    </span>
                )}

                {/* Error */}
                {error && (
                    <span className="sync-error" title={error}>
                        <AlertCircle size={12} />
                        Error
                    </span>
                )}
            </div>

            <div className="sync-status-right">
                {/* File Watching Toggle */}
                <button
                    className={`btn-status ${isWatching ? "active" : ""}`}
                    onClick={handleToggleWatch}
                    disabled={watchLoading}
                    title={isWatching ? "Stop watching for changes" : "Watch for local changes"}
                >
                    {watchLoading ? (
                        <Loader2 size={14} className="spinning" />
                    ) : isWatching ? (
                        <Eye size={14} />
                    ) : (
                        <EyeOff size={14} />
                    )}
                    <span>{isWatching ? "Watching" : "Watch"}</span>
                </button>

                {/* Sync Toggle */}
                <button
                    className={`btn-status ${isSyncing ? "active" : ""}`}
                    onClick={handleToggleSync}
                    disabled={syncLoading}
                    title={isSyncing ? "Stop syncing" : "Start syncing"}
                >
                    {syncLoading ? (
                        <Loader2 size={14} className="spinning" />
                    ) : (
                        <RefreshCw size={14} className={isSyncing ? "spinning-slow" : ""} />
                    )}
                    <span>{isSyncing ? "Stop Sync" : "Start Sync"}</span>
                </button>
            </div>
        </div>
    );
}

function formatLastSync(timestamp: string): string {
    try {
        const date = new Date(timestamp);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffSecs = Math.floor(diffMs / 1000);

        if (diffSecs < 60) return "just now";
        if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
        return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    } catch {
        return "";
    }
}

export default SyncStatusBar;

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type { TransferState, TransferProgress } from "../types";

/** Options for the useFileTransfer hook */
interface UseFileTransferOptions {
    /** Drive ID to filter transfers for (optional) */
    driveId?: string;
    /** Callback when a transfer progresses */
    onProgress?: (progress: TransferProgress) => void;
    /** Callback when a transfer completes */
    onComplete?: (transfer: TransferState) => void;
    /** Callback when a transfer fails */
    onError?: (transfer: TransferState) => void;
}

/** Return type for the useFileTransfer hook */
interface UseFileTransferResult {
    /** All active transfers */
    transfers: TransferState[];
    /** Upload a file */
    uploadFile: (driveId: string, filePath: string) => Promise<string>;
    /** Download a file */
    downloadFile: (driveId: string, hash: string, destinationPath: string) => Promise<void>;
    /** Cancel a transfer */
    cancelTransfer: (transferId: string) => Promise<void>;
    /** Refresh the transfers list */
    refreshTransfers: () => Promise<void>;
    /** Whether any transfer is in progress */
    isTransferring: boolean;
    /** Error message if any */
    error: string | null;
}

/**
 * Hook for managing file transfers
 *
 * @example
 * ```tsx
 * const { transfers, uploadFile, isTransferring } = useFileTransfer({
 *   driveId: selectedDrive.id,
 *   onComplete: (t) => console.log('Transfer complete:', t.path),
 * });
 * ```
 */
export function useFileTransfer(
    options: UseFileTransferOptions = {}
): UseFileTransferResult {
    const { driveId, onProgress, onComplete, onError } = options;

    const [transfers, setTransfers] = useState<TransferState[]>([]);
    const [error, setError] = useState<string | null>(null);

    // Fetch initial transfers list
    const refreshTransfers = useCallback(async () => {
        try {
            const allTransfers = await invoke<TransferState[]>("list_transfers");

            // Filter by drive ID if provided
            const filtered = driveId
                ? allTransfers.filter((t) => t.drive_id === driveId)
                : allTransfers;

            setTransfers(filtered);
            setError(null);
        } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            setError(`Failed to list transfers: ${message}`);
        }
    }, [driveId]);

    // Listen for transfer progress events
    useEffect(() => {
        let unlisten: UnlistenFn | null = null;

        const setup = async () => {
            unlisten = await listen<TransferProgress>(
                "transfer-progress",
                (event) => {
                    const progress = event.payload;

                    // Filter by drive ID if provided
                    if (driveId && progress.drive_id !== driveId) return;

                    // Update transfer in list
                    setTransfers((prev) =>
                        prev.map((t) =>
                            t.id === progress.transfer_id
                                ? {
                                      ...t,
                                      bytes_transferred: progress.bytes_transferred,
                                      total_bytes: progress.total_bytes,
                                      status: progress.status,
                                  }
                                : t
                        )
                    );

                    // Call progress callback
                    onProgress?.(progress);

                    // Check for completion/failure
                    if (progress.status === "Completed") {
                        const transfer = transfers.find(
                            (t) => t.id === progress.transfer_id
                        );
                        if (transfer) {
                            onComplete?.({ ...transfer, status: "Completed" });
                        }
                    } else if (progress.status === "Failed") {
                        const transfer = transfers.find(
                            (t) => t.id === progress.transfer_id
                        );
                        if (transfer) {
                            onError?.({ ...transfer, status: "Failed" });
                        }
                    }
                }
            );
        };

        setup();

        return () => {
            unlisten?.();
        };
    }, [driveId, onProgress, onComplete, onError, transfers]);

    // Fetch transfers on mount and periodically
    useEffect(() => {
        refreshTransfers();

        const interval = setInterval(refreshTransfers, 2000);
        return () => clearInterval(interval);
    }, [refreshTransfers]);

    // Upload a file
    const uploadFile = useCallback(
        async (targetDriveId: string, filePath: string): Promise<string> => {
            try {
                setError(null);
                const hash = await invoke<string>("upload_file", {
                    driveId: targetDriveId,
                    filePath,
                });

                // Refresh transfers list
                await refreshTransfers();

                return hash;
            } catch (err) {
                const message = err instanceof Error ? err.message : String(err);
                setError(`Upload failed: ${message}`);
                throw err;
            }
        },
        [refreshTransfers]
    );

    // Download a file
    const downloadFile = useCallback(
        async (
            targetDriveId: string,
            hash: string,
            destinationPath: string
        ): Promise<void> => {
            try {
                setError(null);
                await invoke("download_file", {
                    driveId: targetDriveId,
                    hash,
                    destinationPath,
                });

                // Refresh transfers list
                await refreshTransfers();
            } catch (err) {
                const message = err instanceof Error ? err.message : String(err);
                setError(`Download failed: ${message}`);
                throw err;
            }
        },
        [refreshTransfers]
    );

    // Cancel a transfer
    const cancelTransfer = useCallback(
        async (transferId: string): Promise<void> => {
            try {
                await invoke("cancel_transfer", { transferId });
                await refreshTransfers();
            } catch (err) {
                const message = err instanceof Error ? err.message : String(err);
                setError(`Cancel failed: ${message}`);
            }
        },
        [refreshTransfers]
    );

    // Check if any transfer is in progress
    const isTransferring = transfers.some(
        (t) => t.status === "InProgress" || t.status === "Pending"
    );

    return {
        transfers,
        uploadFile,
        downloadFile,
        cancelTransfer,
        refreshTransfers,
        isTransferring,
        error,
    };
}

export default useFileTransfer;

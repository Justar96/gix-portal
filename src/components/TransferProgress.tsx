import { useState } from "react";
import {
    Upload,
    Download,
    X,
    ChevronDown,
    ChevronUp,
    CheckCircle,
    AlertCircle,
    Loader2,
} from "lucide-react";
import type { DriveInfo, TransferState } from "../types";
import { formatBytes, getTransferProgress } from "../types";
import { useFileTransfer } from "../hooks";

interface TransferProgressProps {
    drive: DriveInfo;
}

export function TransferProgress({ drive }: TransferProgressProps) {
    const { transfers, cancelTransfer, isTransferring } = useFileTransfer({
        driveId: drive.id,
    });

    const [expanded, setExpanded] = useState(true);

    // Filter to show only active or recent transfers
    const activeTransfers = transfers.filter(
        (t) => t.status === "Pending" || t.status === "InProgress"
    );
    const recentCompleted = transfers
        .filter((t) => t.status === "Completed" || t.status === "Failed")
        .slice(0, 3);

    const allVisible = [...activeTransfers, ...recentCompleted];

    if (allVisible.length === 0) {
        return null;
    }

    return (
        <div className="transfer-progress">
            <div
                className="transfer-header"
                onClick={() => setExpanded(!expanded)}
            >
                <div className="transfer-title">
                    {isTransferring ? (
                        <Loader2 size={14} className="spinning" />
                    ) : (
                        <CheckCircle size={14} />
                    )}
                    <span>
                        {activeTransfers.length > 0
                            ? `${activeTransfers.length} transfer${activeTransfers.length !== 1 ? "s" : ""} in progress`
                            : "Transfers complete"}
                    </span>
                </div>
                <button className="btn-icon">
                    {expanded ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
                </button>
            </div>

            {expanded && (
                <div className="transfer-list">
                    {allVisible.map((transfer) => (
                        <TransferItem
                            key={transfer.id}
                            transfer={transfer}
                            onCancel={() => cancelTransfer(transfer.id)}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}

interface TransferItemProps {
    transfer: TransferState;
    onCancel: () => void;
}

function TransferItem({ transfer, onCancel }: TransferItemProps) {
    const progress = getTransferProgress(transfer);
    const fileName = transfer.path.split(/[/\\]/).pop() || transfer.path;
    const isActive = transfer.status === "Pending" || transfer.status === "InProgress";

    return (
        <div className={`transfer-item ${transfer.status.toLowerCase()}`}>
            <div className="transfer-icon">
                {transfer.direction === "Upload" ? (
                    <Upload size={14} />
                ) : (
                    <Download size={14} />
                )}
            </div>

            <div className="transfer-info">
                <div className="transfer-name" title={transfer.path}>
                    {fileName}
                </div>
                <div className="transfer-details">
                    {isActive ? (
                        <>
                            <span className="transfer-size">
                                {formatBytes(transfer.bytes_transferred)} / {formatBytes(transfer.total_bytes)}
                            </span>
                            <div className="progress-bar">
                                <div
                                    className="progress-fill"
                                    style={{ width: `${progress}%` }}
                                />
                            </div>
                            <span className="transfer-percent">{progress}%</span>
                        </>
                    ) : transfer.status === "Completed" ? (
                        <span className="transfer-complete">
                            <CheckCircle size={12} />
                            {formatBytes(transfer.total_bytes)}
                        </span>
                    ) : transfer.status === "Failed" ? (
                        <span className="transfer-failed">
                            <AlertCircle size={12} />
                            {transfer.error || "Failed"}
                        </span>
                    ) : (
                        <span className="transfer-cancelled">Cancelled</span>
                    )}
                </div>
            </div>

            {isActive && (
                <button
                    className="btn-icon btn-cancel"
                    onClick={onCancel}
                    title="Cancel transfer"
                >
                    <X size={14} />
                </button>
            )}
        </div>
    );
}

export default TransferProgress;

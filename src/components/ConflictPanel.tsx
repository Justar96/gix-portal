import { useState } from "react";
import {
    AlertTriangle,
    ChevronDown,
    ChevronRight,
    FileText,
    X,
    Check,
    GitMerge,
    Copy,
    ArrowLeft,
    ArrowRight,
} from "lucide-react";
import type { DriveInfo, FileConflictInfo, ResolutionStrategy } from "../types";
import {
    formatBytes,
    formatDate,
    shortNodeId,
    RESOLUTION_LABELS,
    RESOLUTION_DESCRIPTIONS,
    getResolutionOptions,
} from "../types";
import { useConflicts } from "../hooks";

interface ConflictPanelProps {
    drive: DriveInfo;
}

export function ConflictPanel({ drive }: ConflictPanelProps) {
    const { conflicts, conflictCount, resolveConflict, dismissConflict, isLoading } =
        useConflicts({ driveId: drive.id });

    const [expandedConflict, setExpandedConflict] = useState<string | null>(null);
    const [resolving, setResolving] = useState<string | null>(null);

    const handleResolve = async (path: string, strategy: ResolutionStrategy) => {
        setResolving(path);
        await resolveConflict(path, strategy);
        setResolving(null);
        setExpandedConflict(null);
    };

    const handleDismiss = async (path: string) => {
        setResolving(path);
        await dismissConflict(path);
        setResolving(null);
    };

    if (conflictCount === 0) {
        return null;
    }

    return (
        <div className="conflict-panel">
            <div className="conflict-header">
                <div className="conflict-title">
                    <AlertTriangle size={16} className="warning-icon" />
                    <span>
                        {conflictCount} Conflict{conflictCount !== 1 ? "s" : ""} Detected
                    </span>
                </div>
            </div>

            <div className="conflict-list">
                {conflicts.map((conflict) => (
                    <ConflictItem
                        key={conflict.id}
                        conflict={conflict}
                        isExpanded={expandedConflict === conflict.id}
                        isResolving={resolving === conflict.path}
                        onToggle={() =>
                            setExpandedConflict(
                                expandedConflict === conflict.id ? null : conflict.id
                            )
                        }
                        onResolve={(strategy) => handleResolve(conflict.path, strategy)}
                        onDismiss={() => handleDismiss(conflict.path)}
                    />
                ))}
            </div>

            {isLoading && (
                <div className="conflict-loading">
                    <div className="loading-spinner" />
                </div>
            )}
        </div>
    );
}

interface ConflictItemProps {
    conflict: FileConflictInfo;
    isExpanded: boolean;
    isResolving: boolean;
    onToggle: () => void;
    onResolve: (strategy: ResolutionStrategy) => void;
    onDismiss: () => void;
}

function ConflictItem({
    conflict,
    isExpanded,
    isResolving,
    onToggle,
    onResolve,
    onDismiss,
}: ConflictItemProps) {
    const resolutionOptions = getResolutionOptions(conflict);
    const fileName = conflict.path.split(/[/\\]/).pop() || conflict.path;

    return (
        <div className={`conflict-item ${isExpanded ? "expanded" : ""}`}>
            <div className="conflict-item-header" onClick={onToggle}>
                <span className="expand-icon">
                    {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </span>
                <FileText size={14} className="file-icon" />
                <span className="conflict-path" title={conflict.path}>
                    {fileName}
                </span>
                <span className="conflict-time">{formatDate(conflict.detected_at)}</span>
                <button
                    className="btn-icon btn-dismiss"
                    onClick={(e) => {
                        e.stopPropagation();
                        onDismiss();
                    }}
                    disabled={isResolving}
                    title="Dismiss"
                >
                    <X size={14} />
                </button>
            </div>

            {isExpanded && (
                <div className="conflict-details">
                    <div className="conflict-versions">
                        <div className="version local">
                            <div className="version-header">
                                <ArrowLeft size={12} />
                                <span>Local Version</span>
                            </div>
                            <div className="version-info">
                                <div className="info-row">
                                    <span className="label">Size:</span>
                                    <span className="value">{formatBytes(conflict.local_size)}</span>
                                </div>
                                <div className="info-row">
                                    <span className="label">Modified:</span>
                                    <span className="value">
                                        {formatDate(conflict.local_modified_at)}
                                    </span>
                                </div>
                                <div className="info-row">
                                    <span className="label">By:</span>
                                    <span className="value">
                                        {shortNodeId(conflict.local_modified_by)}
                                    </span>
                                </div>
                            </div>
                            {conflict.local_preview && (
                                <pre className="version-preview">{conflict.local_preview}</pre>
                            )}
                        </div>

                        <div className="version remote">
                            <div className="version-header">
                                <ArrowRight size={12} />
                                <span>Remote Version</span>
                            </div>
                            <div className="version-info">
                                <div className="info-row">
                                    <span className="label">Size:</span>
                                    <span className="value">{formatBytes(conflict.remote_size)}</span>
                                </div>
                                <div className="info-row">
                                    <span className="label">Modified:</span>
                                    <span className="value">
                                        {formatDate(conflict.remote_modified_at)}
                                    </span>
                                </div>
                                <div className="info-row">
                                    <span className="label">By:</span>
                                    <span className="value">
                                        {shortNodeId(conflict.remote_modified_by)}
                                    </span>
                                </div>
                            </div>
                            {conflict.remote_preview && (
                                <pre className="version-preview">{conflict.remote_preview}</pre>
                            )}
                        </div>
                    </div>

                    <div className="resolution-options">
                        <div className="options-label">Resolve:</div>
                        <div className="options-buttons">
                            {resolutionOptions.map((strategy) => (
                                <button
                                    key={strategy}
                                    className={`btn-resolution ${strategy.toLowerCase()}`}
                                    onClick={() => onResolve(strategy)}
                                    disabled={isResolving}
                                    title={RESOLUTION_DESCRIPTIONS[strategy]}
                                >
                                    {getStrategyIcon(strategy)}
                                    <span>{RESOLUTION_LABELS[strategy]}</span>
                                </button>
                            ))}
                        </div>
                    </div>

                    {isResolving && (
                        <div className="resolving-overlay">
                            <div className="loading-spinner" />
                            <span>Resolving...</span>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}

function getStrategyIcon(strategy: ResolutionStrategy) {
    switch (strategy) {
        case "KeepLocal":
            return <ArrowLeft size={12} />;
        case "KeepRemote":
            return <ArrowRight size={12} />;
        case "KeepBoth":
            return <Copy size={12} />;
        case "ManualMerge":
            return <GitMerge size={12} />;
        default:
            return <Check size={12} />;
    }
}

export default ConflictPanel;

import { useState } from "react";
import { Users, AlertTriangle, PanelRightClose, PanelRight } from "lucide-react";
import type { DriveInfo } from "../types";
import { FileBrowser } from "./FileBrowser";
import { SyncStatusBar } from "./SyncStatusBar";
import { ConflictPanel } from "./ConflictPanel";
import { PresencePanel } from "./PresencePanel";
import { TransferProgress } from "./TransferProgress";
import { useConflicts } from "../hooks";

interface DriveWorkspaceProps {
    drive: DriveInfo;
}

/**
 * DriveWorkspace integrates all P2P features with the file browser:
 * - SyncStatusBar: Sync controls, watching toggle, peer count
 * - ConflictPanel: Shows and resolves file conflicts
 * - PresencePanel: Online users and activity feed
 * - TransferProgress: Active file transfers
 */
export function DriveWorkspace({ drive }: DriveWorkspaceProps) {
    const [showPresence, setShowPresence] = useState(true);
    const { conflictCount } = useConflicts({ driveId: drive.id });

    return (
        <div className="drive-workspace">
            {/* Sync Status Bar */}
            <SyncStatusBar drive={drive} />

            {/* Main Content Area */}
            <div className="workspace-content">
                {/* File Browser with Conflicts */}
                <div className="workspace-main">
                    {/* Conflict Panel (shown when conflicts exist) */}
                    {conflictCount > 0 && <ConflictPanel drive={drive} />}

                    {/* File Browser */}
                    <FileBrowser drive={drive} />
                </div>

                {/* Presence Panel (collapsible sidebar) */}
                <PresencePanel 
                    drive={drive} 
                    isOpen={showPresence}
                    onToggle={() => setShowPresence(!showPresence)}
                    conflictCount={conflictCount}
                />
            </div>

            {/* Transfer Progress (floating) */}
            <TransferProgress drive={drive} />
        </div>
    );
}

export default DriveWorkspace;

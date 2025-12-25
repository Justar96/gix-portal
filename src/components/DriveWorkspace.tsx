import type { DriveInfo } from "../types";
import { FileBrowser } from "./FileBrowser";
import { SyncStatusBar } from "./SyncStatusBar";
import { ConflictPanel } from "./ConflictPanel";
import { TransferProgress } from "./TransferProgress";
import { useConflicts } from "../hooks";

interface DriveWorkspaceProps {
    drive: DriveInfo;
}

/**
 * DriveWorkspace integrates all P2P features with the file browser:
 * - SyncStatusBar: Sync controls, watching toggle, peer count
 * - ConflictPanel: Shows and resolves file conflicts
 * - TransferProgress: Active file transfers
 *
 * Note: PresencePanel is now rendered at the App level for proper layout alignment
 */
export function DriveWorkspace({ drive }: DriveWorkspaceProps) {
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
            </div>

            {/* Transfer Progress (floating) */}
            <TransferProgress drive={drive} />
        </div>
    );
}

export default DriveWorkspace;

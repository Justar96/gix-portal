import { useState } from "react";
import {
    Users,
    Activity,
    ChevronDown,
    ChevronRight,
    Circle,
    File,
    UserPlus,
    UserMinus,
    Lock,
    Unlock,
    AlertTriangle,
    Check,
    FilePlus,
    FileEdit,
    Trash2,
} from "lucide-react";
import type { DriveInfo, UserPresenceInfo, ActivityEntryInfo, ActivityType } from "../types";
import { formatRelativeTime, getStatusColor, ACTIVITY_LABELS } from "../types";
import { usePresence } from "../hooks";

interface PresencePanelProps {
    drive: DriveInfo;
}

export function PresencePanel({ drive }: PresencePanelProps) {
    const { users, onlineCount, activities, isLoading } = usePresence({
        driveId: drive.id,
    });

    const [showUsers, setShowUsers] = useState(true);
    const [showActivity, setShowActivity] = useState(true);

    return (
        <div className="presence-panel">
            {/* Online Users Section */}
            <div className="presence-section">
                <div
                    className="section-header"
                    onClick={() => setShowUsers(!showUsers)}
                >
                    <span className="expand-icon">
                        {showUsers ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </span>
                    <Users size={14} />
                    <span className="section-title">Online</span>
                    <span className="section-count">{onlineCount}</span>
                </div>

                {showUsers && (
                    <div className="section-content">
                        {users.length === 0 ? (
                            <div className="empty-section">
                                <span>No users online</span>
                            </div>
                        ) : (
                            <ul className="user-list">
                                {users.map((user) => (
                                    <UserItem key={user.node_id} user={user} />
                                ))}
                            </ul>
                        )}
                    </div>
                )}
            </div>

            {/* Activity Feed Section */}
            <div className="presence-section">
                <div
                    className="section-header"
                    onClick={() => setShowActivity(!showActivity)}
                >
                    <span className="expand-icon">
                        {showActivity ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </span>
                    <Activity size={14} />
                    <span className="section-title">Activity</span>
                </div>

                {showActivity && (
                    <div className="section-content">
                        {activities.length === 0 ? (
                            <div className="empty-section">
                                <span>No recent activity</span>
                            </div>
                        ) : (
                            <ul className="activity-list">
                                {activities.slice(0, 20).map((activity) => (
                                    <ActivityItem key={activity.id} activity={activity} />
                                ))}
                            </ul>
                        )}
                    </div>
                )}
            </div>

            {isLoading && (
                <div className="presence-loading">
                    <div className="loading-spinner" />
                </div>
            )}
        </div>
    );
}

interface UserItemProps {
    user: UserPresenceInfo;
}

function UserItem({ user }: UserItemProps) {
    return (
        <li className={`user-item ${user.is_self ? "is-self" : ""}`}>
            <span className={`status-dot ${getStatusColor(user.status)}`}>
                <Circle size={8} fill="currentColor" />
            </span>
            <span className="user-id" title={user.node_id}>
                {user.short_id}
                {user.is_self && <span className="self-badge">(you)</span>}
            </span>
            {user.current_activity && (
                <span className="user-activity" title={user.current_activity}>
                    {user.current_activity}
                </span>
            )}
        </li>
    );
}

interface ActivityItemProps {
    activity: ActivityEntryInfo;
}

function ActivityItem({ activity }: ActivityItemProps) {
    const fileName = activity.path?.split(/[/\\]/).pop();

    return (
        <li className={`activity-item ${activity.is_self ? "is-self" : ""}`}>
            <span className="activity-icon">
                {getActivityIcon(activity.activity_type)}
            </span>
            <div className="activity-content">
                <span className="activity-user">{activity.user_short}</span>
                <span className="activity-action">
                    {ACTIVITY_LABELS[activity.activity_type] || activity.activity_type}
                </span>
                {fileName && (
                    <span className="activity-file" title={activity.path || ""}>
                        {fileName}
                    </span>
                )}
            </div>
            <span className="activity-time">
                {formatRelativeTime(activity.timestamp)}
            </span>
        </li>
    );
}

function getActivityIcon(type: ActivityType) {
    const size = 12;
    switch (type) {
        case "FileCreated":
            return <FilePlus size={size} className="icon-created" />;
        case "FileModified":
            return <FileEdit size={size} className="icon-modified" />;
        case "FileDeleted":
            return <Trash2 size={size} className="icon-deleted" />;
        case "FileRenamed":
            return <File size={size} className="icon-renamed" />;
        case "UserJoined":
            return <UserPlus size={size} className="icon-joined" />;
        case "UserLeft":
            return <UserMinus size={size} className="icon-left" />;
        case "LockAcquired":
            return <Lock size={size} className="icon-locked" />;
        case "LockReleased":
            return <Unlock size={size} className="icon-unlocked" />;
        case "ConflictDetected":
            return <AlertTriangle size={size} className="icon-conflict" />;
        case "ConflictResolved":
            return <Check size={size} className="icon-resolved" />;
        default:
            return <Activity size={size} />;
    }
}

export default PresencePanel;

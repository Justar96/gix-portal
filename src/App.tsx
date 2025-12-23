import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Plus, PanelLeftClose, PanelLeft } from "lucide-react";
import { Titlebar } from "./components/Titlebar";
import { IdentityBadge } from "./components/IdentityBadge";
import { DriveList } from "./components/DriveList";
import { CreateDriveModal } from "./components/CreateDriveModal";
import { FileBrowser } from "./components/FileBrowser";
import type { DriveInfo } from "./types";
import "./styles/main.scss";

function App() {
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [selectedDrive, setSelectedDrive] = useState<DriveInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const loadDrives = useCallback(async () => {
    try {
      const driveList = await invoke<DriveInfo[]>("list_drives");
      setDrives(driveList);

      // If selected drive was deleted, clear selection
      if (selectedDrive && !driveList.find((d) => d.id === selectedDrive.id)) {
        setSelectedDrive(null);
      }
    } catch (error) {
      console.error("Failed to load drives:", error);
    } finally {
      setLoading(false);
    }
  }, [selectedDrive]);

  useEffect(() => {
    // Wait a bit for the backend to initialize
    const timer = setTimeout(() => {
      loadDrives();
    }, 1000);
    return () => clearTimeout(timer);
  }, []);

  const handleDriveCreated = (drive: DriveInfo) => {
    setShowCreateModal(false);
    setDrives((prev) => [...prev, drive]);
    setSelectedDrive(drive);
  };

  const handleSelectDrive = (drive: DriveInfo) => {
    setSelectedDrive(drive);
  };

  return (
    <div className="app">
      <Titlebar />
      <main className="app-main">
        <aside className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
          <div className="sidebar-header">
            <button
              className="btn-icon btn-collapse"
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
            >
              {sidebarCollapsed ? <PanelLeft size={14} /> : <PanelLeftClose size={14} />}
            </button>
            {!sidebarCollapsed && (
              <>
                <span className="sidebar-title">Drives</span>
                <button
                  className="btn-icon btn-add"
                  onClick={() => setShowCreateModal(true)}
                  title="Create new drive"
                >
                  <Plus size={14} />
                </button>
              </>
            )}
          </div>

          {!sidebarCollapsed && (
            <div className="sidebar-content">
              {loading ? (
                <div className="loading-state">
                  <div className="loading-spinner" />
                </div>
              ) : (
                <DriveList
                  drives={drives}
                  onSelect={handleSelectDrive}
                  onUpdate={loadDrives}
                  selectedId={selectedDrive?.id ?? null}
                />
              )}
            </div>
          )}
          
          {sidebarCollapsed && (
            <div className="sidebar-collapsed-content">
              <button
                className="btn-icon btn-add-collapsed"
                onClick={() => setShowCreateModal(true)}
                title="Create new drive"
              >
                <Plus size={16} />
              </button>
              {drives.slice(0, 6).map((drive) => (
                <button
                  key={drive.id}
                  className={`collapsed-drive-item ${selectedDrive?.id === drive.id ? 'selected' : ''}`}
                  onClick={() => handleSelectDrive(drive)}
                  title={drive.name}
                >
                  <FolderOpen size={16} />
                </button>
              ))}
              {drives.length > 6 && (
                <span className="collapsed-more">+{drives.length - 6}</span>
              )}
            </div>
          )}
        </aside>

        <section className="content">
          <header className="content-header">
            <div className="content-header-left">
              {selectedDrive && (
                <h1 className="content-title">{selectedDrive.name}</h1>
              )}
            </div>
            <div className="content-header-right">
              <IdentityBadge />
            </div>
          </header>
          <div className="content-body">
            {selectedDrive ? (
              <FileBrowser drive={selectedDrive} />
            ) : (
              <div className="empty-state">
                <div className="empty-icon">
                  <FolderOpen size={24} />
                </div>
                <h3>Welcome to Gix</h3>
                <p>Create a drive to start sharing files peer-to-peer</p>
                <button
                  className="btn-primary"
                  onClick={() => setShowCreateModal(true)}
                >
                  <Plus size={14} />
                  Create Drive
                </button>
              </div>
            )}
          </div>
        </section>
      </main>

      {showCreateModal && (
        <CreateDriveModal
          onClose={() => setShowCreateModal(false)}
          onCreated={handleDriveCreated}
        />
      )}
    </div>
  );
}

export default App;

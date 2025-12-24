import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { SyncStatusBar } from './SyncStatusBar';
import { mockDrive } from '../test/mocks';

// Mock the useDriveEvents hook
vi.mock('../hooks', () => ({
    useDriveEvents: vi.fn(() => ({
        syncStatus: { is_syncing: false, connected_peers: 0, last_sync: null },
        isSyncing: false,
        startSync: vi.fn(),
        stopSync: vi.fn(),
        error: null,
    })),
}));

describe('SyncStatusBar', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockResolvedValue(false);
    });

    it('renders sync status bar with offline state', async () => {
        render(<SyncStatusBar drive={mockDrive} />);

        expect(screen.getByText('Offline')).toBeInTheDocument();
        expect(screen.getByText('Start Sync')).toBeInTheDocument();
        expect(screen.getByText('Watch')).toBeInTheDocument();
    });

    it('shows syncing state when sync is active', async () => {
        const { useDriveEvents } = await import('../hooks');
        vi.mocked(useDriveEvents).mockReturnValue({
            syncStatus: { is_syncing: true, connected_peers: 3, last_sync: null },
            isSyncing: true,
            startSync: vi.fn(),
            stopSync: vi.fn(),
            error: null,
            events: [],
            clearEvents: vi.fn(),
        });

        render(<SyncStatusBar drive={mockDrive} />);

        expect(screen.getByText('Syncing')).toBeInTheDocument();
        expect(screen.getByText('Stop Sync')).toBeInTheDocument();
        expect(screen.getByText('3')).toBeInTheDocument(); // peer count
    });

    it('calls is_watching on mount', async () => {
        render(<SyncStatusBar drive={mockDrive} />);

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('is_watching', { driveId: mockDrive.id });
        });
    });

    it('toggles file watching when watch button clicked', async () => {
        vi.mocked(invoke).mockResolvedValue(false);

        render(<SyncStatusBar drive={mockDrive} />);

        const watchButton = screen.getByText('Watch').closest('button');
        expect(watchButton).toBeInTheDocument();

        fireEvent.click(watchButton!);

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('start_watching', { driveId: mockDrive.id });
        });
    });

    it('shows watching state when file watching is active', async () => {
        vi.mocked(invoke).mockResolvedValue(true);

        render(<SyncStatusBar drive={mockDrive} />);

        await waitFor(() => {
            expect(screen.getByText('Watching')).toBeInTheDocument();
        });
    });

    it('displays error when sync fails', async () => {
        const { useDriveEvents } = await import('../hooks');
        vi.mocked(useDriveEvents).mockReturnValue({
            syncStatus: null,
            isSyncing: false,
            startSync: vi.fn(),
            stopSync: vi.fn(),
            error: 'Connection failed',
            events: [],
            clearEvents: vi.fn(),
        });

        render(<SyncStatusBar drive={mockDrive} />);

        expect(screen.getByText('Error')).toBeInTheDocument();
    });
});

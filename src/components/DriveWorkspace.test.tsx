import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DriveWorkspace } from './DriveWorkspace';
import { mockDrive, mockConflict } from '../test/mocks';

// Mock child components
vi.mock('./FileBrowser', () => ({
    FileBrowser: () => <div data-testid="file-browser">FileBrowser</div>,
}));

vi.mock('./SyncStatusBar', () => ({
    SyncStatusBar: () => <div data-testid="sync-status-bar">SyncStatusBar</div>,
}));

vi.mock('./ConflictPanel', () => ({
    ConflictPanel: () => <div data-testid="conflict-panel">ConflictPanel</div>,
}));

vi.mock('./PresencePanel', () => ({
    PresencePanel: () => <div data-testid="presence-panel">PresencePanel</div>,
}));

vi.mock('./TransferProgress', () => ({
    TransferProgress: () => <div data-testid="transfer-progress">TransferProgress</div>,
}));

// Mock useConflicts hook
vi.mock('../hooks', () => ({
    useConflicts: vi.fn(() => ({
        conflicts: [],
        conflictCount: 0,
    })),
}));

describe('DriveWorkspace', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders all workspace components', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.getByTestId('sync-status-bar')).toBeInTheDocument();
        expect(screen.getByTestId('file-browser')).toBeInTheDocument();
        expect(screen.getByTestId('presence-panel')).toBeInTheDocument();
        expect(screen.getByTestId('transfer-progress')).toBeInTheDocument();
    });

    it('hides conflict panel when no conflicts', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.queryByTestId('conflict-panel')).not.toBeInTheDocument();
    });

    it('shows conflict panel when conflicts exist', async () => {
        const { useConflicts } = await import('../hooks');
        vi.mocked(useConflicts).mockReturnValue({
            conflicts: [mockConflict],
            conflictCount: 1,
            resolveConflict: vi.fn(),
            dismissConflict: vi.fn(),
            refreshConflicts: vi.fn(),
            isLoading: false,
            error: null,
        });

        render(<DriveWorkspace drive={mockDrive} />);

        expect(screen.getByTestId('conflict-panel')).toBeInTheDocument();
    });

    it('toggles presence panel visibility', () => {
        render(<DriveWorkspace drive={mockDrive} />);

        // Initially visible
        expect(screen.getByTestId('presence-panel')).toBeInTheDocument();

        // Find and click toggle button
        const toggleButton = screen.getByTitle('Hide presence panel');
        fireEvent.click(toggleButton);

        // Panel should be hidden
        expect(screen.queryByTestId('presence-panel')).not.toBeInTheDocument();

        // Click again to show
        const showButton = screen.getByTitle('Show presence panel');
        fireEvent.click(showButton);

        // Panel should be visible again
        expect(screen.getByTestId('presence-panel')).toBeInTheDocument();
    });

    it('shows conflict badge when conflicts exist', async () => {
        const { useConflicts } = await import('../hooks');
        vi.mocked(useConflicts).mockReturnValue({
            conflicts: [mockConflict],
            conflictCount: 2,
            resolveConflict: vi.fn(),
            dismissConflict: vi.fn(),
            refreshConflicts: vi.fn(),
            isLoading: false,
            error: null,
        });

        render(<DriveWorkspace drive={mockDrive} />);

        // Should show conflict badge on toggle button
        const badge = screen.getByTitle('2 conflicts');
        expect(badge).toBeInTheDocument();
    });
});

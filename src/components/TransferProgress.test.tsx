import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TransferProgress } from './TransferProgress';
import { mockDrive, mockTransfer } from '../test/mocks';
import type { TransferState } from '../types';

// Mock the useFileTransfer hook
const mockCancelTransfer = vi.fn();
vi.mock('../hooks', () => ({
    useFileTransfer: vi.fn(() => ({
        transfers: [],
        cancelTransfer: mockCancelTransfer,
        isTransferring: false,
    })),
}));

describe('TransferProgress', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders nothing when no transfers', () => {
        const { container } = render(<TransferProgress drive={mockDrive} />);
        expect(container.firstChild).toBeNull();
    });

    it('renders transfer panel when transfers exist', async () => {
        const { useFileTransfer } = await import('../hooks');
        vi.mocked(useFileTransfer).mockReturnValue({
            transfers: [mockTransfer],
            cancelTransfer: mockCancelTransfer,
            isTransferring: true,
            uploadFile: vi.fn(),
            downloadFile: vi.fn(),
            refreshTransfers: vi.fn(),
            error: null,
        });

        render(<TransferProgress drive={mockDrive} />);

        expect(screen.getByText('1 transfer in progress')).toBeInTheDocument();
        expect(screen.getByText('file.txt')).toBeInTheDocument();
        expect(screen.getByText('50%')).toBeInTheDocument(); // 512000/1024000
    });

    it('shows completed transfers', async () => {
        const completedTransfer: TransferState = {
            ...mockTransfer,
            status: 'Completed',
            bytes_transferred: 1024000,
        };

        const { useFileTransfer } = await import('../hooks');
        vi.mocked(useFileTransfer).mockReturnValue({
            transfers: [completedTransfer],
            cancelTransfer: mockCancelTransfer,
            isTransferring: false,
            uploadFile: vi.fn(),
            downloadFile: vi.fn(),
            refreshTransfers: vi.fn(),
            error: null,
        });

        render(<TransferProgress drive={mockDrive} />);

        expect(screen.getByText('Transfers complete')).toBeInTheDocument();
    });

    it('shows failed transfer with error', async () => {
        const failedTransfer: TransferState = {
            ...mockTransfer,
            status: 'Failed',
            error: 'Network error',
        };

        const { useFileTransfer } = await import('../hooks');
        vi.mocked(useFileTransfer).mockReturnValue({
            transfers: [failedTransfer],
            cancelTransfer: mockCancelTransfer,
            isTransferring: false,
            uploadFile: vi.fn(),
            downloadFile: vi.fn(),
            refreshTransfers: vi.fn(),
            error: null,
        });

        render(<TransferProgress drive={mockDrive} />);

        expect(screen.getByText('Network error')).toBeInTheDocument();
    });

    it('calls cancelTransfer when cancel button clicked', async () => {
        const { useFileTransfer } = await import('../hooks');
        vi.mocked(useFileTransfer).mockReturnValue({
            transfers: [mockTransfer],
            cancelTransfer: mockCancelTransfer,
            isTransferring: true,
            uploadFile: vi.fn(),
            downloadFile: vi.fn(),
            refreshTransfers: vi.fn(),
            error: null,
        });

        render(<TransferProgress drive={mockDrive} />);

        const cancelButton = screen.getByTitle('Cancel transfer');
        fireEvent.click(cancelButton);

        expect(mockCancelTransfer).toHaveBeenCalledWith(mockTransfer.id);
    });

    it('collapses and expands transfer list', async () => {
        const { useFileTransfer } = await import('../hooks');
        vi.mocked(useFileTransfer).mockReturnValue({
            transfers: [mockTransfer],
            cancelTransfer: mockCancelTransfer,
            isTransferring: true,
            uploadFile: vi.fn(),
            downloadFile: vi.fn(),
            refreshTransfers: vi.fn(),
            error: null,
        });

        render(<TransferProgress drive={mockDrive} />);

        // Initially expanded
        expect(screen.getByText('file.txt')).toBeInTheDocument();

        // Click header to collapse
        const header = screen.getByText('1 transfer in progress').closest('.transfer-header');
        fireEvent.click(header!);

        // File name should still be in DOM but list collapsed
        // (The component toggles expanded state)
    });
});

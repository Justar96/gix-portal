import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
    X,
    Download,
    Loader2,
    FileText,
    Image as ImageIcon,
    Film,
    Music,
    Code,
    File,
    Maximize2,
    Minimize2,
    ChevronLeft,
    ChevronRight,
} from "lucide-react";
import type { FileEntry, FileContent, FileCategory } from "../types";
import { formatBytes, getFileCategory } from "../types";

interface FilePreviewProps {
    file: FileEntry | null;
    driveId: string;
    onClose: () => void;
    onDownload?: (file: FileEntry) => void;
    onNavigate?: (direction: "prev" | "next") => void;
    hasPrev?: boolean;
    hasNext?: boolean;
}

const PREVIEW_SIZE_LIMIT = 5 * 1024 * 1024; // 5MB max preview size
const TEXT_EXTENSIONS = ["txt", "md", "json", "js", "ts", "jsx", "tsx", "css", "scss", "html", "xml", "yaml", "yml", "toml", "rs", "py", "go", "java", "c", "cpp", "h", "sh", "bat", "sql", "csv"];
const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico"];

export function FilePreview({
    file,
    driveId,
    onClose,
    onDownload,
    onNavigate,
    hasPrev = false,
    hasNext = false,
}: FilePreviewProps) {
    const [content, setContent] = useState<FileContent | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isFullscreen, setIsFullscreen] = useState(false);

    const loadPreview = useCallback(async () => {
        if (!file || file.is_dir) return;

        // Check size limit
        if (file.size > PREVIEW_SIZE_LIMIT) {
            setError(`File too large to preview (${formatBytes(file.size)}). Maximum preview size is ${formatBytes(PREVIEW_SIZE_LIMIT)}.`);
            return;
        }

        setLoading(true);
        setError(null);
        setContent(null);

        try {
            const result = await invoke<FileContent>("read_file", {
                driveId,
                path: file.path,
            });
            setContent(result);
        } catch (e) {
            setError(`Failed to load preview: ${e}`);
        } finally {
            setLoading(false);
        }
    }, [file, driveId]);

    useEffect(() => {
        loadPreview();
    }, [loadPreview]);

    // Keyboard navigation
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                if (isFullscreen) {
                    setIsFullscreen(false);
                } else {
                    onClose();
                }
            } else if (e.key === "ArrowLeft" && hasPrev && onNavigate) {
                onNavigate("prev");
            } else if (e.key === "ArrowRight" && hasNext && onNavigate) {
                onNavigate("next");
            }
        };

        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [onClose, onNavigate, hasPrev, hasNext, isFullscreen]);

    if (!file) return null;

    const ext = file.name.split(".").pop()?.toLowerCase() || "";
    const category = getFileCategory(file.name);
    const isText = TEXT_EXTENSIONS.includes(ext);
    const isImage = IMAGE_EXTENSIONS.includes(ext);

    const renderPreviewContent = () => {
        if (loading) {
            return (
                <div className="preview-loading">
                    <Loader2 size={32} className="spinning" />
                    <span>Loading preview...</span>
                </div>
            );
        }

        if (error) {
            return (
                <div className="preview-error">
                    <File size={48} />
                    <p>{error}</p>
                    {onDownload && (
                        <button className="btn-secondary" onClick={() => onDownload(file)}>
                            <Download size={14} />
                            Download Instead
                        </button>
                    )}
                </div>
            );
        }

        if (!content) {
            return (
                <div className="preview-unsupported">
                    {getCategoryIcon(category, 48)}
                    <p>Preview not available for this file type</p>
                    {onDownload && (
                        <button className="btn-secondary" onClick={() => onDownload(file)}>
                            <Download size={14} />
                            Download File
                        </button>
                    )}
                </div>
            );
        }

        // Image preview
        if (isImage && content.content) {
            const mimeType = content.mime_type || `image/${ext === "svg" ? "svg+xml" : ext}`;
            const dataUrl = `data:${mimeType};base64,${content.content}`;
            return (
                <div className="preview-image">
                    <img src={dataUrl} alt={file.name} />
                </div>
            );
        }

        // Text preview
        if (isText && content.content) {
            try {
                const text = atob(content.content);
                return (
                    <div className="preview-text">
                        <pre>
                            <code className={`language-${ext}`}>{text}</code>
                        </pre>
                    </div>
                );
            } catch {
                return (
                    <div className="preview-error">
                        <p>Unable to decode file content</p>
                    </div>
                );
            }
        }

        // Fallback
        return (
            <div className="preview-unsupported">
                {getCategoryIcon(category, 48)}
                <p>Preview not available</p>
                <span className="preview-mime">{content.mime_type || "Unknown type"}</span>
            </div>
        );
    };

    return (
        <div className={`file-preview-overlay ${isFullscreen ? "fullscreen" : ""}`}>
            <div className="file-preview-panel">
                <div className="preview-header">
                    <div className="preview-title">
                        {getCategoryIcon(category, 16)}
                        <span className="preview-filename" title={file.path}>
                            {file.name}
                        </span>
                        <span className="preview-size">{formatBytes(file.size)}</span>
                    </div>
                    <div className="preview-actions">
                        {onDownload && (
                            <button
                                className="btn-icon"
                                onClick={() => onDownload(file)}
                                title="Download"
                            >
                                <Download size={16} />
                            </button>
                        )}
                        <button
                            className="btn-icon"
                            onClick={() => setIsFullscreen(!isFullscreen)}
                            title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
                        >
                            {isFullscreen ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
                        </button>
                        <button className="btn-icon" onClick={onClose} title="Close (Esc)">
                            <X size={16} />
                        </button>
                    </div>
                </div>

                <div className="preview-content">
                    {renderPreviewContent()}
                </div>

                {onNavigate && (hasPrev || hasNext) && (
                    <div className="preview-navigation">
                        <button
                            className="btn-nav prev"
                            onClick={() => onNavigate("prev")}
                            disabled={!hasPrev}
                            title="Previous (←)"
                        >
                            <ChevronLeft size={24} />
                        </button>
                        <button
                            className="btn-nav next"
                            onClick={() => onNavigate("next")}
                            disabled={!hasNext}
                            title="Next (→)"
                        >
                            <ChevronRight size={24} />
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}

function getCategoryIcon(category: FileCategory, size: number) {
    switch (category) {
        case "document":
            return <FileText size={size} />;
        case "image":
            return <ImageIcon size={size} />;
        case "video":
            return <Film size={size} />;
        case "audio":
            return <Music size={size} />;
        case "code":
            return <Code size={size} />;
        default:
            return <File size={size} />;
    }
}

export default FilePreview;

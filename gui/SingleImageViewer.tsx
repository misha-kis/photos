import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SingleImageViewerProps {
  imageIndex: number;
  onClose: () => void;
}

export function SingleImageViewer({
  imageIndex,
  onClose,
}: SingleImageViewerProps) {
  const [imageSrc, setImageSrc] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadImage = useCallback(async () => {
    setLoading(true);
    setError(null);
    setImageSrc(null);
    try {
      // You'll need to implement this backend function
      // It should return the full-size image as a base64 string
      const base64 = await invoke<string>("load_image", { index: imageIndex });
      setImageSrc(`data:image/jpeg;base64,${base64}`);
    } catch (e: any) {
      console.error(`Failed to load image ${imageIndex}:`, e);
      setError(`Failed to load image: ${e.message || String(e)}`);
    } finally {
      setLoading(false);
    }
  }, [imageIndex]);

  useEffect(() => {
    loadImage();
  }, [loadImage]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "100%",
        height: "100%",
        backgroundColor: "rgba(0, 0, 0, 0.9)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
        color: "white",
      }}
    >
      <button
        onClick={onClose}
        style={{
          position: "absolute",
          top: 20,
          right: 20,
          background: "none",
          border: "none",
          color: "white",
          fontSize: "2em",
          cursor: "pointer",
          zIndex: 1001,
        }}
      >
        &times;
      </button>

      {loading && <div>Loading full image...</div>}
      {error && <div style={{ color: "red" }}>Error: {error}</div>}
      {imageSrc && !loading && (
        <img
          src={imageSrc}
          alt={`Full size image ${imageIndex}`}
          style={{
            maxWidth: "90%",
            maxHeight: "90%",
            objectFit: "contain",
          }}
        />
      )}
    </div>
  );
}

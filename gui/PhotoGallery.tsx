import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useRef, useCallback } from "react";

const THUMB_SIZE = 128;
const GRID_GAP = 8;

interface PhotoGalleryProps {
  itemsCount: number;
  onImageClick: (index: number) => void;
}

interface PhotoData {
  src: string | null;
  loading: boolean;
}

export default function PhotoGallery({
  itemsCount,
  onImageClick,
}: PhotoGalleryProps) {
  const [photos, setPhotos] = useState<PhotoData[]>(
    Array.from({ length: itemsCount }, () => ({ src: null, loading: false })),
  );
  const containerRef = useRef<HTMLDivElement>(null);
  const [columnCount, setColumnCount] = useState(5);

  const loadThumbnail = useCallback(async (index: number) => {
    setPhotos((prev) => {
      if (prev[index]?.loading || prev[index]?.src) return prev;
      const copy = [...prev];
      copy[index] = { ...copy[index], loading: true };
      return copy;
    });

    try {
      const base64 = await invoke<string>("load_thumbnail", { index });
      const src = `data:image/jpeg;base64,${base64}`;
      setPhotos((prev) => {
        const copy = [...prev];
        copy[index] = { src, loading: false };
        return copy;
      });
    } catch (e) {
      console.error(`Failed to load thumbnail ${index}:`, e);
      setPhotos((prev) => {
        const copy = [...prev];
        copy[index] = { src: null, loading: false };
        return copy;
      });
    }
  }, []);

  useEffect(() => {
    const handleResize = () => {
      if (!containerRef.current) return;
      const availableWidth = containerRef.current.offsetWidth;
      const columnWidth = THUMB_SIZE + GRID_GAP;
      setColumnCount(
        Math.max(1, Math.floor((availableWidth + GRID_GAP) / columnWidth)),
      );
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100vh",
        overflowY: "auto",
        padding: GRID_GAP,
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: `repeat(auto-fit, minmax(${THUMB_SIZE}px, 1fr))`,
          gap: GRID_GAP,
          justifyContent: "center",
        }}
      >
        {photos.map((photo, index) => (
          <LazyPhoto
            key={index}
            index={index}
            src={photo.src}
            loading={photo.loading}
            loadThumbnail={loadThumbnail}
            onClick={() => onImageClick(index)}
          />
        ))}
      </div>
    </div>
  );
}

interface LazyPhotoProps {
  index: number;
  src: string | null;
  loading: boolean;
  loadThumbnail: (index: number) => void;
  onClick: () => void;
}

function LazyPhoto({
  index,
  src,
  loading,
  loadThumbnail,
  onClick,
}: LazyPhotoProps) {
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          loadThumbnail(index);
          observer.disconnect();
        }
      },
      { threshold: 0.1 },
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, [index, loadThumbnail]);

  return (
    <div
      ref={ref}
      onClick={onClick}
      style={{
        width: THUMB_SIZE,
        height: THUMB_SIZE,
        borderRadius: 8,
        overflow: "hidden",
        background: loading ? "#ddd" : src ? "transparent" : "#eee",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        cursor: "pointer",
      }}
    >
      {src && (
        <img
          src={src}
          alt={`Photo ${index}`}
          draggable={false}
          style={{
            width: "100%",
            height: "100%",
            objectFit: "cover",
          }}
        />
      )}
    </div>
  );
}

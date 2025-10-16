import { VirtuosoGrid } from "react-virtuoso";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useCallback, useRef, forwardRef } from "react";

const THUMB_SIZE = 128;
const GRID_GAP = 8;
const PRELOAD_MARGIN = 2;

function useThumbnails() {
  const cacheRef = useRef<Map<number, string | "loading" | null>>(new Map());
  const [, forceUpdate] = useState(0);

  const loadThumbnail = useCallback(async (index: number) => {
    const cache = cacheRef.current;

    if (cache.has(index) && cache.get(index) !== null) {
      return;
    }

    cache.set(index, "loading");
    forceUpdate((x) => x + 1);

    try {
      const base64 = await invoke<string>("load_thumbnail", { index });
      cache.set(index, `data:image/jpeg;base64,${base64}`);
    } catch (e: any) {
      console.error(`Failed to load thumbnail ${index}:`, e);
      console.error("Error details:", e.message || e);
      cache.set(index, null); // Mark as failed to load
    }

    forceUpdate((x) => x + 1);
  }, []);

  const getThumbnail = useCallback((index: number) => {
    const value = cacheRef.current.get(index);
    return value === "loading" ? null : (value ?? null);
  }, []);

  useEffect(() => {
    return () => {
      cacheRef.current.forEach((url) => {
        if (url && typeof url === "string" && url.startsWith("blob:")) {
          URL.revokeObjectURL(url);
        }
      });
    };
  }, []);

  return { getThumbnail, loadThumbnail };
}

function PhotoItem({
  index,
  getThumbnail,
  loadThumbnail,
}: {
  index: number;
  getThumbnail: (index: number) => string | null;
  loadThumbnail: (index: number) => void;
}) {
  const src = getThumbnail(index);

  const isCurrentlyLoading = src === null;

  useEffect(() => {
    if (isCurrentlyLoading) {
      loadThumbnail(index);
    }
  }, [index, isCurrentlyLoading, loadThumbnail]);

  return (
    <div
      style={{
        width: THUMB_SIZE,
        height: THUMB_SIZE,
        borderRadius: 8,
        overflow: "hidden",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: isCurrentlyLoading ? "#ddd" : src ? "transparent" : "#eee",
      }}
    >
      {src && (
        <img
          src={src}
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

export default function PhotoGallery({
  itemsCount,
  columnCount,
}: {
  itemsCount: number;
  columnCount: number;
}) {
  const { getThumbnail, loadThumbnail } = useThumbnails();

  return (
    <VirtuosoGrid
      totalCount={itemsCount}
      overscan={PRELOAD_MARGIN * columnCount}
      style={{
        height: "100vh",
        width: "100%",
      }}
      itemContent={(index) => (
        <PhotoItem
          index={index}
          getThumbnail={getThumbnail}
          loadThumbnail={loadThumbnail}
        />
      )}
      components={{
        List: forwardRef<HTMLDivElement, any>(({ style, children }, ref) => (
          <div
            ref={ref}
            style={{
              display: "grid",
              gridTemplateColumns: `repeat(${columnCount}, ${THUMB_SIZE}px)`,
              justifyContent: "center",
              gap: GRID_GAP,
              ...style,
            }}
          >
            {children}
          </div>
        )),
      }}
    />
  );
}

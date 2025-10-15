import { VirtuosoGrid } from "react-virtuoso";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useCallback, useRef, forwardRef } from "react";

const THUMB_SIZE = 120;
const GRID_GAP = 8;
const PRELOAD_MARGIN = 2;

function useThumbnails() {
  // Keep cache in a ref to prevent re-renders from causing re-loads
  const cacheRef = useRef<Map<number, string | null>>(new Map());
  const [, forceUpdate] = useState(0); // For manual re-render

  const loadThumbnail = useCallback(async (index: number) => {
    const cache = cacheRef.current;

    if (cache.has(index)) return; // Already loaded or failed

    // Mark as "loading"
    cache.set(index, null);

    try {
      const base64 = await invoke<string>("load_thumbnail", { index });
      cache.set(index, `data:image/jpeg;base64,${base64}`);
    } catch (e) {
      console.error(`Failed to load thumbnail ${index}`, e);
      cache.set(index, null);
    }

    // Trigger re-render of items
    forceUpdate((x) => x + 1);
  }, []);

  const getThumbnail = useCallback((index: number) => {
    return cacheRef.current.get(index) ?? null;
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

  useEffect(() => {
    if (src === null) {
      // Either not loaded or failed previously
      loadThumbnail(index);
    }
  }, [index, src, loadThumbnail]);

  return (
    <div
      style={{
        width: THUMB_SIZE,
        height: THUMB_SIZE,
        margin: GRID_GAP / 2,
        borderRadius: 8,
        overflow: "hidden",
        background: src ? "transparent" : "#eee",
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
      overscan={PRELOAD_MARGIN * THUMB_SIZE}
      style={{
        height: "100vh",
        width: "100%",
      }}
      itemContent={(index) => (
        <PhotoItem
          key={index}
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

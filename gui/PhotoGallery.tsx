import { VirtuosoGrid } from "react-virtuoso";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useCallback, useRef, forwardRef } from "react";

const THUMB_SIZE = 120;
const GRID_GAP = 8;
const PRELOAD_MARGIN = 2;

function useThumbnails() {
  const cacheRef = useRef<Map<number, string | null>>(new Map());
  const [, forceUpdate] = useState(0);

  const loadThumbnail = useCallback(async (index: number) => {
    const cache = cacheRef.current;

    if (cache.has(index)) {
      if (cache.get(index) === "loading") {
        return;
      }
      if (cache.get(index) !== null) {
        return;
      }
    }

    cache.set(index, "loading");

    try {
      // const bytes = await invoke<Uint8Array>("load_thumbnail", { index });
      // const blob = new Blob([bytes], { type: "image/webp" });
      // const url = URL.createObjectURL(blob);
      // cache.set(index, url);
      const base64 = await invoke<string>("load_thumbnail", { index });
      cache.set(index, `data:image/jpeg;base64,${base64}`);
    } catch (e: any) {
      console.error(`Failed to load thumbnail ${index}:`, e);
      console.error("Error details:", e.message || e);
      cache.set(index, null);
    }

    forceUpdate((x) => x + 1);
  }, []);

  const getThumbnail = useCallback((index: number) => {
    const value = cacheRef.current.get(index);
    return value === "loading" ? null : (value ?? null);
  }, []);

  // Cleanup Object URLs when component unmounts
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
  const isLoading =
    useRef<Map<number, string | null>>(new Map()).current.get(index) ===
    "loading";

  useEffect(() => {
    if (src === null && !isLoading) {
      loadThumbnail(index);
    }
  }, [index, src, loadThumbnail, isLoading]);

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
        background: isLoading ? "#ddd" : src ? "transparent" : "#eee",
      }}
    >
      {src && src !== "loading" && (
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

<script lang="ts">
    import { onMount } from "svelte";
    import { convertFileSrc, invoke } from "@tauri-apps/api/core";
    import Image from "./Image.svelte";

    type Item = {
        id: number;
        title: string;
    };

    let container: HTMLDivElement;

    type Props = {
        getImageIds?: () => Promise<string[]>;
        initialItems?: string[];
        preview?: boolean;
    };

    let { getImageIds, initialItems = [], preview = false }: Props = $props();

    // Grid configuration
    const rowHeight = 120;
    const columnWidth = 180;
    const gap = 16;
    const overscan = 3;

    let containerHeight = $state(0);
    let scrollTop = $state(0);
    let fullscreenItemId = $state<number | null>(null);
    let fullscreenSrc = $state<string | null>(null);
    let columnCount = $state(1);

    // let imageIds = $state([]);
    let items: string[] = $state([]);

    let rowSize = $derived(rowHeight + gap);
    let rowCount = $derived(Math.ceil(items.length / columnCount));
    let totalHeight = $derived(Math.max(0, rowCount * rowSize - gap));

    let firstRow = $derived(
        Math.max(0, Math.floor(scrollTop / rowSize) - overscan),
    );
    let visibleRowCount = $derived(
        Math.ceil(containerHeight / rowSize) + overscan * 2,
    );

    let lastRow = $derived(Math.min(rowCount, firstRow + visibleRowCount));
    let startIndex = $derived(firstRow * columnCount);
    let endIndex = $derived(Math.min(items.length, lastRow * columnCount));
    let visibleItems = $derived(items.slice(startIndex, endIndex));
    $inspect(
        scrollTop,
        firstRow,
        startIndex,
        endIndex,
        visibleItems.length,
        visibleItems[0],
    );
    // $inspect(visibleItems);

    function updateColumns() {
        if (!container) return;

        columnCount = Math.max(
            1,
            Math.floor((container.clientWidth + gap) / (columnWidth + gap)),
        );
    }

    function handleScroll() {
        scrollTop = container.scrollTop;
    }

    function updateContainerHeight() {
        if (container) {
            containerHeight = container.clientHeight;
        }
    }

    async function openImage(imageItemId: number) {
        const imageId = items[imageItemId];
        const path = preview
            ? imageId
            : await invoke<string>("get_original_path", {
                  imageId,
              });

        fullscreenSrc = convertFileSrc(path);
        fullscreenItemId = imageItemId;
    }

    function closeFullscreen() {
        fullscreenSrc = null;
        fullscreenItemId = null;
    }

    function handleKeydown(event: KeyboardEvent) {
        if (fullscreenItemId === null) return;
        if (event.key === "Escape") {
            closeFullscreen();
        }
        if (event.key === "ArrowLeft" && fullscreenItemId! > 0) {
            openImage(fullscreenItemId! - 1);
        }
        if (
            event.key === "ArrowRight" &&
            fullscreenItemId! < items.length - 1
        ) {
            openImage(fullscreenItemId! + 1);
        }
    }

    async function updateImageIds() {
        if (initialItems.length) {
            items = initialItems;
            return;
        }
        if (getImageIds) {
            items = await getImageIds();
        }
    }

    onMount(() => {
        updateColumns();
        updateContainerHeight();
        (async () => {
            await updateImageIds();
        })();

        const observer = new ResizeObserver(() => {
            updateColumns();
            updateContainerHeight();
        });
        observer.observe(container);

        return () => observer.disconnect();
    });
</script>

<svelte:window onkeydown={handleKeydown} />

<div
    bind:this={container}
    class="min-h-0 min-w-0 flex-1 overflow-y-auto"
    onscroll={handleScroll}
>
    <!-- Fake full-height content -->
    <div class="relative w-full" style={`height: ${totalHeight}px`}>
        <!-- Only visible rows -->
        <div
            class="absolute left-0 right-0 grid"
            style="
        grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
        gap: 16px;
        "
            style:transform={`translateY(${firstRow * rowSize}px)`}
        >
            {#each visibleItems as item, itemId (item)}
                {@const absoluteItemId = startIndex + itemId}
                <button
                    type="button"
                    class="h-[120px] cursor-zoom-in overflow-hidden rounded-lg border bg-white p-4 text-left shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                    aria-label={`Open image ${item}`}
                    onclick={() => openImage(absoluteItemId)}
                >
                    <Image {item} {preview} />
                </button>
            {/each}
        </div>
    </div>
</div>

{#if fullscreenSrc}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/90 p-8"
        role="presentation"
        tabindex="-1"
        onclick={(event) => {
            if (event.target === event.currentTarget) closeFullscreen();
        }}
    >
        <div
            class="relative flex max-h-full max-w-full items-center justify-center"
            role="dialog"
            aria-modal="true"
            aria-label="Fullscreen image"
            tabindex="-1"
        >
            <img
                src={fullscreenSrc}
                alt="Fullscreen"
                class="max-h-[calc(100vh-4rem)] max-w-[calc(100vw-4rem)] object-contain"
            />
            <button
                type="button"
                class="absolute right-3 top-3 rounded-full bg-black/60 px-3 py-1 text-xl text-white"
                aria-label="Close fullscreen image"
                onclick={closeFullscreen}
            >
                ×
            </button>
        </div>
    </div>
{/if}

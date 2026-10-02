<script lang="ts">
    import { onMount } from "svelte";
    import { convertFileSrc } from "@tauri-apps/api/core";
    import Image from "./Image.svelte";
    import type { Photo } from "$lib/types";

    let container: HTMLDivElement;

    type Props = {
        getPhotos?: () => Promise<Photo[]>;
        initialItems?: string[];
        onItemClick?: (item: Photo, index: number) => void;
        showFullscreen?: boolean;
    };

    let {
        getPhotos,
        initialItems = [],
        onItemClick,
        showFullscreen = true,
    }: Props = $props();

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

    let items: Photo[] = $state([]);

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

    function openImage(imageItemId: number) {
        fullscreenSrc = convertFileSrc(items[imageItemId].originalPath);
        fullscreenItemId = imageItemId;
    }

    function closeFullscreen() {
        fullscreenSrc = null;
        fullscreenItemId = null;
    }

    function handleKeydown(event: KeyboardEvent) {
        const itemId = fullscreenItemId;
        if (itemId === null) return;
        if (event.key === "Escape") {
            closeFullscreen();
        }
        if (event.key === "ArrowLeft" && itemId > 0) {
            openImage(itemId - 1);
        }
        if (event.key === "ArrowRight" && itemId < items.length - 1) {
            openImage(itemId + 1);
        }
    }

    async function updateImages() {
        if (initialItems.length) {
            items = initialItems.map((path) => ({
                id: path,
                thumbnailPath: path,
                originalPath: path,
            }));
            return;
        }
        if (getPhotos) {
            items = await getPhotos();
        }
    }

    onMount(() => {
        updateColumns();
        updateContainerHeight();
        void updateImages();

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
            style:grid-template-columns={`repeat(${columnCount}, minmax(0, 1fr))`}
            style:gap={`${gap}px`}
            style:transform={`translateY(${firstRow * rowSize}px)`}
        >
            {#each visibleItems as item, itemId (item.id)}
                {@const absoluteItemId = startIndex + itemId}
                <button
                    type="button"
                    class="relative flex h-32 flex-col overflow-hidden bg-transparent p-0 text-left focus:outline-none focus:ring-2 focus:ring-blue-500"
                    class:cursor-zoom-in={showFullscreen}
                    aria-label={`Open image ${item.id}`}
                    onclick={() =>
                        onItemClick
                            ? onItemClick(item, absoluteItemId)
                            : showFullscreen && openImage(absoluteItemId)}
                >
                    <div class="min-h-0 flex-1">
                        <Image src={item.thumbnailPath} alt="" />
                    </div>
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
                class="button-bordered absolute right-3 top-3 rounded-full bg-black/60 px-3 py-1 text-xl text-white"
                aria-label="Close fullscreen image"
                onclick={closeFullscreen}
            >
                ×
            </button>
        </div>
    </div>
{/if}

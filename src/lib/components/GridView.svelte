<script lang="ts">
    import { onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Image from "./Image.svelte";

    type Item = {
        id: number;
        title: string;
    };

    let container: HTMLDivElement;

    let { getImageIds } = $props();

    // Grid configuration
    const rowHeight = 120;
    const columnWidth = 180;
    const gap = 16;
    const overscan = 3;

    let containerHeight = 600;
    let scrollTop = 0;
    let columnCount = 1;

    // let imageIds = $state([]);
    let items = $state([]);

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

    async function updateImageIds() {
        items = await getImageIds();
        console.log(items);
        console.log(items.length);
    }

    onMount(() => {
        updateColumns();
        (async () => {
            await updateImageIds();
        })();

        const observer = new ResizeObserver(updateColumns);
        observer.observe(container);

        return () => observer.disconnect();
    });
</script>

<div
    bind:this={container}
    class="h-[600px] overflow-y-auto"
    on:scroll={handleScroll}
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
            {#each visibleItems as item}
                <div class="h-[120px] rounded-lg border bg-white p-4 shadow-sm">
                    <!-- <Image {item} /> -->
                    {item}
                </div>
            {/each}
        </div>
    </div>
</div>

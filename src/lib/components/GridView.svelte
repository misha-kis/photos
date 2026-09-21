<script lang="ts">
    import { onMount } from "svelte";
    import Image from "./Image.svelte";

    type Item = {
        id: number;
        title: string;
    };

    let container: HTMLDivElement;

    export let items: Item[] = [];

    // Grid configuration
    const rowHeight = 120;
    const columnWidth = 180;
    const gap = 16;
    const overscan = 3;

    let containerHeight = 600;
    let scrollTop = 0;
    let columnCount = 1;

    $: rowSize = rowHeight + gap;
    $: rowCount = Math.ceil(items.length / columnCount);
    $: totalHeight = Math.max(0, rowCount * rowSize - gap);

    $: firstRow = Math.max(0, Math.floor(scrollTop / rowSize) - overscan);

    $: visibleRowCount = Math.ceil(containerHeight / rowSize) + overscan * 2;

    $: lastRow = Math.min(rowCount, firstRow + visibleRowCount);

    $: startIndex = firstRow * columnCount;
    $: endIndex = Math.min(items.length, lastRow * columnCount);

    $: visibleItems = items.slice(startIndex, endIndex);

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

    onMount(() => {
        updateColumns();

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
            {#each visibleItems as item (item.id)}
                <div class="h-[120px] rounded-lg border bg-white p-4 shadow-sm">
                    <Image {item} />
                </div>
            {/each}
        </div>
    </div>
</div>

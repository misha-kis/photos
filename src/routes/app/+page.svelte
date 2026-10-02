<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import SideBar from "$lib/components/SideBar.svelte";
    import GridView from "$lib/components/GridView.svelte";
    import ImportView from "$lib/components/ImportView.svelte";
    import PeopleView from "$lib/components/PeopleView.svelte";
    import type { Photo } from "$lib/types";

    let showImport = $state(false);
    let view = $state<"gallery" | "people">("gallery");

    async function getPhotos(): Promise<Photo[]> {
        const records = await invoke<[string, string, string][]>(
            "get_image_ids_with_paths",
        );
        return records.map(([id, thumbnailPath, originalPath]) => ({
            id,
            thumbnailPath,
            originalPath,
        }));
    }

    async function navigate(viewName: "gallery" | "people") {
        view = viewName;
    }
</script>

<div class="flex h-screen overflow-hidden">
    <SideBar active={view} onnavigate={navigate} />
    <div
        class="flex min-h-0 min-w-0 flex-1 flex-col bg-zinc-900 p-8 text-zinc-100"
    >
        {#if showImport}
            <ImportView
                oncancel={() => (showImport = false)}
                oncomplete={() => (showImport = false)}
            />
        {:else if view === "people"}
            <PeopleView />
        {:else}
            <div class="mb-4 flex items-center justify-between">
                <p>Library</p>
                <button
                    type="button"
                    class="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
                    onclick={() => (showImport = true)}>Import photos</button
                >
            </div>
            <GridView {getPhotos} />
        {/if}
    </div>
</div>

<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import { open } from "@tauri-apps/plugin-dialog";
    import Image from "$lib/components/Image.svelte";
    import SideBar from "$lib/components/SideBar.svelte";
    import GridView from "$lib/components/GridView.svelte";
    import ImportView from "$lib/components/ImportView.svelte";
    import type { Photo } from "$lib/types";

    let name = $state("");
    let greetMsg = $state("");
    let selectedDirectory = $state("");
    let showImport = $state(false);
    // let image_ids = $state();

    async function setGallery(gallery: string | null) {
        await invoke("set_gallery", { gallery }).then(() => {
            console.log("gallery opened");
        });
    }

    const galleryChangedListener = await listen<string>(
        "gallery-changed",
        (evt) => {
            selectedDirectory = evt.payload;
        },
    );

    async function selectDirectory() {
        const selected = await open({
            directory: true,
            multiple: false,
            title: "Select the Library directory",
        });

        if (typeof selected === "string") {
            await setGallery(selected);
            selectedDirectory = selected;
        }
    }

    // const items: { id: number; title: string }[] = [];
    // for (let i = 0; i < 100; i++) {
    //     items.push({ id: i, title: `Photo ${i + 1}` });
    // }

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
</script>

{#if selectedDirectory}
    <div class="flex h-screen overflow-hidden">
        <SideBar />
        <div class="flex min-h-0 min-w-0 flex-1 flex-col bg-gray-100 p-8">
            {#if showImport}
                <ImportView
                    oncancel={() => (showImport = false)}
                    oncomplete={() => (showImport = false)}
                />
            {:else}
                <div class="mb-4 flex items-center justify-between">
                    <p>Selected: {selectedDirectory}</p>
                    <button
                        type="button"
                        class="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
                        onclick={() => (showImport = true)}
                    >
                        Import photos
                    </button>
                </div>
                <GridView {getPhotos} />
            {/if}
        </div>
    </div>
{:else}
    <button onclick={selectDirectory}>Open/Create a Library</button>
    <p>No directory selected.</p>
{/if}

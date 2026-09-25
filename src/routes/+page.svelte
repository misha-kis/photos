<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import { open } from "@tauri-apps/plugin-dialog";
    import Image from "$lib/components/Image.svelte";
    import SideBar from "$lib/components/SideBar.svelte";
    import GridView from "$lib/components/GridView.svelte";

    let name = $state("");
    let greetMsg = $state("");
    let selectedDirectory = $state("");
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

    async function getImageIds(): Promise<string[]> {
        return await invoke("get_image_ids");
    }
</script>

{#if selectedDirectory}
    <div class="flex h-screen overflow-hidden">
        <SideBar />
        <div class="min-w-0 flex-1 bg-gray-100 p-8">
            <p>Selected: {selectedDirectory}</p>
            <GridView {getImageIds} />
        </div>
    </div>
{:else}
    <button onclick={selectDirectory}>Open/Create a Library</button>
    <p>No directory selected.</p>
{/if}

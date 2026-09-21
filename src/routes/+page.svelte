<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { open } from "@tauri-apps/plugin-dialog";
    import Image from "$lib/components/Image.svelte";
    import SideBar from "$lib/components/SideBar.svelte";
    import GridView from "$lib/components/GridView.svelte";

    let name = $state("");
    let greetMsg = $state("");

    async function greet(event: Event) {
        event.preventDefault();
        // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
        greetMsg = await invoke("greet", { name });
    }

    let selectedDirectory = $state("");

    async function selectDirectory() {
        const selected = await open({
            directory: true,
            multiple: false,
            title: "Select the Library directory",
        });

        if (typeof selected === "string") {
            selectedDirectory = selected;
        }
    }

    const items: { id: number; title: string }[] = [];
    for (let i = 0; i < 100; i++) {
        items.push({ id: i, title: `Photo ${i + 1}` });
    }
</script>

{#if selectedDirectory}
    <div class="flex h-screen overflow-hidden">
        <SideBar />
        <div class="min-w-0 flex-1 bg-gray-100 p-8">
            <p>Selected: {selectedDirectory}</p>
            <GridView {items} />
        </div>
    </div>
{:else}
    <button onclick={selectDirectory}>Open/Create a Library</button>
    <p>No directory selected.</p>
{/if}

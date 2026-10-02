<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { open } from "@tauri-apps/plugin-dialog";
    import { goto } from "$app/navigation";

    async function selectDirectory() {
        const selected = await open({
            directory: true,
            multiple: false,
            title: "Select the Library directory",
        });

        if (typeof selected === "string") {
            await invoke("set_gallery", { gallery: selected });
            await goto("/app");
        }
    }
</script>

<div
    class="flex min-h-screen flex-col items-center justify-center gap-4 bg-zinc-900 p-8 text-zinc-100"
>
    <button class="button-bordered px-4 py-2" onclick={selectDirectory}
        >Open/Create a Library</button
    >
    <p class="text-zinc-400">No directory selected.</p>
</div>

<script lang="ts">
    import { onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import { open } from "@tauri-apps/plugin-dialog";
    import GridView from "./GridView.svelte";

    type Props = {
        oncancel?: () => void;
        oncomplete?: () => void;
    };

    let { oncancel, oncomplete }: Props = $props();

    let directory = $state<string | null>(null);
    let items = $state<string[]>([]);
    let loading = $state(true);
    let importing = $state(false);
    let completed = $state(0);
    let total = $state(0);
    let error = $state<string | null>(null);

    let progress = $derived(
        total > 0 ? Math.min(100, (completed / total) * 100) : 0,
    );

    async function chooseDirectory() {
        const selected = await open({
            directory: true,
            multiple: false,
            title: "Select images to import",
        });

        if (typeof selected !== "string") {
            oncancel?.();
            return;
        }

        directory = selected;
        loading = true;
        error = null;

        try {
            items = await invoke<string[]>("discover_images_for_import", {
                directory: selected,
            });
        } catch (cause) {
            error = String(cause);
        } finally {
            loading = false;
        }
    }

    async function applyImport() {
        if (!items.length || importing) return;

        importing = true;
        completed = 0;
        total = items.length;
        error = null;

        try {
            await invoke("import_images", { imagePaths: items });
            oncomplete?.();
        } catch (cause) {
            error = String(cause);
            importing = false;
        }
    }

    function cancelImport() {
        if (!importing) oncancel?.();
    }

    onMount(() => {
        let unlisten: (() => void) | undefined;
        let active = true;

        (async () => {
            unlisten = await listen<[number, number]>(
                "import-progress",
                (event) => {
                    if (!active) return;
                    [completed, total] = event.payload;
                },
            );
        })();

        chooseDirectory();

        return () => {
            active = false;
            unlisten?.();
        };
    });
</script>

<section class="flex min-h-0 flex-1 flex-col gap-4" aria-label="Import photos">
    <header class="flex items-center justify-between gap-4">
        <div>
            <h1 class="text-2xl font-semibold">Import photos</h1>
            {#if directory}
                <p class="truncate text-sm text-zinc-400">{directory}</p>
            {/if}
        </div>
        <button
            type="button"
            class="rounded border px-3 py-2 text-sm hover:bg-zinc-800 disabled:opacity-50"
            onclick={chooseDirectory}
            disabled={importing}
        >
            Choose another folder
        </button>
    </header>

    {#if loading}
        <div class="flex flex-1 items-center justify-center text-zinc-400">
            Looking for images…
        </div>
    {:else if error}
        <div
            class="rounded border border-red-200 bg-red-50 p-4 text-red-700"
            role="alert"
        >
            {error}
        </div>
    {:else if !items.length}
        <div class="flex flex-1 items-center justify-center text-zinc-400">
            No importable images were found.
        </div>
    {:else}
        <GridView initialItems={items} />
    {/if}

    {#if importing}
        <div class="space-y-2" aria-live="polite">
            <div class="flex justify-between text-sm">
                <span>Importing photos…</span>
                <span>{completed} / {total}</span>
            </div>
            <progress class="h-2 w-full" max="100" value={progress}>
                {progress}%
            </progress>
        </div>
    {/if}

    <footer class="flex justify-end gap-3 border-t pt-4">
        <button
            type="button"
            class="rounded border px-4 py-2 hover:bg-zinc-800 disabled:opacity-50"
            onclick={cancelImport}
            disabled={importing}
        >
            Cancel
        </button>
        <button
            type="button"
            class="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={applyImport}
            disabled={loading || importing || !items.length}
        >
            {importing
                ? "Importing…"
                : `Import ${items.length} ${items.length === 1 ? "photo" : "photos"}`}
        </button>
    </footer>
</section>

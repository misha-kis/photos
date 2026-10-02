<script lang="ts">
    type Props = {
        active: "gallery" | "people";
        onnavigate: (view: "gallery" | "people") => void;
    };
    let { active, onnavigate }: Props = $props();
</script>

<aside class="w-40 shrink-0 bg-zinc-950 text-zinc-100">
    <div class="h-full overflow-y-auto p-5">
        <h2 class="mb-8 text-2xl font-bold">Gallery</h2>
        <nav class="space-y-2">
            <button
                class="block w-full rounded px-4 py-2 text-left hover:bg-zinc-800"
                class:bg-zinc-800={active === "gallery"}
                onclick={() => onnavigate("gallery")}>Gallery</button
            >
            <button
                class="block w-full rounded px-4 py-2 text-left hover:bg-zinc-800"
                class:bg-zinc-800={active === "people"}
                onclick={() => onnavigate("people")}>People</button
            >
        </nav>
    </div>
</aside>

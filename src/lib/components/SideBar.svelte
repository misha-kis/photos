<script lang="ts">
    type Props = {
        active: "gallery" | "people";
        onnavigate: (view: "gallery" | "people") => void;
    };
    let { active, onnavigate }: Props = $props();
</script>

<aside class="w-64 shrink-0 bg-gray-900 text-white">
    <div class="h-full overflow-y-auto p-5">
        <h2 class="mb-8 text-2xl font-bold">Gallery</h2>
        <nav class="space-y-2">
            <button
                class="block w-full rounded px-4 py-2 text-left hover:bg-gray-700"
                class:bg-gray-700={active === "gallery"}
                onclick={() => onnavigate("gallery")}>Gallery</button
            >
            <button
                class="block w-full rounded px-4 py-2 text-left hover:bg-gray-700"
                class:bg-gray-700={active === "people"}
                onclick={() => onnavigate("people")}>People</button
            >
        </nav>
    </div>
</aside>

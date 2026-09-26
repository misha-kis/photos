<script lang="ts">
    import { convertFileSrc, invoke } from "@tauri-apps/api/core";

    let { item, preview = false }: { item: string; preview?: boolean } =
        $props();

    let imageSrc = $state("");

    async function getThumbnailPath() {
        if (preview) {
            imageSrc = convertFileSrc(item);
            return;
        }

        const path = await invoke<string>("get_thumbnail_path", {
            imageId: item,
        });

        imageSrc = convertFileSrc(path);
    }

    getThumbnailPath();
</script>

<img src={imageSrc} alt={item} class="h-full w-full object-cover" />

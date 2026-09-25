<script lang="ts">
    import { convertFileSrc } from "@tauri-apps/api/core";
    import { invoke } from "@tauri-apps/api/core";

    let item = $props();

    let imageSrc = $state("");

    async function getThumbnailPath() {
        const path = await invoke<string>("get_thumbnail_path", {
            imageId: item.item,
        });
        imageSrc = convertFileSrc(path);
    }

    getThumbnailPath();
</script>

<img src={imageSrc} alt={item} />

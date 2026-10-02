<script lang="ts">
    import { convertFileSrc } from "@tauri-apps/api/core";

    let { src, alt }: { src: string; alt: string } = $props();

    let imageSrc = $derived(
        src.startsWith("blob:") ||
            src.startsWith("data:") ||
            src.startsWith("http")
            ? src
            : convertFileSrc(src),
    );
</script>

<img src={imageSrc} {alt} class="h-full w-full object-cover" />

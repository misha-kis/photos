<script lang="ts">
    import { convertFileSrc } from "@tauri-apps/api/core";

    let { src, alt }: { src: string; alt: string } = $props();

    let imageSrc = $derived(convertFileSrc(src));
</script>

<img src={imageSrc} {alt} class="h-full w-full object-cover" />

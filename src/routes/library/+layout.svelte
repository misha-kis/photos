<script lang="ts">
    import SideBar from "$lib/components/SideBar.svelte";
    let { children } = $props();
</script>

<div class="flex h-screen overflow-hidden">
    <SideBar />
    <div class="min-w-0 flex-1 overflow-y-auto bg-zinc-900 p-8 text-zinc-100">
        {@render children()}
    </div>
</div>

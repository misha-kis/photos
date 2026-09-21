<script lang="ts">
    // import SideBar from "$lib/components/SideBar.svelte";
    import "./layout.css";
    let { children } = $props();
</script>

<!-- <div class="flex h-screen overflow-hidden">
    <SideBar /> -->
<!-- <main class="min-w-0 flex-1 overflow-y-auto bg-gray-100 p-8"> -->
<main>
    {@render children()}
</main>
<!-- </main> -->
<!-- </div> -->

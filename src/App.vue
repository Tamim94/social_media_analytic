<script setup lang="ts">
import Dashboard from './views/Dashboard.vue'
</script>

<template>
  <div class="min-h-screen bg-slate-950 text-slate-100">
    <header class="border-b border-slate-800 px-6 py-4">
      <h1 class="text-lg font-semibold tracking-tight">
        wikistream<span class="ml-2 text-sm font-normal text-slate-400">live Wikimedia edit analytics</span>
      </h1>
    </header>
    <main class="mx-auto max-w-6xl px-6 py-8">
      <Dashboard />
    </main>
  </div>
</template>

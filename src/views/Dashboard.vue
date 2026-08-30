<script setup lang="ts">
// Placeholder shell — the real charts are scoped in ticket 09 and built in
// ticket 10. The anon read path (RLS on the rollup views) lands with them.
</script>

<template>
  <section>
    <h2 class="text-2xl font-bold">Every edit, on every Wikimedia wiki, as it happens.</h2>
    <p class="mt-2 max-w-2xl text-slate-400">
      A Rust daemon consumes Wikimedia's EventStreams firehose (~30 edits/second worldwide),
      rolls it up into minute/hour/day buckets with HyperLogLog editor estimates, and holds
      90 days of queryable history inside a 500 MB free-tier Postgres.
    </p>

    <div class="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <div class="rounded-lg border border-slate-800 bg-slate-900 p-4">
        <div class="text-sm text-slate-400">Ingest rate</div>
        <div class="mt-1 text-2xl font-semibold">~30/s</div>
      </div>
      <div class="rounded-lg border border-slate-800 bg-slate-900 p-4">
        <div class="text-sm text-slate-400">Daemon memory</div>
        <div class="mt-1 text-2xl font-semibold">~14 MB</div>
      </div>
      <div class="rounded-lg border border-slate-800 bg-slate-900 p-4">
        <div class="text-sm text-slate-400">History</div>
        <div class="mt-1 text-2xl font-semibold">accumulating</div>
      </div>
      <div class="rounded-lg border border-slate-800 bg-slate-900 p-4">
        <div class="text-sm text-slate-400">Recorded gaps</div>
        <div class="mt-1 text-2xl font-semibold">0</div>
      </div>
    </div>

    <p class="mt-8 text-sm text-slate-500">
      Charts land with the dashboard build; live numbers will come from the daemon's own
      stats — measured, not aspirational.
    </p>
  </section>
</template>

<script setup>
const entries = ref([]);
const loading = ref(false);
const error = ref(null);

const form = reactive({
  user_name: "",
  entry_date: "",
  hours: null,
  description: "",
});

// Fetch all entries from the backend and store them.
async function loadEntries() {
  loading.value = true;
  error.value = null;
  try {
    entries.value = await $fetch("/api/entries");
  } catch (e) {
    error.value = "Could not load entries — is the backend running?";
  } finally {
    loading.value = false;
  }
}

// Send the form as a new entry, then clear it and reload the list.
async function addEntry() {
  error.value = null;
  try {
    await $fetch("/api/entries", {
      method: "POST",
      body: {
        user_name: form.user_name,
        entry_date: form.entry_date,
        hours: form.hours,
        description: form.description,
      },
    });
    form.user_name = "";
    form.entry_date = "";
    form.hours = null;
    form.description = "";
    await loadEntries();
  } catch (e) {
    error.value = "Could not save the entry — check the fields and try again.";
  }
}

// Load the list once when the page first appears.
onMounted(loadEntries);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-2xl mx-auto">
      <h1 class="text-2xl font-bold text-slate-800 mb-6">Timesheet</h1>

      <form @submit.prevent="addEntry" class="bg-white rounded-2xl shadow p-6 mb-8 space-y-4">
        <div>
          <label class="block text-sm font-medium text-slate-600 mb-1">Name</label>
          <input v-model="form.user_name" type="text" required
            class="w-full rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <div class="flex gap-4">
          <div class="flex-1">
            <label class="block text-sm font-medium text-slate-600 mb-1">Date</label>
            <input v-model="form.entry_date" type="date" required
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div class="flex-1">
            <label class="block text-sm font-medium text-slate-600 mb-1">Hours</label>
            <input v-model.number="form.hours" type="number" step="0.25" min="0" required
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
        </div>
        <div>
          <label class="block text-sm font-medium text-slate-600 mb-1">What you did</label>
          <textarea v-model="form.description" rows="2" required
            class="w-full rounded-lg border border-slate-300 px-3 py-2"></textarea>
        </div>
        <button type="submit"
          class="bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700">
          Add entry
        </button>
      </form>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow p-6">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">Entries</h2>
        <p v-if="loading" class="text-slate-400">Loading…</p>
        <p v-else-if="entries.length === 0" class="text-slate-400">No entries yet.</p>
        <ul v-else class="divide-y divide-slate-100">
          <li v-for="entry in entries" :key="entry.id" class="py-3">
            <div class="flex justify-between">
              <span class="font-medium text-slate-800">{{ entry.user_name }}</span>
              <span class="text-slate-500">{{ entry.entry_date }} · {{ entry.hours }}h</span>
            </div>
            <p class="text-slate-600 text-sm mt-1">{{ entry.description }}</p>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
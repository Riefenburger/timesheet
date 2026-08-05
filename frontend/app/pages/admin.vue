<script setup>
const entries = ref([]);
const loading = ref(false);
const error = ref(null);

// STUB: hardcoded admin identity for now (employee 3, Amanda).
// Real auth later supplies this from the session instead.
const ADMIN_ID = 2;

async function loadAllEntries() {
  loading.value = true;
  error.value = null;
  try {
    entries.value = await $fetch("/api/admin/entries", {
      headers: { "X-Employee-Id": String(ADMIN_ID) },
    });
  } catch (e) {
    error.value = "Could not load entries — are you signed in as an admin?";
  } finally {
    loading.value = false;
  }
}

onMounted(loadAllEntries);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-4xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Admin — All Entries</h1>
      </div>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow overflow-hidden">
        <p v-if="loading" class="text-slate-400 p-6">Loading…</p>
        <p v-else-if="entries.length === 0" class="text-slate-400 p-6">No entries yet.</p>
        <table v-else class="w-full text-sm">
          <thead class="bg-slate-50 text-slate-500 text-left">
            <tr>
              <th class="px-4 py-3 font-medium">Employee</th>
              <th class="px-4 py-3 font-medium">Date</th>
              <th class="px-4 py-3 font-medium">Class</th>
              <th class="px-4 py-3 font-medium">Room</th>
              <th class="px-4 py-3 font-medium">Details</th>
              <th class="px-4 py-3 font-medium text-right">Hours</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <tr v-for="entry in entries" :key="entry.id">
              <td class="px-4 py-3 text-slate-800">
                {{ entry.employee_name }}
                <span class="text-slate-400">#{{ entry.employee_number }}</span>
              </td>
              <td class="px-4 py-3 text-slate-600">{{ entry.entry_date }}</td>
              <td class="px-4 py-3 text-slate-600">{{ entry.class_name }}</td>
              <td class="px-4 py-3 text-slate-600">{{ entry.teacher_room }}</td>
              <td class="px-4 py-3 text-slate-600">{{ entry.details }}</td>
              <td class="px-4 py-3 text-slate-800 text-right">{{ entry.hours }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
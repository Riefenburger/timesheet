<script setup>
const employees = ref([]);
const selectedEmployeeId = ref(null);
const entries = ref([]);
const loading = ref(false);
const error = ref(null);

const form = reactive({
  entry_date: "",
  class_name: "",
  teacher_room: "",
  details: "",
  hours: null,
});

// Load the employee list for the "who am I" picker.
async function loadEmployees() {
  try {
    employees.value = await $fetch("/api/employees");
  } catch (e) {
    error.value = "Could not load employees — is the backend running?";
  }
}

// Load the selected employee's own entries — sends the identity header.
async function loadEntries() {
  if (!selectedEmployeeId.value) {
    entries.value = [];
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    entries.value = await $fetch("/api/entries", {
      headers: { "X-Employee-Id": String(selectedEmployeeId.value) },
    });
  } catch (e) {
    error.value = "Could not load entries.";
  } finally {
    loading.value = false;
  }
}

// Submit as the selected employee, then refresh the list.
async function addEntry() {
  error.value = null;
  try {
    await $fetch("/api/entries", {
      method: "POST",
      headers: { "X-Employee-Id": String(selectedEmployeeId.value) },
      body: {
        entry_date: form.entry_date,
        class_name: form.class_name,
        teacher_room: form.teacher_room,
        details: form.details,
        hours: form.hours,
      },
    });
    form.entry_date = "";
    form.class_name = "";
    form.teacher_room = "";
    form.details = "";
    form.hours = null;
    await loadEntries();
  } catch (e) {
    error.value = "Could not save the entry — check the fields and try again.";
  }
}

// When you switch who you are, reload that person's entries.
watch(selectedEmployeeId, loadEntries);

onMounted(loadEmployees);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-2xl mx-auto">
      <h1 class="text-2xl font-bold text-slate-800 mb-6">Timesheet</h1>

      <div class="bg-white rounded-2xl shadow p-6 mb-6">
        <label class="block text-sm font-medium text-slate-600 mb-1">Logged in as</label>
        <select v-model.number="selectedEmployeeId"
          class="w-full rounded-lg border border-slate-300 px-3 py-2">
          <option :value="null" disabled>Select an employee…</option>
          <option v-for="emp in employees" :key="emp.id" :value="emp.id">
            {{ emp.name }} (#{{ emp.employee_number }})
          </option>
        </select>
        <p class="text-xs text-slate-400 mt-2">
          Temporary stand-in for Google sign-in — switch to see each person's own entries.
        </p>
      </div>

      <form @submit.prevent="addEntry" class="bg-white rounded-2xl shadow p-6 mb-8">
        <fieldset :disabled="!selectedEmployeeId" class="space-y-4 disabled:opacity-50">
          <div class="flex gap-4">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Date</label>
              <input v-model="form.entry_date" type="date" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="w-28">
              <label class="block text-sm font-medium text-slate-600 mb-1">Hours</label>
              <input v-model.number="form.hours" type="number" step="0.25" min="0" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Class Name / Time</label>
            <input v-model="form.class_name" type="text" required
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Teacher Room #</label>
            <input v-model="form.teacher_room" type="text" required
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Teach / Dem Details</label>
            <input v-model="form.details" type="text" required
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <button type="submit"
            class="bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700">
            Add entry
          </button>
        </fieldset>
      </form>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow p-6">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">Entries</h2>
        <p v-if="!selectedEmployeeId" class="text-slate-400">Pick an employee to see their entries.</p>
        <p v-else-if="loading" class="text-slate-400">Loading…</p>
        <p v-else-if="entries.length === 0" class="text-slate-400">No entries yet.</p>
        <ul v-else class="divide-y divide-slate-100">
          <li v-for="entry in entries" :key="entry.id" class="py-3">
            <div class="flex justify-between">
              <span class="font-medium text-slate-800">{{ entry.class_name }}</span>
              <span class="text-slate-500">{{ entry.entry_date }} · {{ entry.hours }}h</span>
            </div>
            <p class="text-slate-600 text-sm mt-1">
              Room {{ entry.teacher_room }} — {{ entry.details }}
            </p>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
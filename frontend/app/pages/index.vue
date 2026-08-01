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
  hours: null,
  category: null,
  session_duration: null,
  session_count: null,
});

// Categories the selected employee is rated for.
const myCategories = ref([]);       // normal category names
const privateDurations = ref([]);   // durations they have private rates for

// Is the currently-picked category the private one?
const isPrivate = computed(() => form.category === "private");

async function loadEmployees() {
  try {
    employees.value = await $fetch("/api/employees");
  } catch (e) {
    error.value = "Could not load employees — is the backend running?";
  }
}

async function loadEntries() {
  if (!selectedEmployeeId.value) { entries.value = []; return; }
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

async function loadMyCategories() {
  if (!selectedEmployeeId.value) {
    myCategories.value = [];
    privateDurations.value = [];
    return;
  }
  try {
    const data = await $fetch("/api/entries/categories", {
      headers: { "X-Employee-Id": String(selectedEmployeeId.value) },
    });
    myCategories.value = data.categories ?? [];
    privateDurations.value = data.private_durations ?? [];
  } catch (e) {
    myCategories.value = [];
    privateDurations.value = [];
  }
}

function resetForm() {
  form.entry_date = "";
  form.class_name = "";
  form.teacher_room = "";
  form.hours = null;
  form.category = null;
  form.session_duration = null;
  form.session_count = null;
}

async function addEntry() {
  error.value = null;

  // Build the body — private sends duration + count (hours derived server-side);
  // normal sends hours.
  const body = {
    entry_date: form.entry_date,
    class_name: form.class_name,
    teacher_room: form.teacher_room,
    details: "",
    category: form.category,
    hours: isPrivate.value ? 0 : form.hours,
    session_duration: isPrivate.value ? form.session_duration : null,
    session_count: isPrivate.value ? form.session_count : null,
  };

  try {
    await $fetch("/api/entries", {
      method: "POST",
      headers: { "X-Employee-Id": String(selectedEmployeeId.value) },
      body,
    });
    resetForm();
    await loadEntries();
  } catch (e) {
    error.value = "Could not save the entry — check the fields and try again.";
  }
}

// Clear the private/hours sub-fields whenever the category changes, so stale
// values from a previous pick don't linger.
watch(() => form.category, () => {
  form.hours = null;
  form.session_duration = null;
  form.session_count = null;
});

watch(selectedEmployeeId, () => {
  loadEntries();
  loadMyCategories();
  resetForm();
});

onMounted(loadEmployees);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-2xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Timesheet</h1>
        <div class="flex items-center gap-4">
          <NuxtLink to="/admin-totals" class="text-sm text-slate-500 hover:text-slate-800">Admin Totals →</NuxtLink>
          <NuxtLink to="/super-totals" class="text-sm text-slate-500 hover:text-slate-800">Pay →</NuxtLink>
          <NuxtLink to="/admin-users" class="text-sm text-slate-500 hover:text-slate-800">Employees →</NuxtLink>
        </div>
      </div>

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
          Temporary stand-in for sign-in — switch to see each person's own entries.
        </p>
      </div>

      <form @submit.prevent="addEntry" class="bg-white rounded-2xl shadow p-6 mb-8">
        <fieldset :disabled="!selectedEmployeeId" class="space-y-4 disabled:opacity-50">

          <!-- Row 1: Class Name & Time, Room #, Date -->
          <div class="flex gap-4">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Class Name &amp; Time</label>
              <input v-model="form.class_name" type="text" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="w-28">
              <label class="block text-sm font-medium text-slate-600 mb-1">Room #</label>
              <input v-model="form.teacher_room" type="text" :required="!isPrivate"
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="w-40">
              <label class="block text-sm font-medium text-slate-600 mb-1">Date</label>
              <input v-model="form.entry_date" type="date" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
          </div>

          <!-- Row 2: Role (category), then Hours OR the private expansion -->
          <div class="flex gap-4 items-start">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Role</label>
              <select v-model="form.category" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2 capitalize">
                <option :value="null" disabled>Select a role…</option>
                <option v-for="c in myCategories" :key="c" :value="c" class="capitalize">{{ c }}</option>
                <option v-if="privateDurations.length" value="private">private</option>
              </select>
            </div>

            <!-- Normal: hours -->
            <div v-if="!isPrivate" class="w-28">
              <label class="block text-sm font-medium text-slate-600 mb-1">Hours</label>
              <input v-model.number="form.hours" type="number" step="0.25" min="0" :required="!isPrivate"
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>

            <!-- Private: duration + session count -->
            <template v-else>
              <div class="w-32">
                <label class="block text-sm font-medium text-slate-600 mb-1">Duration</label>
                <select v-model.number="form.session_duration" :required="isPrivate"
                  class="w-full rounded-lg border border-slate-300 px-3 py-2">
                  <option :value="null" disabled>Length…</option>
                  <option v-for="d in privateDurations" :key="d" :value="d">{{ d }} min</option>
                </select>
              </div>
              <div class="w-28">
                <label class="block text-sm font-medium text-slate-600 mb-1"># Sessions</label>
                <input v-model.number="form.session_count" type="number" step="1" min="1" :required="isPrivate"
                  class="w-full rounded-lg border border-slate-300 px-3 py-2" />
              </div>
            </template>
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
              <span class="font-medium text-slate-800">
                {{ entry.class_name }}
                <span v-if="entry.session_duration" class="text-xs text-indigo-500 ml-1">
                  ({{ entry.session_count }}× {{ entry.session_duration }}min private)
                </span>
                <span v-else-if="entry.category" class="text-xs text-slate-400 ml-1 capitalize">
                  {{ entry.category }}
                </span>
              </span>
              <span class="text-slate-500">{{ entry.entry_date }} · {{ entry.hours }}h</span>
            </div>
            <p v-if="entry.details" class="text-slate-600 text-sm mt-1">
              Room {{ entry.teacher_room }}
            </p>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
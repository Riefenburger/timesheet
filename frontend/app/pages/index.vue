<script setup>
const { user } = useAuth();

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

const myCategories = ref([]);
const privateDurations = ref([]);
const isPrivate = computed(() => form.category === "private");
const isLumpSum = computed(() => lumpSumCategories.value.includes(form.category));

const { start, end } = usePayPeriod();

async function loadEntries() {
  loading.value = true;
  error.value = null;
  try {
    entries.value = await $fetch("/api/entries", {
      query: { period_start: start.value, period_end: end.value },
    });
  } catch (e) {
    error.value = "Could not load entries.";
  } finally {
    loading.value = false;
  }
}

async function loadMyCategories() {
  try {
    const data = await $fetch("/api/entries/categories");
    myCategories.value = data.categories ?? [];
    privateDurations.value = data.private_durations ?? [];
  } catch (e) {
    myCategories.value = [];
    privateDurations.value = [];
  }
}

const lumpSumCategories = ref([]); // names of categories flagged is_lump_sum
async function loadLumpSumFlags() {
  try {
    const all = await $fetch("/api/categories");
    lumpSumCategories.value = all.filter((c) => c.is_lump_sum).map((c) => c.name);
  } catch (e) {
    lumpSumCategories.value = [];
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
  const body = {
    entry_date: form.entry_date,
    class_name: isLumpSum.value ? "" : form.class_name,
    teacher_room: isLumpSum.value ? "" : form.teacher_room,
    details: "",
    category: form.category,
    hours: (isPrivate.value || isLumpSum.value) ? 0 : form.hours,
    session_duration: isPrivate.value ? form.session_duration : null,
    session_count: isPrivate.value ? form.session_count : null,
  };
  try {
    await $fetch("/api/entries", { method: "POST", body });
    resetForm();
    await loadEntries();
  } catch (e) {
    error.value = "Could not save the entry — check the fields and try again.";
  }
}

watch(() => form.category, () => {
  form.hours = null;
  form.session_duration = null;
  form.session_count = null;
});

onMounted(() => {
  loadEntries();
  loadMyCategories();
  loadLumpSumFlags();
});
</script>

<template>
  <div class="py-6 px-4 sm:py-10">
    <div class="max-w-2xl mx-auto">
      <h1 class="text-2xl font-bold text-ink-900 mb-6">
        My Time
        <span v-if="user" class="text-base font-normal text-ink-400">— {{ user.name }}</span>
      </h1>

      <form @submit.prevent="addEntry" class="bg-white rounded-2xl shadow p-4 sm:p-6 mb-8">
        <fieldset class="space-y-4">
          <div class="flex flex-col sm:flex-row gap-4">
            <div v-if="!isLumpSum" class="flex-1">
              <label class="block text-sm font-medium text-ink-700 mb-1">Class Name &amp; Time</label>
              <input v-model="form.class_name" type="text" :required="!isLumpSum"
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div v-if="!isLumpSum" class="w-full sm:w-28">
              <label class="block text-sm font-medium text-ink-700 mb-1">Room #</label>
              <input v-model="form.teacher_room" type="text" :required="!isPrivate && !isLumpSum"
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="w-full sm:w-40">
              <label class="block text-sm font-medium text-ink-700 mb-1">Date</label>
              <input v-model="form.entry_date" type="date" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
          </div>

          <div class="flex flex-col sm:flex-row gap-4 items-stretch sm:items-start">
            <div class="flex-1">
              <label class="block text-sm font-medium text-ink-700 mb-1">Role</label>
              <select v-model="form.category" required
                class="w-full rounded-lg border border-slate-300 px-3 py-2 capitalize">
                <option :value="null" disabled>Select a role…</option>
                <option v-for="c in myCategories" :key="c" :value="c" class="capitalize">{{ c }}</option>
                <option v-if="privateDurations.length" value="private">private</option>
              </select>
            </div>
            <div v-if="!isPrivate && !isLumpSum" class="w-full sm:w-28">
              <label class="block text-sm font-medium text-ink-700 mb-1">Hours</label>
              <input v-model.number="form.hours" type="number" step="0.25" min="0" :required="!isPrivate && !isLumpSum"
                class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <template v-else-if="isPrivate">
              <div class="w-full sm:w-32">
                <label class="block text-sm font-medium text-ink-700 mb-1">Duration</label>
                <select v-model.number="form.session_duration" :required="isPrivate"
                  class="w-full rounded-lg border border-slate-300 px-3 py-2">
                  <option :value="null" disabled>Length…</option>
                  <option v-for="d in privateDurations" :key="d" :value="d">{{ d }} min</option>
                </select>
              </div>
              <div class="w-full sm:w-28">
                <label class="block text-sm font-medium text-ink-700 mb-1"># Sessions</label>
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

      <div class="bg-white rounded-2xl shadow p-4 sm:p-6">
        <h2 class="text-lg font-semibold text-ink-900 mb-4">Entries</h2>
        <p v-if="loading" class="text-ink-400">Loading…</p>
        <p v-else-if="entries.length === 0" class="text-ink-400">No entries yet.</p>
        <ul v-else class="divide-y divide-slate-100">
          <li v-for="entry in entries" :key="entry.id" class="py-3">
            <div class="flex justify-between gap-2 flex-wrap">
              <span class="font-medium text-ink-900">
                {{ entry.class_name }}
                <span v-if="entry.session_duration" class="text-xs text-indigo-500 ml-1">
                  ({{ entry.session_count }}× {{ entry.session_duration }}min private)
                </span>
                <span v-else-if="entry.category" class="text-xs text-ink-400 ml-1 capitalize">
                  {{ entry.category }}
                </span>
              </span>
              <span class="text-ink-500">{{ entry.entry_date }} · {{ entry.hours }}h</span>
            </div>
            <p v-if="entry.teacher_room" class="text-ink-700 text-sm mt-1">
              Room {{ entry.teacher_room }}
            </p>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
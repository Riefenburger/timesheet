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

// --- Edit mode ---
const editingId = ref(null);
const formEl = ref(null);
// An entry can hold a category (or private duration) the employee no longer has a
// rate for. Those aren't in the pickers, so they're injected while editing that
// entry — otherwise the select would show blank and silently lose the value.
const extraCategory = ref(null);
const extraDuration = ref(null);
const categoryOptions = computed(() => {
  const list = [...myCategories.value];
  if (extraCategory.value && !list.includes(extraCategory.value)) list.push(extraCategory.value);
  return list;
});
const durationOptions = computed(() => {
  const list = [...privateDurations.value];
  if (extraDuration.value !== null && !list.includes(extraDuration.value)) {
    list.push(extraDuration.value);
    list.sort((a, b) => a - b);
  }
  return list;
});

const { start, end } = usePayPeriod();

async function loadEntries() {
  loading.value = true;
  error.value = null;
  // Never leave a delete armed on a row that's about to be re-rendered.
  confirmDeleteId.value = null;
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
  editingId.value = null;
  extraCategory.value = null;
  extraDuration.value = null;
  // Delete lives in the edit controls now, so leaving edit mode must disarm it.
  confirmDeleteId.value = null;
}

async function submitEntry() {
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
    if (editingId.value === null) {
      await $fetch("/api/entries", { method: "POST", body });
    } else {
      await $fetch(`/api/entries/${editingId.value}`, { method: "PUT", body });
    }
    resetForm();
    await loadEntries();
  } catch (e) {
    error.value = editingId.value === null
      ? "Could not save the entry — check the fields and try again."
      : "Could not update the entry — check the fields and try again.";
  }
}

// Load an entry back into the form. Set while populating so the category watcher
// below doesn't wipe the values we just assigned: that watcher flushes on the
// next tick, so a plain assignment would be cleared a moment later.
let populating = false;

async function startEdit(entry) {
  error.value = null;
  confirmDeleteId.value = null;
  populating = true;
  editingId.value = entry.id;
  // Keep a category/duration the employee no longer has a rate for selectable.
  extraCategory.value = entry.category && !myCategories.value.includes(entry.category)
    && entry.category !== "private" ? entry.category : null;
  extraDuration.value = entry.session_duration !== null
    && !privateDurations.value.includes(entry.session_duration) ? entry.session_duration : null;
  form.entry_date = entry.entry_date;
  form.class_name = entry.class_name;
  form.teacher_room = entry.teacher_room;
  form.category = entry.category;
  // Private hours are derived from duration × count, so leave hours empty there.
  form.hours = entry.session_duration ? null : Number(entry.hours);
  form.session_duration = entry.session_duration;
  form.session_count = entry.session_count;
  await nextTick();          // the watcher has flushed by now
  populating = false;
  formEl.value?.scrollIntoView({ behavior: "smooth", block: "start" });
}

function cancelEdit() {
  resetForm();
  error.value = null;
}

// ---- Delete, with an inline two-step confirm ----
const confirmDeleteId = ref(null);
const deleting = ref(false);

async function deleteEntry(id) {
  error.value = null;
  deleting.value = true;
  try {
    await $fetch(`/api/entries/${id}`, { method: "DELETE" });
    // Deleting the entry being edited leaves the form pointing at nothing.
    if (editingId.value === id) resetForm();
    confirmDeleteId.value = null;
    await loadEntries();
  } catch (e) {
    error.value = "Could not delete the entry.";
  } finally {
    deleting.value = false;
  }
}

watch(() => form.category, () => {
  if (populating) return;     // only clear on a real user change
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

      <form ref="formEl" @submit.prevent="submitEntry" class="bg-white rounded-2xl shadow p-4 sm:p-6 mb-8">
        <fieldset class="space-y-4">
          <div v-if="editingId !== null" class="flex items-center justify-between gap-2 -mt-1">
            <h2 class="text-sm font-semibold text-indigo-600">Editing entry</h2>
            <button type="button" @click="cancelEdit"
              class="text-xs text-ink-400 hover:text-ink-700 px-2 py-1">Cancel</button>
          </div>
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
                <option v-for="c in categoryOptions" :key="c" :value="c" class="capitalize">{{ c }}</option>
                <option v-if="privateDurations.length || extraDuration !== null" value="private">private</option>
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
                  <option v-for="d in durationOptions" :key="d" :value="d">{{ d }} min</option>
                </select>
              </div>
              <div class="w-full sm:w-28">
                <label class="block text-sm font-medium text-ink-700 mb-1"># Sessions</label>
                <input v-model.number="form.session_count" type="number" step="1" min="1" :required="isPrivate"
                  class="w-full rounded-lg border border-slate-300 px-3 py-2" />
              </div>
            </template>
          </div>

          <div class="flex flex-wrap items-center gap-3">
            <!-- Two-step confirm replaces the controls, so Save can't be hit by
                 accident while a delete is armed. -->
            <template v-if="editingId !== null && confirmDeleteId === editingId">
              <span class="text-sm text-ink-700 w-full sm:w-auto">Delete this entry?</span>
              <button type="button" @click="deleteEntry(editingId)" :disabled="deleting"
                class="bg-red-600 text-white rounded-lg px-4 py-2 font-medium hover:bg-red-700 disabled:opacity-50">
                {{ deleting ? "Deleting…" : "Yes, delete" }}
              </button>
              <button type="button" @click="confirmDeleteId = null"
                class="rounded-lg border border-slate-300 px-4 py-2 font-medium text-ink-700 hover:bg-slate-50">
                Cancel
              </button>
            </template>
            <template v-else>
              <button type="submit"
                class="bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700">
                {{ editingId === null ? "Add entry" : "Save changes" }}
              </button>
              <button v-if="editingId !== null" type="button" @click="cancelEdit"
                class="rounded-lg border border-slate-300 px-4 py-2 font-medium text-ink-700 hover:bg-slate-50">
                Cancel
              </button>
              <!-- Sits apart from Save/Cancel on a wide screen; wraps below them
                   on a phone rather than crowding them. -->
              <button v-if="editingId !== null" type="button" @click="confirmDeleteId = editingId"
                class="sm:ml-auto rounded-lg border border-red-300 px-4 py-2 font-medium text-red-600 hover:bg-red-50">
                Delete
              </button>
            </template>
          </div>
        </fieldset>
      </form>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow p-4 sm:p-6">
        <h2 class="text-lg font-semibold text-ink-900 mb-4">Entries</h2>
        <p v-if="loading" class="text-ink-400">Loading…</p>
        <p v-else-if="entries.length === 0" class="text-ink-400">No entries yet.</p>
        <ul v-else class="divide-y divide-slate-100">
          <li v-for="entry in entries" :key="entry.id" class="py-3"
            :class="editingId === entry.id ? 'bg-indigo-50/50 -mx-2 px-2 rounded-lg' : ''">
            <div class="flex items-start gap-2">
              <div class="flex-1 min-w-0">
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
              </div>
              <!-- Always visible, never hover-revealed: a hover-only control is
                   unreachable on a phone. 40px box for the tap target. -->
              <button type="button" @click="startEdit(entry)"
                :aria-label="`Edit entry: ${entry.class_name || entry.category || entry.entry_date}`"
                title="Edit entry"
                class="shrink-0 w-10 h-10 -mr-2 flex items-center justify-center rounded-lg
                       hover:bg-slate-100 touch-manipulation"
                :class="editingId === entry.id ? 'text-indigo-600' : 'text-ink-400 hover:text-ink-900'">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24"
                  stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round"
                    d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                </svg>
              </button>
            </div>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
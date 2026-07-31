<script setup>
const ADMIN_ID = 3; // STUB: Amanda (admin) on dev

const { current, prev, next, goTo, label, start, end, makePeriod, monthName } = usePayPeriod();

const employees = ref([]);
const loading = ref(false);
const error = ref(null);

const payFilter = ref("both"); // both | check | payroll
const expanded = reactive({});   // employee_id -> bool
const sessions = reactive({});   // employee_id -> entries[]
const sessionsLoading = reactive({});

const showCalendar = ref(false);
const calYear = ref(current.value.year);
const calMonth = ref(current.value.month);

const headers = { "X-Employee-Id": String(ADMIN_ID) };

async function loadTotals() {
  loading.value = true;
  error.value = null;
  for (const k of Object.keys(expanded)) delete expanded[k];
  for (const k of Object.keys(sessions)) delete sessions[k];
  try {
    employees.value = await $fetch("/api/admin/totals", {
      headers,
      query: { period_start: start.value, period_end: end.value },
    });
  } catch (e) {
    error.value = "Could not load totals — are you an admin?";
  } finally {
    loading.value = false;
  }
}

// Filtered roster by pay method.
const visibleEmployees = computed(() => {
  if (payFilter.value === "both") return employees.value;
  return employees.value.filter((e) => e.pay_method === payFilter.value);
});

// Per-employee summed hours across all categories.
function sumHours(emp, field) {
  return emp.categories.reduce((acc, c) => acc + Number(c[field]), 0);
}
function totalHours(emp) {
  return emp.categories.reduce(
    (acc, c) => acc + Number(c.regular_hours) + Number(c.overtime_hours) + Number(c.sick_hours),
    0
  );
}

async function toggleExpand(emp) {
  const id = emp.employee_id;
  expanded[id] = !expanded[id];
  if (expanded[id] && sessions[id] === undefined) {
    sessionsLoading[id] = true;
    try {
      sessions[id] = await $fetch(`/api/admin/employees/${id}/entries`, {
        headers,
        query: { period_start: start.value, period_end: end.value },
      });
    } catch (e) {
      sessions[id] = [];
    } finally {
      sessionsLoading[id] = false;
    }
  }
}

function dayClasses(day) {
  const half = day <= 15 ? 1 : 2;
  const isSelected = isCurrentPeriod(half);
  if (isSelected) return "bg-emerald-100 text-emerald-700 hover:bg-emerald-200";
  if (half === 1) return "bg-blue-50 text-blue-700 hover:bg-blue-100";
  return "bg-amber-50 text-amber-700 hover:bg-amber-100";
}

const months = ["January","February","March","April","May","June",
  "July","August","September","October","November","December"];

// Years to offer in the dropdown (a few back and forward from now).
const yearOptions = computed(() => {
  const y = new Date().getFullYear();
  const arr = [];
  for (let i = y - 3; i <= y + 1; i++) arr.push(i);
  return arr;
});

// Build the calendar grid: array of weeks, each a 7-slot array of day numbers (or null).
const calendarWeeks = computed(() => {
  const firstDow = new Date(calYear.value, calMonth.value - 1, 1).getDay(); // 0=Sun
  const daysInMonth = new Date(calYear.value, calMonth.value, 0).getDate();
  const cells = [];
  for (let i = 0; i < firstDow; i++) cells.push(null);
  for (let d = 1; d <= daysInMonth; d++) cells.push(d);
  while (cells.length % 7 !== 0) cells.push(null);
  const weeks = [];
  for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));
  return weeks;
});

const daysInCalMonth = computed(() => new Date(calYear.value, calMonth.value, 0).getDate());

function pickPeriod(half) {
  goTo(calYear.value, calMonth.value, half);
  showCalendar.value = false;
  loadTotals();
}
function isCurrentPeriod(half) {
  return current.value.year === calYear.value
    && current.value.month === calMonth.value
    && current.value.half === half;
}

function changePeriod(fn) {
  fn();
  loadTotals();
}

const CATEGORY_OPTIONS = ["teaching", "assisting", "office"];
const TYPE_OPTIONS = ["regular", "overtime", "sick"];

const savingEntry = ref(null); // entry id currently saving

// Save an entry's full state (we send all fields; dropdowns changed one).
async function saveEntry(emp, entry) {
  savingEntry.value = entry.id;
  try {
    await $fetch(`/api/admin/entries/${entry.id}`, {
      method: "PUT",
      headers,
      body: {
        entry_date: entry.entry_date,
        class_name: entry.class_name,
        teacher_room: entry.teacher_room,
        details: entry.details,
        hours: Number(entry.hours),
        category: entry.category === "" ? null : entry.category,
        type: entry.type,
      },
    });
    // Refresh this employee's sessions and the period totals.
    await refreshAfterEntryChange(emp);
  } catch (e) {
    error.value = "Could not save that change.";
  } finally {
    savingEntry.value = null;
  }
}

// Re-fetch the period totals (updates header rollups) and this employee's sessions.
async function refreshAfterEntryChange(emp) {
  // Reload the whole period so header sums reflect the change.
  const updated = await $fetch("/api/admin/totals", {
    headers,
    query: { period_start: start.value, period_end: end.value },
  });
  employees.value = updated;
  // Reload this employee's drill-down sessions.
  delete sessions[emp.employee_id];
  sessions[emp.employee_id] = await $fetch(`/api/admin/employees/${emp.employee_id}/entries`, {
    headers,
    query: { period_start: start.value, period_end: end.value },
  });
}

// --- Entry menu / edit / add / delete ---
const openMenuId = ref(null); // which entry's ⋮ menu is open

const showEntryModal = ref(false);
const entryModalMode = ref("edit"); // "edit" | "add"
const entryModalEmp = ref(null);    // which employee we're adding/editing for
const savingModal = ref(false);
const entryModalError = ref(null);

const entryForm = reactive({
  id: null,
  entry_date: "",
  class_name: "",
  teacher_room: "",
  details: "",
  hours: null,
  category: null,
  type: "regular",
});

function toggleMenu(id) {
  openMenuId.value = openMenuId.value === id ? null : id;
}

function openEditEntry(emp, s) {
  entryModalMode.value = "edit";
  entryModalEmp.value = emp;
  entryForm.id = s.id;
  entryForm.entry_date = s.entry_date;
  entryForm.class_name = s.class_name;
  entryForm.teacher_room = s.teacher_room;
  entryForm.details = s.details;
  entryForm.hours = Number(s.hours);
  entryForm.category = s.category;
  entryForm.type = s.type;
  entryModalError.value = null;
  openMenuId.value = null;
  showEntryModal.value = true;
}

function openAddEntry(emp) {
  entryModalMode.value = "add";
  entryModalEmp.value = emp;
  entryForm.id = null;
  entryForm.entry_date = start.value; // default to the period's start date
  entryForm.class_name = "";
  entryForm.teacher_room = "";
  entryForm.details = "";
  entryForm.hours = null;
  entryForm.category = null;
  entryForm.type = "regular";
  entryModalError.value = null;
  showEntryModal.value = true;
}

function closeEntryModal() {
  showEntryModal.value = false;
  entryModalError.value = null;
}

async function saveEntryModal() {
  entryModalError.value = null;
  if (!entryForm.entry_date || entryForm.hours === null || Number(entryForm.hours) <= 0) {
    entryModalError.value = "Date and a positive number of hours are required.";
    return;
  }
  const body = {
    entry_date: entryForm.entry_date,
    class_name: entryForm.class_name,
    teacher_room: entryForm.teacher_room,
    details: entryForm.details,
    hours: Number(entryForm.hours),
    category: entryForm.category === "" ? null : entryForm.category,
    type: entryForm.type,
  };
  savingModal.value = true;
  try {
    if (entryModalMode.value === "add") {
      await $fetch("/api/admin/entries", {
        method: "POST", headers,
        body: { employee_id: entryModalEmp.value.employee_id, ...body },
      });
    } else {
      await $fetch(`/api/admin/entries/${entryForm.id}`, { method: "PUT", headers, body });
    }
    closeEntryModal();
    await refreshAfterEntryChange(entryModalEmp.value);
  } catch (e) {
    entryModalError.value = (e && e.data) ? String(e.data) : "Could not save the entry.";
  } finally {
    savingModal.value = false;
  }
}

// Delete with a two-step confirm baked into the menu.
const confirmDeleteId = ref(null);
function askDelete(id) { confirmDeleteId.value = id; }
async function doDelete(emp, id) {
  try {
    await $fetch(`/api/admin/entries/${id}`, { method: "DELETE", headers });
    confirmDeleteId.value = null;
    openMenuId.value = null;
    await refreshAfterEntryChange(emp);
  } catch (e) {
    error.value = "Could not delete that entry.";
  }
}

onMounted(loadTotals);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-6xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Admin — Period Totals</h1>
        <div class="flex items-center gap-4">
          <NuxtLink to="/admin" class="text-sm text-slate-500 hover:text-slate-800">Entries →</NuxtLink>
          <NuxtLink to="/admin-users" class="text-sm text-slate-500 hover:text-slate-800">Employees →</NuxtLink>
        </div>
      </div>

      <!-- Period navigation -->
      <div class="bg-white rounded-2xl shadow p-4 mb-6 flex items-center gap-4">
        <button @click="changePeriod(prev)"
          class="rounded-lg border border-slate-300 px-3 py-2 hover:bg-slate-50">←</button>

        <div class="relative">
          <button @click="showCalendar = !showCalendar"
            class="font-medium text-slate-800 px-3 py-2 rounded-lg hover:bg-slate-50">
            {{ label }} ▾
          </button>

          <!-- Calendar popover: one continuous month grid, halves color-coded -->
          <div v-if="showCalendar"
            class="absolute top-full left-0 mt-2 bg-white rounded-xl shadow-xl border border-slate-200 p-4 z-30 w-80">
            <!-- Month / year quick-nav -->
            <div class="flex items-center gap-2 mb-3">
              <select v-model.number="calMonth" class="rounded-lg border border-slate-300 px-2 py-1 text-sm flex-1">
                <option v-for="(m, i) in months" :key="i" :value="i + 1">{{ m }}</option>
              </select>
              <select v-model.number="calYear" class="rounded-lg border border-slate-300 px-2 py-1 text-sm">
                <option v-for="y in yearOptions" :key="y" :value="y">{{ y }}</option>
              </select>
            </div>

            <!-- Legend -->
            <div class="flex gap-4 mb-2 text-xs">
              <button @click="pickPeriod(1)" class="flex items-center gap-1 hover:underline">
                <span class="w-3 h-3 rounded-sm bg-blue-100 border border-blue-300"></span>
                <span class="text-blue-600">1–15</span>
              </button>
              <button @click="pickPeriod(2)" class="flex items-center gap-1 hover:underline">
                <span class="w-3 h-3 rounded-sm bg-amber-100 border border-amber-300"></span>
                <span class="text-amber-600">16–{{ daysInCalMonth }}</span>
              </button>
            </div>

            <!-- Day-of-week header -->
            <div class="grid grid-cols-7 gap-px mb-1">
              <div v-for="(d, i) in ['S','M','T','W','T','F','S']" :key="i"
                class="text-center text-xs text-slate-400 font-medium">{{ d }}</div>
            </div>

            <!-- One continuous month grid -->
            <div class="grid grid-cols-7 gap-px">
              <template v-for="(week, wi) in calendarWeeks" :key="wi">
                <template v-for="(day, di) in week" :key="wi + '-' + di">
                  <div v-if="day === null"></div>
                  <button v-else
                    @click="pickPeriod(day <= 15 ? 1 : 2)"
                    :class="['text-center text-xs py-1.5 rounded transition',
                      dayClasses(day)]">
                    {{ day }}
                  </button>
                </template>
              </template>
            </div>
          </div>
        </div>

        <button @click="changePeriod(next)"
          class="rounded-lg border border-slate-300 px-3 py-2 hover:bg-slate-50">→</button>

        <div class="flex-1"></div>

        <!-- Pay method filter -->
        <select v-model="payFilter" class="rounded-lg border border-slate-300 px-3 py-2 text-sm">
          <option value="both">All employees</option>
          <option value="payroll">Payroll only</option>
          <option value="check">Check only</option>
        </select>
      </div>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow overflow-x-auto">
        <p v-if="loading" class="text-slate-400 p-6">Loading…</p>
        <table v-else class="w-full text-sm">
          <thead class="bg-slate-50 text-slate-500 text-left">
            <tr>
              <th class="px-3 py-3 font-medium w-6"></th>
              <th class="px-3 py-3 font-medium">Employee</th>
              <th class="px-3 py-3 font-medium text-right">Regular</th>
              <th class="px-3 py-3 font-medium text-right">Overtime</th>
              <th class="px-3 py-3 font-medium text-right">Sick</th>
              <th class="px-3 py-3 font-medium text-right">Other $</th>
              <th class="px-3 py-3 font-medium text-right">Comp $</th>
              <th class="px-3 py-3 font-medium text-right">Coaching $</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <template v-for="emp in visibleEmployees" :key="emp.employee_id">
              <tr class="bg-white">
                <td class="px-3 py-3 text-center">
                  <button @click="toggleExpand(emp)"
                    class="text-slate-400 hover:text-slate-700 text-lg leading-none font-mono">
                    {{ expanded[emp.employee_id] ? "▾" : "▸" }}
                  </button>
                </td>
                <td class="px-3 py-3 text-slate-800 font-medium whitespace-nowrap">
                  {{ emp.employee_name }}
                  <span class="text-slate-400 font-normal">#{{ emp.employee_number }}</span>
                  <span class="ml-1 text-xs text-slate-400 capitalize">({{ emp.pay_method }})</span>
                </td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumHours(emp, "regular_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumHours(emp, "overtime_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumHours(emp, "sick_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-600">{{ Number(emp.other_earn).toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-600">{{ Number(emp.competition_earn).toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-600">{{ Number(emp.coaching_earn).toFixed(2) }}</td>
              </tr>
              <tr v-if="expanded[emp.employee_id]" class="bg-slate-50">
                <td></td>
                <td colspan="7" class="px-3 py-3">
                  <p v-if="sessionsLoading[emp.employee_id]" class="text-slate-400 text-xs">Loading sessions…</p>
                  <p v-else-if="!sessions[emp.employee_id] || sessions[emp.employee_id].length === 0"
                    class="text-slate-400 text-xs">No sessions logged this period.</p>
                  <table v-else class="w-full text-xs">
                    <thead class="text-slate-400 text-left">
                      <tr>
                        <th class="px-2 py-1 font-medium">Date</th>
                        <th class="px-2 py-1 font-medium">Class</th>
                        <th class="px-2 py-1 font-medium">Room</th>
                        <th class="px-2 py-1 font-medium">Details</th>
                        <th class="px-2 py-1 font-medium">Category</th>
                        <th class="px-2 py-1 font-medium">Type</th>
                        <th class="px-2 py-1 font-medium text-right">Hours</th>
                        <th class="px-2 py-1 font-medium w-8"></th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="s in sessions[emp.employee_id]" :key="s.id" class="text-slate-600">
                        <td class="px-2 py-1 whitespace-nowrap">{{ s.entry_date }}</td>
                        <td class="px-2 py-1">{{ s.class_name }}</td>
                        <td class="px-2 py-1">{{ s.teacher_room }}</td>
                        <td class="px-2 py-1">{{ s.details }}</td>
                        <td class="px-2 py-1 capitalize text-slate-500">{{ s.category || "—" }}</td>
                        <td class="px-2 py-1">
                          <select v-model="s.type" @change="saveEntry(emp, s)"
                            :disabled="savingEntry === s.id"
                            :class="['rounded border px-1 py-0.5 text-xs',
                              s.type === 'overtime' ? 'border-amber-400 text-amber-700' :
                              s.type === 'sick' ? 'border-purple-400 text-purple-700' :
                              'border-slate-300']">
                            <option v-for="t in TYPE_OPTIONS" :key="t" :value="t">{{ t }}</option>
                          </select>
                        </td>
                        <td class="px-2 py-1 text-right">{{ s.hours }}</td>
                        <td class="px-2 py-1 relative">
                          <button @click="toggleMenu(s.id)"
                            class="text-slate-400 hover:text-slate-700 px-1">⋮</button>
                          <div v-if="openMenuId === s.id"
                            class="absolute right-0 top-full mt-1 bg-white rounded-lg shadow-lg border border-slate-200 z-20 w-32 text-left">
                            <button @click="openEditEntry(emp, s)"
                              class="block w-full px-3 py-2 text-xs hover:bg-slate-50 text-slate-700">Edit</button>
                            <template v-if="confirmDeleteId === s.id">
                              <button @click="doDelete(emp, s.id)"
                                class="block w-full px-3 py-2 text-xs bg-red-500 text-white hover:bg-red-600">Confirm delete</button>
                              <button @click="confirmDeleteId = null"
                                class="block w-full px-3 py-2 text-xs hover:bg-slate-50 text-slate-500">Cancel</button>
                            </template>
                            <button v-else @click="askDelete(s.id)"
                              class="block w-full px-3 py-2 text-xs hover:bg-slate-50 text-red-600">Delete</button>
                          </div>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                  <button @click="openAddEntry(emp)"
                    class="mt-2 text-xs bg-slate-700 text-white rounded px-3 py-1.5 hover:bg-slate-600">
                    + Add entry
                  </button>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </div>
    <!-- Add / Edit entry modal -->
    <div v-if="showEntryModal"
      class="fixed inset-0 bg-black/40 flex items-center justify-center z-50 px-4 py-8 overflow-y-auto"
      @click.self="closeEntryModal">
      <div class="bg-white rounded-2xl shadow-xl p-6 w-full max-w-md my-auto">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">
          {{ entryModalMode === "add" ? "Add Entry" : "Edit Entry" }}
          <span v-if="entryModalEmp" class="text-sm font-normal text-slate-500">— {{ entryModalEmp.employee_name }}</span>
        </h2>
        <div class="space-y-3">
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Date</label>
            <input v-model="entryForm.entry_date" type="date" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div class="flex gap-3">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Class</label>
              <input v-model="entryForm.class_name" type="text" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Room</label>
              <input v-model="entryForm.teacher_room" type="text" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Details</label>
            <input v-model="entryForm.details" type="text" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div class="flex gap-3">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Hours</label>
              <input v-model.number="entryForm.hours" type="number" step="0.25" min="0" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Category</label>
              <select v-model="entryForm.category" class="w-full rounded-lg border border-slate-300 px-3 py-2">
                <option :value="null">— none —</option>
                <option v-for="c in ['teaching','assisting','office']" :key="c" :value="c">{{ c }}</option>
              </select>
            </div>
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Type</label>
              <select v-model="entryForm.type" class="w-full rounded-lg border border-slate-300 px-3 py-2">
                <option v-for="t in ['regular','overtime','sick']" :key="t" :value="t">{{ t }}</option>
              </select>
            </div>
          </div>
          <p v-if="entryModalError" class="text-red-600 text-sm">{{ entryModalError }}</p>
          <div class="flex justify-end gap-3 pt-2">
            <button @click="closeEntryModal"
              class="rounded-lg border border-slate-300 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50">Cancel</button>
            <button @click="saveEntryModal" :disabled="savingModal"
              class="bg-emerald-600 text-white rounded-lg px-4 py-2 text-sm font-medium hover:bg-emerald-500 disabled:opacity-50">
              {{ savingModal ? "Saving…" : "Save" }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
<script setup>
const { current, prev, next, goTo, label, start, end } = usePayPeriod();

const employees = ref([]);
const loading = ref(false);
const error = ref(null);
const payFilter = ref("both");

const showCalendar = ref(false);
const calYear = ref(current.value.year);
const calMonth = ref(current.value.month);

// employee_id -> { categoryName: amount, "private_NN": amount }
const rateMap = reactive({});

async function loadRatesFor(empId) {
  try {
    const rates = await $fetch(`/api/admin/employees/${empId}/rates`);
    const m = {};
    for (const r of rates) m[r.label] = Number(r.amount);
    rateMap[empId] = m;
  } catch (e) {
    rateMap[empId] = {};
  }
}

function catRate(emp, category) {
  const m = rateMap[emp.employee_id];
  return m && m[category] !== undefined ? m[category] : null;
}
function privRate(emp, duration) {
  const m = rateMap[emp.employee_id];
  return m && m[`private_${duration}`] !== undefined ? m[`private_${duration}`] : null;
}

async function loadTotals() {
  loading.value = true;
  error.value = null;
  try {
    const data = await $fetch("/api/admin/totals", {
      query: { period_start: start.value, period_end: end.value },
    });
    employees.value = data.map((emp) => ({
      ...emp,
      _other: Number(emp.other_earn),
      _comp: Number(emp.competition_earn),
      _coaching: Number(emp.coaching_earn),
      _savingDollars: false,
      _privateExpanded: false,
      categories: emp.categories
        .filter((c) => c.category !== "private")
        .map((c) => ({
          ...c,
          _regular: Number(c.regular_hours),
          _overtime: Number(c.overtime_hours),
          _sick: Number(c.sick_hours),
          _saving: false,
        })),
      privates: (emp.private_sessions || []).map((p) => ({
        ...p,
        _count: Number(p.session_count),
        _saving: false,
      })),
    }));
    // Fetch each employee's rates (separately, for now).
    for (const emp of employees.value) loadRatesFor(emp.employee_id);
  } catch (e) {
    error.value = "Could not load totals — are you a super admin?";
  } finally {
    loading.value = false;
  }
}

const visibleEmployees = computed(() => {
  if (payFilter.value === "both") return employees.value;
  return employees.value.filter((e) => e.pay_method === payFilter.value);
});

function sumField(emp, field) {
  return emp.categories.reduce((a, c) => a + Number(c[field]), 0);
}
function privateHours(emp) {
  return emp.privates.reduce((a, p) => a + (p.session_duration / 60) * Number(p.session_count), 0);
}

function isDirty(c) {
  return c._regular !== Number(c.regular_hours)
    || c._overtime !== Number(c.overtime_hours)
    || c._sick !== Number(c.sick_hours);
}
function isMismatch(c) {
  const typed = c._regular + c._overtime;
  return Math.abs(typed - Number(c.logged_hours)) > 0.001;
}
function dollarsDirty(emp) {
  return emp._other !== Number(emp.other_earn)
    || emp._comp !== Number(emp.competition_earn)
    || emp._coaching !== Number(emp.coaching_earn);
}
function privateDirty(p) { return p._count !== Number(p.session_count); }
function privateMismatch(p) { return p._count !== Number(p.logged_count); }

async function saveCategory(emp, c) {
  c._saving = true;
  error.value = null;
  try {
    await $fetch("/api/admin/category-hours", {
      method: "POST",
      body: {
        employee_id: emp.employee_id,
        period_start: start.value, period_end: end.value,
        category: c.category,
        regular_hours: c._regular, overtime_hours: c._overtime, sick_hours: c._sick,
      },
    });
    c.regular_hours = c._regular;
    c.overtime_hours = c._overtime;
    c.sick_hours = c._sick;
    c.admin_edited = true;
  } catch (e) {
    error.value = "Could not save that row.";
  } finally {
    c._saving = false;
  }
}
async function revertCategory(emp, c) {
  c._saving = true;
  error.value = null;
  try {
    await $fetch("/api/admin/category-hours/revert", {
      method: "POST",
      body: {
        employee_id: emp.employee_id,
        period_start: start.value, period_end: end.value,
        category: c.category,
      },
    });
    await loadTotals();
  } catch (e) {
    error.value = "Could not refresh that row.";
    c._saving = false;
  }
}
async function saveDollars(emp) {
  emp._savingDollars = true;
  error.value = null;
  try {
    await $fetch("/api/admin/dollar-totals", {
      method: "POST",
      body: {
        employee_id: emp.employee_id,
        period_start: start.value, period_end: end.value,
        other_earn: emp._other, competition_earn: emp._comp, coaching_earn: emp._coaching,
      },
    });
    emp.other_earn = emp._other;
    emp.competition_earn = emp._comp;
    emp.coaching_earn = emp._coaching;
  } catch (e) {
    error.value = "Could not save dollars.";
  } finally {
    emp._savingDollars = false;
  }
}
async function savePrivate(emp, p) {
  p._saving = true;
  error.value = null;
  try {
    await $fetch("/api/admin/private-counts", {
      method: "POST",
      body: {
        employee_id: emp.employee_id,
        period_start: start.value, period_end: end.value,
        session_duration: p.session_duration, session_count: p._count,
      },
    });
    p.session_count = p._count;
    p.admin_edited = true;
  } catch (e) {
    error.value = "Could not save that private count.";
  } finally {
    p._saving = false;
  }
}
async function revertPrivate(emp, p) {
  p._saving = true;
  error.value = null;
  try {
    await $fetch("/api/admin/category-hours/revert", {
      method: "POST",
      body: {
        employee_id: emp.employee_id,
        period_start: start.value, period_end: end.value,
        category: "private", session_duration: p.session_duration,
      },
    });
    await loadTotals();
  } catch (e) {
    error.value = "Could not refresh that private row.";
    p._saving = false;
  }
}

// --- Entry drill-down ---
const entriesOpen = reactive({});
const entriesData = reactive({});
const entriesLoading = reactive({});
function catKey(emp, category) { return `${emp.employee_id}:${category}`; }
function privKey(emp, duration) { return `${emp.employee_id}:private:${duration}`; }

async function toggleCategoryEntries(emp, category) {
  const key = catKey(emp, category);
  entriesOpen[key] = !entriesOpen[key];
  if (entriesOpen[key] && entriesData[key] === undefined) {
    entriesLoading[key] = true;
    try {
      entriesData[key] = await $fetch(`/api/admin/employees/${emp.employee_id}/entries`, {
        query: { period_start: start.value, period_end: end.value,
          category: category === "uncategorized" ? "__uncategorized__" : category },
      });
    } catch (e) { entriesData[key] = []; }
    finally { entriesLoading[key] = false; }
  }
}
async function togglePrivateEntries(emp, duration) {
  const key = privKey(emp, duration);
  entriesOpen[key] = !entriesOpen[key];
  if (entriesOpen[key] && entriesData[key] === undefined) {
    entriesLoading[key] = true;
    try {
      const all = await $fetch(`/api/admin/employees/${emp.employee_id}/entries`, {
        query: { period_start: start.value, period_end: end.value, category: "private" },
      });
      entriesData[key] = all.filter((e) => e.session_duration === duration);
    } catch (e) { entriesData[key] = []; }
    finally { entriesLoading[key] = false; }
  }
}

const allCategories = ref([]);
async function loadCategoryList() {
  try { allCategories.value = await $fetch("/api/categories"); }
  catch (e) { allCategories.value = []; }
}

const showEntryModal = ref(false);
const entryModalMode = ref("edit");
const entryModalEmp = ref(null);
const entryModalKey = ref(null);
const savingModal = ref(false);
const entryModalError = ref(null);
const entryForm = reactive({
  id: null, entry_date: "", class_name: "", teacher_room: "",
  category: null, hours: null, session_duration: null, session_count: null,
});
const entryIsPrivate = computed(() => {
  const cat = allCategories.value.find((c) => c.name === entryForm.category);
  return cat ? cat.is_private : false;
});

function openEditEntry(emp, key, s) {
  entryModalMode.value = "edit";
  entryModalEmp.value = emp;
  entryModalKey.value = key;
  entryForm.id = s.id;
  entryForm.entry_date = s.entry_date;
  entryForm.class_name = s.class_name;
  entryForm.teacher_room = s.teacher_room;
  entryForm.category = s.category;
  entryForm.hours = Number(s.hours);
  entryForm.session_duration = s.session_duration;
  entryForm.session_count = s.session_count;
  entryModalError.value = null;
  showEntryModal.value = true;
}
function openAddEntry(emp, key, presetCategory, presetDuration) {
  entryModalMode.value = "add";
  entryModalEmp.value = emp;
  entryModalKey.value = key;
  entryForm.id = null;
  entryForm.entry_date = start.value;
  entryForm.class_name = "";
  entryForm.teacher_room = "";
  entryForm.category = presetCategory ?? null;
  entryForm.hours = null;
  entryForm.session_duration = presetDuration ?? null;
  entryForm.session_count = null;
  entryModalError.value = null;
  showEntryModal.value = true;
}
function closeEntryModal() { showEntryModal.value = false; entryModalError.value = null; }

async function saveEntryModal() {
  entryModalError.value = null;
  if (!entryForm.entry_date) { entryModalError.value = "Date is required."; return; }
  const priv = entryIsPrivate.value;
  const body = {
    entry_date: entryForm.entry_date,
    class_name: entryForm.class_name,
    teacher_room: entryForm.teacher_room,
    details: "",
    category: entryForm.category === "" ? null : entryForm.category,
    hours: priv ? 0 : Number(entryForm.hours),
    session_duration: priv ? entryForm.session_duration : null,
    session_count: priv ? entryForm.session_count : null,
    type: "regular",
  };
  savingModal.value = true;
  try {
    if (entryModalMode.value === "add") {
      await $fetch("/api/admin/entries", {
        method: "POST",
        body: { employee_id: entryModalEmp.value.employee_id, ...body },
      });
    } else {
      await $fetch(`/api/admin/entries/${entryForm.id}`, { method: "PUT", body });
    }
    closeEntryModal();
    await refreshAfterEntryChange(entryModalEmp.value, entryModalKey.value);
  } catch (e) {
    entryModalError.value = (e && e.data) ? String(e.data) : "Could not save the entry.";
  } finally {
    savingModal.value = false;
  }
}

const confirmDeleteId = ref(null);
async function doDelete(emp, key, id) {
  try {
    await $fetch(`/api/admin/entries/${id}`, { method: "DELETE"});
    confirmDeleteId.value = null;
    await refreshAfterEntryChange(emp, key);
  } catch (e) { error.value = "Could not delete that entry."; }
}

async function refreshAfterEntryChange(emp, key) {
  await loadTotals();
  delete entriesData[key];
  entriesOpen[key] = false;
  if (key.includes(":private:")) {
    const duration = Number(key.split(":private:")[1]);
    await togglePrivateEntries(emp, duration);
  } else {
    const category = key.split(":")[1];
    await toggleCategoryEntries(emp, category);
  }
}

const months = ["January","February","March","April","May","June",
  "July","August","September","October","November","December"];
const yearOptions = computed(() => {
  const y = new Date().getFullYear();
  const arr = [];
  for (let i = y - 3; i <= y + 1; i++) arr.push(i);
  return arr;
});
const calendarWeeks = computed(() => {
  const firstDow = new Date(calYear.value, calMonth.value - 1, 1).getDay();
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
function isCurrentPeriod(half) {
  return current.value.year === calYear.value
    && current.value.month === calMonth.value
    && current.value.half === half;
}
function dayClasses(day) {
  const half = day <= 15 ? 1 : 2;
  if (isCurrentPeriod(half)) return "bg-emerald-100 text-emerald-700 hover:bg-emerald-200";
  if (half === 1) return "bg-blue-50 text-blue-700 hover:bg-blue-100";
  return "bg-amber-50 text-amber-700 hover:bg-amber-100";
}
function pickPeriod(half) {
  goTo(calYear.value, calMonth.value, half);
  showCalendar.value = false;
  loadTotals();
}
function changePeriod(fn) { fn(); loadTotals(); }

onMounted(() => { loadTotals(); loadCategoryList(); });
</script>

<template>
  <div class="py-10 px-4">
    <div class="max-w-6xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Super Admin — Totals &amp; Rates</h1>
      </div>
      <div class="bg-white rounded-2xl shadow p-4 mb-6 flex items-center gap-4">
        <button @click="changePeriod(prev)"
          class="rounded-lg border border-slate-300 px-3 py-2 hover:bg-slate-50">←</button>
        <div class="relative">
          <button @click="showCalendar = !showCalendar"
            class="font-medium text-slate-800 px-3 py-2 rounded-lg hover:bg-slate-50">
            {{ label }} ▾
          </button>
          <div v-if="showCalendar"
            class="absolute top-full left-0 mt-2 bg-white rounded-xl shadow-xl border border-slate-200 p-4 z-30 w-80">
            <div class="flex items-center gap-2 mb-3">
              <select v-model.number="calMonth" class="rounded-lg border border-slate-300 px-2 py-1 text-sm flex-1">
                <option v-for="(m, i) in months" :key="i" :value="i + 1">{{ m }}</option>
              </select>
              <select v-model.number="calYear" class="rounded-lg border border-slate-300 px-2 py-1 text-sm">
                <option v-for="y in yearOptions" :key="y" :value="y">{{ y }}</option>
              </select>
            </div>
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
            <div class="grid grid-cols-7 gap-px mb-1">
              <div v-for="(d, i) in ['S','M','T','W','T','F','S']" :key="i"
                class="text-center text-xs text-slate-400 font-medium">{{ d }}</div>
            </div>
            <div class="grid grid-cols-7 gap-px">
              <template v-for="(week, wi) in calendarWeeks" :key="wi">
                <template v-for="(day, di) in week" :key="wi + '-' + di">
                  <div v-if="day === null"></div>
                  <button v-else @click="pickPeriod(day <= 15 ? 1 : 2)"
                    :class="['text-center text-xs py-1.5 rounded transition', dayClasses(day)]">
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
              <th class="px-3 py-3 font-medium">Employee / Category</th>
              <th class="px-3 py-3 font-medium text-right">Rate</th>
              <th class="px-3 py-3 font-medium text-right">Regular</th>
              <th class="px-3 py-3 font-medium text-right">Overtime</th>
              <th class="px-3 py-3 font-medium text-right">Sick</th>
              <th class="px-3 py-3 font-medium text-right">Logged</th>
              <th class="px-3 py-3 font-medium text-right">Other $</th>
              <th class="px-3 py-3 font-medium text-right">Comp $</th>
              <th class="px-3 py-3 font-medium text-right">Coaching $</th>
              <th class="px-3 py-3 font-medium w-32"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <template v-for="emp in visibleEmployees" :key="emp.employee_id">
              <tr class="bg-white">
                <td class="px-3 py-3 text-slate-800 font-medium whitespace-nowrap">
                  {{ emp.employee_name }}
                  <span class="text-slate-400 font-normal">#{{ emp.employee_number }}</span>
                  <span class="ml-1 text-xs text-slate-400 capitalize">({{ emp.pay_method }})</span>
                </td>
                <td></td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumField(emp, "regular_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumField(emp, "overtime_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-800">{{ sumField(emp, "sick_hours").toFixed(2) }}</td>
                <td class="px-3 py-3 text-right text-slate-400">{{ sumField(emp, "logged_hours").toFixed(2) }}</td>
                <td class="px-3 py-2 text-right">
                  <input v-model.number="emp._other" type="number" step="0.01" min="0"
                    class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                </td>
                <td class="px-3 py-2 text-right">
                  <input v-model.number="emp._comp" type="number" step="0.01" min="0"
                    class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                </td>
                <td class="px-3 py-2 text-right">
                  <input v-model.number="emp._coaching" type="number" step="0.01" min="0"
                    class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                </td>
                <td class="px-3 py-2 text-right">
                  <button v-if="dollarsDirty(emp)" @click="saveDollars(emp)" :disabled="emp._savingDollars"
                    class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500 disabled:opacity-50">
                    {{ emp._savingDollars ? "…" : "Save $" }}
                  </button>
                </td>
              </tr>

              <template v-for="c in emp.categories" :key="emp.employee_id + '-' + c.category">
                <tr class="bg-slate-50/50">
                  <td class="pl-8 pr-3 py-2 text-slate-600 capitalize">
                    <button @click="toggleCategoryEntries(emp, c.category)"
                      class="text-slate-400 hover:text-slate-700 mr-1 font-mono">
                      {{ entriesOpen[catKey(emp, c.category)] ? "▾" : "▸" }}
                    </button>
                    {{ c.category }}
                    <span v-if="c.admin_edited && !isDirty(c)" class="ml-1 text-xs text-indigo-500">✎ edited</span>
                  </td>
                  <td class="px-3 py-2 text-right text-slate-500">
                    <span v-if="catRate(emp, c.category) !== null">${{ catRate(emp, c.category).toFixed(2) }}</span>
                    <span v-else class="text-red-400 text-xs">no rate</span>
                  </td>
                  <td class="px-3 py-2 text-right">
                    <input v-model.number="c._regular" type="number" step="0.25" min="0"
                      class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                  </td>
                  <td class="px-3 py-2 text-right">
                    <input v-model.number="c._overtime" type="number" step="0.25" min="0"
                      class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                  </td>
                  <td class="px-3 py-2 text-right">
                    <input v-model.number="c._sick" type="number" step="0.25" min="0"
                      class="w-20 rounded border border-slate-300 px-2 py-1 text-right" />
                  </td>
                  <td class="px-3 py-2 text-right text-slate-500">{{ Number(c.logged_hours).toFixed(2) }}</td>
                  <td colspan="3"></td>
                  <td class="px-3 py-2 text-right whitespace-nowrap">
                    <span v-if="isDirty(c) && isMismatch(c)" class="text-xs text-red-600 mr-1">⚠</span>
                    <button v-if="isDirty(c)" @click="saveCategory(emp, c)" :disabled="c._saving"
                      class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500 disabled:opacity-50">
                      {{ c._saving ? "…" : "Save" }}
                    </button>
                    <button v-else-if="c.admin_edited" @click="revertCategory(emp, c)" :disabled="c._saving"
                      class="text-xs text-slate-400 hover:text-slate-700 px-1" title="Revert to computed">↻</button>
                  </td>
                </tr>
                <tr v-if="entriesOpen[catKey(emp, c.category)]" class="bg-white">
                  <td colspan="10" class="pl-12 pr-3 py-2">
                    <p v-if="entriesLoading[catKey(emp, c.category)]" class="text-xs text-slate-400">Loading…</p>
                    <template v-else>
                      <table class="w-full text-xs mb-2">
                        <tbody>
                          <tr v-for="s in entriesData[catKey(emp, c.category)]" :key="s.id" class="text-slate-500">
                            <td class="py-1 whitespace-nowrap">{{ s.entry_date }}</td>
                            <td class="py-1">{{ s.class_name }}</td>
                            <td class="py-1">Rm {{ s.teacher_room }}</td>
                            <td class="py-1 text-right">{{ s.hours }}h</td>
                            <td class="py-1 text-right w-24">
                              <button @click="openEditEntry(emp, catKey(emp, c.category), s)"
                                class="text-slate-400 hover:text-slate-700 mr-2">edit</button>
                              <template v-if="confirmDeleteId === s.id">
                                <button @click="doDelete(emp, catKey(emp, c.category), s.id)"
                                  class="text-red-600 mr-1">✓</button>
                                <button @click="confirmDeleteId = null" class="text-slate-400">✕</button>
                              </template>
                              <button v-else @click="confirmDeleteId = s.id" class="text-red-500 hover:text-red-700">del</button>
                            </td>
                          </tr>
                          <tr v-if="!entriesData[catKey(emp, c.category)] || entriesData[catKey(emp, c.category)].length === 0">
                            <td colspan="5" class="py-1 text-slate-400">No entries in this category.</td>
                          </tr>
                        </tbody>
                      </table>
                      <button @click="openAddEntry(emp, catKey(emp, c.category), c.category, null)"
                        class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600">+ Add entry</button>
                    </template>
                  </td>
                </tr>
              </template>

              <tr v-if="emp.privates.length" class="bg-slate-50/50">
                <td class="pl-8 pr-3 py-2 text-slate-600">
                  <button @click="emp._privateExpanded = !emp._privateExpanded"
                    class="text-slate-400 hover:text-slate-700 mr-1 font-mono">
                    {{ emp._privateExpanded ? "▾" : "▸" }}
                  </button>
                  private
                  <span class="text-xs text-slate-400">({{ privateHours(emp).toFixed(2) }} hrs)</span>
                </td>
                <td colspan="9"></td>
              </tr>
              <template v-if="emp._privateExpanded">
                <template v-for="p in emp.privates" :key="emp.employee_id + '-priv-' + p.session_duration">
                  <tr class="bg-slate-50">
                    <td class="pl-16 pr-3 py-2 text-slate-500">
                      <button @click="togglePrivateEntries(emp, p.session_duration)"
                        class="text-slate-400 hover:text-slate-700 mr-1 font-mono">
                        {{ entriesOpen[privKey(emp, p.session_duration)] ? "▾" : "▸" }}
                      </button>
                      {{ p.session_duration }} min
                      <span v-if="p.admin_edited && !privateDirty(p)" class="ml-1 text-xs text-indigo-500">✎ edited</span>
                    </td>
                    <td class="px-3 py-2 text-right text-slate-500">
                      <span v-if="privRate(emp, p.session_duration) !== null">${{ privRate(emp, p.session_duration).toFixed(2) }}</span>
                      <span v-else class="text-red-400 text-xs">no rate</span>
                    </td>
                    <td class="px-3 py-2 text-right">
                      <div class="flex items-center justify-end gap-1">
                        <input v-model.number="p._count" type="number" step="1" min="0"
                          class="w-16 rounded border border-slate-300 px-2 py-1 text-right" />
                        <span class="text-xs text-slate-400">sess.</span>
                      </div>
                    </td>
                    <td colspan="3" class="px-3 py-2 text-right text-slate-400 text-xs">
                      logged: {{ ((p.session_duration / 60) * p.logged_count).toFixed(2) }} hrs
                    </td>
                    <td colspan="3"></td>
                    <td class="px-3 py-2 text-right whitespace-nowrap">
                      <span v-if="privateDirty(p) && privateMismatch(p)" class="text-xs text-red-600 mr-1">⚠</span>
                      <button v-if="privateDirty(p)" @click="savePrivate(emp, p)" :disabled="p._saving"
                        class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500 disabled:opacity-50">
                        {{ p._saving ? "…" : "Save" }}
                      </button>
                      <button v-else-if="p.admin_edited" @click="revertPrivate(emp, p)" :disabled="p._saving"
                        class="text-xs text-slate-400 hover:text-slate-700 px-1" title="Revert to computed">↻</button>
                    </td>
                  </tr>
                  <tr v-if="entriesOpen[privKey(emp, p.session_duration)]" class="bg-white">
                    <td colspan="10" class="pl-20 pr-3 py-2">
                      <p v-if="entriesLoading[privKey(emp, p.session_duration)]" class="text-xs text-slate-400">Loading…</p>
                      <template v-else>
                        <table class="w-full text-xs mb-2">
                          <tbody>
                            <tr v-for="s in entriesData[privKey(emp, p.session_duration)]" :key="s.id" class="text-slate-500">
                              <td class="py-1 whitespace-nowrap">{{ s.entry_date }}</td>
                              <td class="py-1">{{ s.class_name }}</td>
                              <td class="py-1">{{ s.session_count }}× {{ s.session_duration }}min</td>
                              <td class="py-1 text-right">{{ s.hours }}h</td>
                              <td class="py-1 text-right w-24">
                                <button @click="openEditEntry(emp, privKey(emp, p.session_duration), s)"
                                  class="text-slate-400 hover:text-slate-700 mr-2">edit</button>
                                <template v-if="confirmDeleteId === s.id">
                                  <button @click="doDelete(emp, privKey(emp, p.session_duration), s.id)"
                                    class="text-red-600 mr-1">✓</button>
                                  <button @click="confirmDeleteId = null" class="text-slate-400">✕</button>
                                </template>
                                <button v-else @click="confirmDeleteId = s.id" class="text-red-500 hover:text-red-700">del</button>
                              </td>
                            </tr>
                            <tr v-if="!entriesData[privKey(emp, p.session_duration)] || entriesData[privKey(emp, p.session_duration)].length === 0">
                              <td colspan="5" class="py-1 text-slate-400">No private entries at this duration.</td>
                            </tr>
                          </tbody>
                        </table>
                        <button @click="openAddEntry(emp, privKey(emp, p.session_duration), 'private', p.session_duration)"
                          class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600">+ Add entry</button>
                      </template>
                    </td>
                  </tr>
                </template>
              </template>
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
              <label class="block text-sm font-medium text-slate-600 mb-1">Class &amp; Time</label>
              <input v-model="entryForm.class_name" type="text" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
            <div class="w-24">
              <label class="block text-sm font-medium text-slate-600 mb-1">Room</label>
              <input v-model="entryForm.teacher_room" type="text" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
            </div>
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Category</label>
            <select v-model="entryForm.category" class="w-full rounded-lg border border-slate-300 px-3 py-2">
              <option :value="null">— none —</option>
              <option v-for="cat in allCategories" :key="cat.id" :value="cat.name" class="capitalize">{{ cat.name }}</option>
            </select>
          </div>
          <div v-if="!entryIsPrivate">
            <label class="block text-sm font-medium text-slate-600 mb-1">Hours</label>
            <input v-model.number="entryForm.hours" type="number" step="0.25" min="0" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div v-else class="flex gap-3">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Duration</label>
              <select v-model.number="entryForm.session_duration" class="w-full rounded-lg border border-slate-300 px-3 py-2">
                <option :value="null" disabled>Length…</option>
                <option v-for="d in [20,30,45,60]" :key="d" :value="d">{{ d }} min</option>
              </select>
            </div>
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1"># Sessions</label>
              <input v-model.number="entryForm.session_count" type="number" step="1" min="1" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
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
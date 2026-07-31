<script setup>
const ADMIN_ID = 3; // STUB: Amanda (admin) on dev
const { current, prev, next, goTo, label, start, end } = usePayPeriod();

const employees = ref([]);
const loading = ref(false);
const error = ref(null);
const payFilter = ref("both");

const showCalendar = ref(false);
const calYear = ref(current.value.year);
const calMonth = ref(current.value.month);

const headers = { "X-Employee-Id": String(ADMIN_ID) };

async function loadTotals() {
  loading.value = true;
  error.value = null;
  try {
    const data = await $fetch("/api/admin/totals", {
      headers,
      query: { period_start: start.value, period_end: end.value },
    });
    employees.value = data.map((emp) => ({
      ...emp,
      _other: Number(emp.other_earn),
      _comp: Number(emp.competition_earn),
      _coaching: Number(emp.coaching_earn),
      _savingDollars: false,
      categories: emp.categories
        .filter((c) => c.category !== "private")
        .map((c) => ({
          ...c,
          _regular: Number(c.regular_hours),
          _overtime: Number(c.overtime_hours),
          _sick: Number(c.sick_hours),
          _saving: false,
        })),
    }));
  } catch (e) {
    error.value = "Could not load totals — are you an admin?";
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

async function saveCategory(emp, c) {
  c._saving = true;
  error.value = null;
  try {
    await $fetch("/api/admin/category-hours", {
      method: "POST", headers,
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
      method: "POST", headers,
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
      method: "POST", headers,
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

onMounted(loadTotals);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-6xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Admin — Period Totals</h1>
        <div class="flex items-center gap-4">
          <NuxtLink to="/" class="text-sm text-slate-500 hover:text-slate-800">← Entry</NuxtLink>
          <NuxtLink to="/admin-users" class="text-sm text-slate-500 hover:text-slate-800">Employees →</NuxtLink>
        </div>
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

              <tr v-for="c in emp.categories" :key="emp.employee_id + '-' + c.category" class="bg-slate-50/50">
                <td class="pl-8 pr-3 py-2 text-slate-600 capitalize">
                  {{ c.category }}
                  <span v-if="c.admin_edited && !isDirty(c)" class="ml-1 text-xs text-indigo-500">✎ edited</span>
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
            </template>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
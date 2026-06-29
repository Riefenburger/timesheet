<script setup>
const ADMIN_ID = 1; // STUB admin identity

const FIELDS = [
  "regular_hours", "overtime_hours", "other_earn",
  "competition_earn", "coaching_earn", "sick_hours",
];

const num = (v) => Number(v) || 0;

const periodStart = ref("2026-04-16");
const periodEnd = ref("2026-04-30");
const rows = ref([]);
const loading = ref(false);
const saving = ref(false);
const error = ref(null);

// Per-employee expand state and their fetched sessions.
const expanded = reactive({});      // employee_id -> true/false
const sessions = reactive({});      // employee_id -> array of entries
const sessionsLoading = reactive({}); // employee_id -> true/false

function snapshot(row) {
  const s = {};
  for (const f of FIELDS) s[f] = num(row[f]);
  return s;
}

async function loadTotals() {
  loading.value = true;
  error.value = null;
  // reset all per-row expand state when reloading a period, so cached
  // sessions from a previous period can't leak into this one
  for (const k of Object.keys(expanded)) delete expanded[k];
  for (const k of Object.keys(sessions)) delete sessions[k];
  for (const k of Object.keys(sessionsLoading)) delete sessionsLoading[k];
  try {
    const data = await $fetch("/api/admin/totals", {
      headers: { "X-Employee-Id": String(ADMIN_ID) },
      query: { period_start: periodStart.value, period_end: periodEnd.value },
    });
    rows.value = data.map((r) => {
      const isNew = r.totals_id === null;
      const logged = r.logged_hours ?? 0;
      const vals = {
        regular_hours: isNew ? logged : (r.regular_hours ?? 0),
        overtime_hours: r.overtime_hours ?? 0,
        other_earn: r.other_earn ?? 0,
        competition_earn: r.competition_earn ?? 0,
        coaching_earn: r.coaching_earn ?? 0,
        sick_hours: r.sick_hours ?? 0,
      };
      return {
        employee_id: r.employee_id,
        employee_name: r.employee_name,
        employee_number: r.employee_number,
        logged_hours: r.logged_hours,
        entered: r.totals_id !== null,
        ...vals,
        _original: { ...vals },
      };
    });
  } catch (e) {
    error.value = "Could not load totals — are you an admin?";
  } finally {
    loading.value = false;
  }
}

// Toggle a row open/closed; fetch its sessions the first time it opens.
async function toggleExpand(row) {
  const id = row.employee_id;
  expanded[id] = !expanded[id];
  if (expanded[id] && sessions[id] === undefined) {
    sessionsLoading[id] = true;
    try {
      sessions[id] = await $fetch(`/api/admin/employees/${id}/entries`, {
        headers: { "X-Employee-Id": String(ADMIN_ID) },
        query: { period_start: periodStart.value, period_end: periodEnd.value },
      });
    } catch (e) {
      sessions[id] = [];
      error.value = "Could not load that employee's sessions.";
    } finally {
      sessionsLoading[id] = false;
    }
  }
}

function isDirty(row) {
  return FIELDS.some((f) => num(row[f]) !== num(row._original[f]));
}

const dirtyRows = computed(() => rows.value.filter(isDirty));

function distributedHours(row) {
  return num(row.regular_hours) + num(row.overtime_hours) + num(row.sick_hours);
}

function reconcile(row) {
  if (row.logged_hours === null || row.logged_hours === undefined) return "none";
  const logged = num(row.logged_hours);
  const dist = distributedHours(row);
  if (Math.abs(logged - dist) < 0.001) return "ok";
  return "mismatch";
}

function mismatchMessage(row) {
  const logged = num(row.logged_hours);
  const dist = distributedHours(row);
  const diff = (logged - dist).toFixed(2);
  return `Logged ${logged.toFixed(2)}h, distributed ${dist.toFixed(2)}h (off by ${diff})`;
}

function reloadPeriod() {
  if (
    dirtyRows.value.length > 0 &&
    !confirm("You have unsaved changes that will be lost. Load anyway?")
  ) {
    return;
  }
  loadTotals();
}

async function saveAll() {
  const toSave = dirtyRows.value;
  if (toSave.length === 0) return;
  saving.value = true;
  error.value = null;
  try {
    for (const row of toSave) {
      await $fetch("/api/admin/totals", {
        method: "POST",
        headers: { "X-Employee-Id": String(ADMIN_ID) },
        body: {
          employee_id: row.employee_id,
          period_start: periodStart.value,
          period_end: periodEnd.value,
          regular_hours: num(row.regular_hours),
          overtime_hours: num(row.overtime_hours),
          other_earn: num(row.other_earn),
          competition_earn: num(row.competition_earn),
          coaching_earn: num(row.coaching_earn),
          sick_hours: num(row.sick_hours),
        },
      });
      row._original = snapshot(row);
      row.entered = true;
    }
  } catch (e) {
    error.value = "Could not save all rows — some may not have saved. Try again.";
  } finally {
    saving.value = false;
  }
}

// Download the period's totals as a CSV file.
async function exportCsv() {
  error.value = null;
  try {
    // Fetch the raw file bytes (a "blob"), not parsed JSON.
    const blob = await $fetch("/api/admin/totals/export", {
      headers: { "X-Employee-Id": String(ADMIN_ID) },
      query: { period_start: periodStart.value, period_end: periodEnd.value },
      responseType: "blob",
    });

    // Turn the blob into a temporary URL and click an invisible link to download it.
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `timesheet_${periodStart.value}_to_${periodEnd.value}.csv`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  } catch (e) {
    error.value = "Could not export — are you an admin?";
  }
}

onMounted(loadTotals);
</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-6xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Admin — Period Totals</h1>
        <NuxtLink to="/admin" class="text-sm text-slate-500 hover:text-slate-800">
          View entries →
        </NuxtLink>
      </div>

      <div class="bg-white rounded-2xl shadow p-6 mb-6 flex items-end gap-4">
        <div>
          <label class="block text-sm font-medium text-slate-600 mb-1">Period start</label>
          <input v-model="periodStart" type="date"
            class="rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <div>
          <label class="block text-sm font-medium text-slate-600 mb-1">Period end</label>
          <input v-model="periodEnd" type="date"
            class="rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <button @click="reloadPeriod"
          class="bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700">
          Load period
        </button>
        <div class="flex-1"></div>
        <button @click="exportCsv"
          class="bg-white text-slate-700 border border-slate-300 rounded-lg px-4 py-2 font-medium hover:bg-slate-50">
          Export CSV
        </button>
        <button @click="saveAll" :disabled="dirtyRows.length === 0 || saving"
          class="bg-emerald-600 text-white rounded-lg px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
          {{ saving ? "Saving…" : `Save all (${dirtyRows.length})` }}
        </button>
      </div>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow overflow-x-auto">
        <p v-if="loading" class="text-slate-400 p-6">Loading…</p>
        <table v-else class="w-full text-sm">
          <thead class="bg-slate-50 text-slate-500 text-left">
            <tr>
              <th class="px-3 py-3 font-medium w-6"></th>
              <th class="px-3 py-3 font-medium">Employee</th>
              <th class="px-3 py-3 font-medium">Logged</th>
              <th class="px-3 py-3 font-medium">Regular</th>
              <th class="px-3 py-3 font-medium">Overtime</th>
              <th class="px-3 py-3 font-medium">Sick</th>
              <th class="px-3 py-3 font-medium">Other $</th>
              <th class="px-3 py-3 font-medium">Comp $</th>
              <th class="px-3 py-3 font-medium">Coaching $</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <template v-for="row in rows" :key="row.employee_id">
              <tr :class="{ 'bg-blue-50': isDirty(row), 'bg-amber-50': reconcile(row) === 'mismatch' }">
                <td class="px-3 py-2 text-center">
                  <button @click="toggleExpand(row)"
                    class="text-slate-400 hover:text-slate-700 w-5 text-2xl leading-none font-mono"
                    :title="expanded[row.employee_id] ? 'Hide sessions' : 'Show sessions'">
                    {{ expanded[row.employee_id] ? "▾" : "▸" }}
                  </button>
                </td>
                <td class="px-3 py-2 text-slate-800 whitespace-nowrap">
                  {{ row.employee_name }}
                  <span class="text-slate-400">#{{ row.employee_number }}</span>
                  <span v-if="!row.entered" class="ml-1 text-xs text-amber-500">(new)</span>
                  <span v-if="isDirty(row)" class="ml-1 text-xs text-blue-500" title="Unsaved">●</span>
                  <div v-if="reconcile(row) === 'mismatch'" class="text-xs text-amber-600 mt-0.5">
                    ⚠ {{ mismatchMessage(row) }}
                  </div>
                </td>
                <td class="px-3 py-2 whitespace-nowrap"
                  :class="reconcile(row) === 'mismatch' ? 'text-amber-700 font-medium' : 'text-slate-500'">
                  {{ row.logged_hours === null ? "—" : Number(row.logged_hours).toFixed(2) }}
                </td>
                <td class="px-3 py-2"><input v-model.number="row.regular_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
                <td class="px-3 py-2"><input v-model.number="row.overtime_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
                <td class="px-3 py-2"><input v-model.number="row.sick_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
                <td class="px-3 py-2"><input v-model.number="row.other_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
                <td class="px-3 py-2"><input v-model.number="row.competition_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
                <td class="px-3 py-2"><input v-model.number="row.coaching_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
              </tr>
              <tr v-if="expanded[row.employee_id]" class="bg-slate-50">
                <td></td>
                <td colspan="8" class="px-3 py-3">
                  <p v-if="sessionsLoading[row.employee_id]" class="text-slate-400 text-xs">Loading sessions…</p>
                  <p v-else-if="!sessions[row.employee_id] || sessions[row.employee_id].length === 0"
                    class="text-slate-400 text-xs">No sessions logged this period.</p>
                  <table v-else class="w-full text-xs">
                    <thead class="text-slate-400 text-left">
                      <tr>
                        <th class="px-2 py-1 font-medium">Date</th>
                        <th class="px-2 py-1 font-medium">Class</th>
                        <th class="px-2 py-1 font-medium">Room</th>
                        <th class="px-2 py-1 font-medium">Details</th>
                        <th class="px-2 py-1 font-medium text-right">Hours</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="s in sessions[row.employee_id]" :key="s.id" class="text-slate-600">
                        <td class="px-2 py-1">{{ s.entry_date }}</td>
                        <td class="px-2 py-1">{{ s.class_name }}</td>
                        <td class="px-2 py-1">{{ s.teacher_room }}</td>
                        <td class="px-2 py-1">{{ s.details }}</td>
                        <td class="px-2 py-1 text-right">{{ s.hours }}</td>
                      </tr>
                    </tbody>
                  </table>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
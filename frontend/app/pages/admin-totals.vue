<script setup>
const ADMIN_ID = 3; // STUB admin identity

const FIELDS = [
  "regular_hours", "overtime_hours", "other_earn",
  "competition_earn", "coaching_earn", "sick_hours",
];

const num = (v) => Number(v) || 0; // coerce blank/NaN inputs to 0

const periodStart = ref("2026-04-16");
const periodEnd = ref("2026-04-30");
const rows = ref([]);
const loading = ref(false);
const saving = ref(false);
const error = ref(null);

// Snapshot a row's six values — used to detect changes later.
function snapshot(row) {
  const s = {};
  for (const f of FIELDS) s[f] = num(row[f]);
  return s;
}

async function loadTotals() {
  loading.value = true;
  error.value = null;
  try {
    const data = await $fetch("/api/admin/totals", {
      headers: { "X-Employee-Id": String(ADMIN_ID) },
      query: { period_start: periodStart.value, period_end: periodEnd.value },
    });
    rows.value = data.map((r) => {
      const vals = {
        regular_hours: r.regular_hours ?? 0,
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
        entered: r.totals_id !== null,
        ...vals,
        _original: { ...vals }, // remember the loaded state
      };
    });
  } catch (e) {
    error.value = "Could not load totals — are you an admin?";
  } finally {
    loading.value = false;
  }
}

// A row is "dirty" if any value differs from what was last loaded or saved.
function isDirty(row) {
  return FIELDS.some((f) => num(row[f]) !== num(row._original[f]));
}

const dirtyRows = computed(() => rows.value.filter(isDirty));

// Reload guard: don't silently discard unsaved edits.
function reloadPeriod() {
  if (
    dirtyRows.value.length > 0 &&
    !confirm("You have unsaved changes that will be lost. Load anyway?")
  ) {
    return;
  }
  loadTotals();
}

// Save only the rows that actually changed.
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
      row._original = snapshot(row); // this row is now clean
      row.entered = true;
    }
  } catch (e) {
    error.value = "Could not save all rows — some may not have saved. Try again.";
  } finally {
    saving.value = false;
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
              <th class="px-3 py-3 font-medium">Employee</th>
              <th class="px-3 py-3 font-medium">Regular</th>
              <th class="px-3 py-3 font-medium">Overtime</th>
              <th class="px-3 py-3 font-medium">Other $</th>
              <th class="px-3 py-3 font-medium">Comp $</th>
              <th class="px-3 py-3 font-medium">Coaching $</th>
              <th class="px-3 py-3 font-medium">Sick</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <tr v-for="row in rows" :key="row.employee_id" :class="{ 'bg-blue-50': isDirty(row) }">
              <td class="px-3 py-2 text-slate-800 whitespace-nowrap">
                {{ row.employee_name }}
                <span class="text-slate-400">#{{ row.employee_number }}</span>
                <span v-if="!row.entered" class="ml-1 text-xs text-amber-500">(new)</span>
                <span v-if="isDirty(row)" class="ml-1 text-xs text-blue-500" title="Unsaved">●</span>
              </td>
              <td class="px-3 py-2"><input v-model.number="row.regular_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
              <td class="px-3 py-2"><input v-model.number="row.overtime_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
              <td class="px-3 py-2"><input v-model.number="row.other_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
              <td class="px-3 py-2"><input v-model.number="row.competition_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
              <td class="px-3 py-2"><input v-model.number="row.coaching_earn" type="number" step="0.01" class="w-24 rounded border border-slate-300 px-2 py-1" /></td>
              <td class="px-3 py-2"><input v-model.number="row.sick_hours" type="number" step="0.25" class="w-20 rounded border border-slate-300 px-2 py-1" /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
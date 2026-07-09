<script setup>
const SUPER_ADMIN_ID = 2; // STUB: Mark (super_admin) on the dev database

const CATEGORIES = ["teaching", "assisting", "office"];

const employees = ref([]);
const loading = ref(false);
const error = ref(null);

const showModal = ref(false);
const editingId = ref(null);
const saving = ref(false);
const modalError = ref(null);

const form = reactive({
  name: "", employee_number: "", email: "",
  role: "user", pay_method: "payroll", is_salaried: false, salary: null,
});

// Rate state (only used in edit mode)
const rates = ref([]);
const ratesLoading = ref(false);
const rateError = ref(null);
const newRate = reactive({ label: "teaching", amount: null });

const headers = { "X-Employee-Id": String(SUPER_ADMIN_ID) };

function roleLabel(r) {
  return { user: "User", admin: "Admin", super_admin: "Super Admin" }[r] || r;
}

async function loadEmployees() {
  loading.value = true;
  error.value = null;
  try {
    employees.value = await $fetch("/api/employees", { headers });
  } catch (e) {
    error.value = "Could not load employees.";
  } finally {
    loading.value = false;
  }
}

function resetForm() {
  form.name = ""; form.employee_number = ""; form.email = "";
  form.role = "user"; form.pay_method = "payroll"; form.is_salaried = false; form.salary = null;
}

function openAdd() {
  editingId.value = null;
  resetForm();
  rates.value = [];
  modalError.value = null;
  showModal.value = true;
}

async function openEdit(emp) {
  editingId.value = emp.id;
  form.name = emp.name;
  form.employee_number = emp.employee_number;
  form.email = emp.email ?? "";
  form.role = emp.role;
  form.pay_method = emp.pay_method;
  form.is_salaried = emp.is_salaried;
  form.salary = emp.salary;
  modalError.value = null;
  showModal.value = true;
  await loadRates(emp.id);
}

function closeModal() {
  showModal.value = false;
  modalError.value = null;
  rateError.value = null;
}

async function saveEmployee() {
  modalError.value = null;
  if (form.is_salaried && (form.salary === null || form.salary === "" || Number(form.salary) <= 0)) {
    modalError.value = "Salaried employees need a salary amount.";
    return;
  }
  const body = {
    name: form.name,
    employee_number: form.employee_number,
    email: form.email.trim() === "" ? null : form.email.trim(),
    role: form.role,
    pay_method: form.pay_method,
    is_salaried: form.is_salaried,
    salary: form.is_salaried ? Number(form.salary) : null,
  };
  saving.value = true;
  try {
    if (editingId.value === null) {
      await $fetch("/api/admin/employees", { method: "POST", headers, body });
    } else {
      await $fetch(`/api/admin/employees/${editingId.value}`, { method: "PUT", headers, body });
    }
    closeModal();
    await loadEmployees();
  } catch (e) {
    modalError.value = (e && e.data) ? String(e.data) : "Could not save. Check the fields and try again.";
  } finally {
    saving.value = false;
  }
}

// ---- Rate management (edit mode only, saves immediately) ----

async function loadRates(employeeId) {
  ratesLoading.value = true;
  rateError.value = null;
  try {
    rates.value = await $fetch(`/api/admin/employees/${employeeId}/rates`, { headers });
  } catch (e) {
    rateError.value = "Could not load rates.";
  } finally {
    ratesLoading.value = false;
  }
}

// Which of the three categories don't yet have a rate (so we don't offer duplicates).
const availableCategories = computed(() =>
  CATEGORIES.filter((c) => !rates.value.some((r) => r.label === c))
);

async function addRate() {
  rateError.value = null;
  if (!newRate.label || newRate.amount === null || Number(newRate.amount) <= 0) {
    rateError.value = "Pick a category and a rate amount.";
    return;
  }
  try {
    await $fetch("/api/admin/rates", {
      method: "POST", headers,
      body: { employee_id: editingId.value, label: newRate.label, amount: Number(newRate.amount) },
    });
    newRate.amount = null;
    await loadRates(editingId.value);
    newRate.label = availableCategories.value[0] ?? "teaching";
  } catch (e) {
    rateError.value = (e && e.data) ? String(e.data) : "Could not add rate.";
  }
}

async function saveRate(rate) {
  rateError.value = null;
  try {
    await $fetch(`/api/admin/rates/${rate.id}`, {
      method: "PUT", headers,
      body: { label: rate.label, amount: Number(rate.amount) },
    });
    await loadRates(editingId.value);
  } catch (e) {
    rateError.value = (e && e.data) ? String(e.data) : "Could not update rate.";
  }
}

async function deleteRate(rate) {
  rateError.value = null;
  try {
    await $fetch(`/api/admin/rates/${rate.id}`, { method: "DELETE", headers });
    await loadRates(editingId.value);
  } catch (e) {
    rateError.value = "Could not delete rate.";
  }
}

onMounted(loadEmployees);

</script>

<template>
  <div class="min-h-screen bg-slate-100 py-10 px-4">
    <div class="max-w-4xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Manage Employees</h1>
        <div class="flex items-center gap-4">
          <NuxtLink to="/admin" class="text-sm text-slate-500 hover:text-slate-800">Entries →</NuxtLink>
          <button @click="openAdd"
            class="bg-emerald-600 text-white rounded-lg px-4 py-2 text-sm font-medium hover:bg-emerald-500">
            + Add Employee
          </button>
        </div>
      </div>

      <p v-if="error" class="text-red-600 mb-4">{{ error }}</p>

      <div class="bg-white rounded-2xl shadow overflow-x-auto">
        <p v-if="loading" class="text-slate-400 p-6">Loading…</p>
        <table v-else class="w-full text-sm">
          <thead class="bg-slate-50 text-slate-500 text-left">
            <tr>
              <th class="px-4 py-3 font-medium">Name</th>
              <th class="px-4 py-3 font-medium">Employee #</th>
              <th class="px-4 py-3 font-medium">Role</th>
              <th class="px-4 py-3 font-medium">Pay Method</th>
              <th class="px-4 py-3 font-medium w-10"></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-100">
            <tr v-for="emp in employees" :key="emp.id" class="group">
              <td class="px-4 py-3 text-slate-800">{{ emp.name }}</td>
              <td class="px-4 py-3 text-slate-600">{{ emp.employee_number }}</td>
              <td class="px-4 py-3 text-slate-600">{{ roleLabel(emp.role) }}</td>
              <td class="px-4 py-3 text-slate-600 capitalize">{{ emp.pay_method }}</td>
              <td class="px-4 py-3">
                <button @click="openEdit(emp)"
                  class="opacity-0 group-hover:opacity-100 transition text-slate-400 hover:text-slate-800"
                  title="Edit employee">
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24"
                    stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round"
                      d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                  </svg>
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div v-if="showModal"
      class="fixed inset-0 bg-black/40 flex items-center justify-center z-50 px-4 py-8 overflow-y-auto"
      @click.self="closeModal">
      <div class="bg-white rounded-2xl shadow-xl p-6 w-full max-w-md my-auto">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">
          {{ editingId === null ? "Add Employee" : "Edit Employee" }}
        </h2>

        <form @submit.prevent="saveEmployee" class="space-y-3">
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Name</label>
            <input v-model="form.name" type="text" required class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Employee #</label>
            <input v-model="form.employee_number" type="text" required class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1">Email (optional)</label>
            <input v-model="form.email" type="email" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div class="flex gap-3">
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Role</label>
              <select v-model="form.role" class="w-full rounded-lg border border-slate-300 px-3 py-2">
                <option value="user">User</option>
                <option value="admin">Admin</option>
                <option value="super_admin">Super Admin</option>
              </select>
            </div>
            <div class="flex-1">
              <label class="block text-sm font-medium text-slate-600 mb-1">Pay Method</label>
              <select v-model="form.pay_method" class="w-full rounded-lg border border-slate-300 px-3 py-2">
                <option value="payroll">Payroll</option>
                <option value="check">Check</option>
              </select>
            </div>
          </div>
          <div class="flex items-center gap-2 pt-1">
            <input v-model="form.is_salaried" type="checkbox" id="salaried" class="rounded" />
            <label for="salaried" class="text-sm text-slate-600">Salaried employee</label>
          </div>
          <div v-if="form.is_salaried">
            <label class="block text-sm font-medium text-slate-600 mb-1">Salary (per period)</label>
            <input v-model.number="form.salary" type="number" step="0.01" min="0" class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>

          <p v-if="modalError" class="text-red-600 text-sm">{{ modalError }}</p>

          <div class="flex justify-end gap-3 pt-2">
            <button type="button" @click="closeModal"
              class="rounded-lg border border-slate-300 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50">Cancel</button>
            <button type="submit" :disabled="saving"
              class="bg-emerald-600 text-white rounded-lg px-4 py-2 text-sm font-medium hover:bg-emerald-500 disabled:opacity-50">
              {{ saving ? "Saving…" : "Save" }}
            </button>
          </div>
        </form>

        <!-- Rates: edit mode only -->
        <div v-if="editingId !== null" class="mt-6 pt-5 border-t border-slate-100">
          <h3 class="text-sm font-semibold text-slate-700 mb-3">Pay Rates</h3>
          <p v-if="ratesLoading" class="text-slate-400 text-sm">Loading rates…</p>
          <template v-else>
            <div v-if="rates.length === 0" class="text-slate-400 text-sm mb-3">No rates set yet.</div>
            <div v-for="rate in rates" :key="rate.id" class="flex items-center gap-2 mb-2">
              <span class="w-24 text-sm text-slate-700 capitalize">{{ rate.label }}</span>
              <span class="text-slate-400">$</span>
              <input v-model.number="rate.amount" type="number" step="0.01" min="0"
                class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
              <button @click="saveRate(rate)"
                class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600">Save</button>
              <button @click="deleteRate(rate)"
                class="text-xs bg-red-500 text-white rounded px-2 py-1 hover:bg-red-600 ml-1">Remove</button>
            </div>

            <div v-if="availableCategories.length > 0" class="flex items-center gap-2 mt-3">
              <select v-model="newRate.label" class="w-24 rounded border border-slate-300 px-2 py-1 text-sm capitalize">
                <option v-for="c in availableCategories" :key="c" :value="c">{{ c }}</option>
              </select>
              <span class="text-slate-400">$</span>
              <input v-model.number="newRate.amount" type="number" step="0.01" min="0" placeholder="0.00"
                class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
              <button @click="addRate"
                class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500">+ Add</button>
            </div>
            <p v-if="rateError" class="text-red-600 text-sm mt-2">{{ rateError }}</p>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
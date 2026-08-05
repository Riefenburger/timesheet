<script setup>
const employees = ref([]);
const loading = ref(false);
const error = ref(null);

// Global category list (for the rate-assignment dropdown + management).
const categories = ref([]);
async function loadCategories() {
  try { categories.value = await $fetch("/api/categories"); }
  catch (e) { categories.value = []; }
}

const showModal = ref(false);
const editingId = ref(null);
const saving = ref(false);
const modalError = ref(null);
const form = reactive({
  name: "", employee_number: "", email: "",
  role: "user", pay_method: "payroll", is_salaried: false, salary: null,
  _hasAccount: false,
});

const rates = ref([]);
const ratesLoading = ref(false);
const rateError = ref(null);

const DURATIONS = [20, 30, 45, 60];

function roleLabel(r) {
  return { user: "User", admin: "Admin", super_admin: "Super Admin" }[r] || r;
}

async function loadEmployees() {
  loading.value = true;
  error.value = null;
  try {
    employees.value = await $fetch("/api/employees");
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
  form._hasAccount = emp.has_account;
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
      await $fetch("/api/admin/employees", { method: "POST", body });
    } else {
      await $fetch(`/api/admin/employees/${editingId.value}`, { method: "PUT", body });
    }
    closeModal();
    await loadEmployees();
  } catch (e) {
    modalError.value = (e && e.data) ? String(e.data) : "Could not save. Check the fields and try again.";
  } finally {
    saving.value = false;
  }
}

// ---- Invite generation ----
const inviteLink = ref(null);
const inviteError = ref(null);
const generatingInvite = ref(false);
const inviteCopied = ref(false);

async function generateInvite() {
  inviteError.value = null;
  inviteLink.value = null;
  inviteCopied.value = false;
  generatingInvite.value = true;
  try {
    const data = await $fetch("/api/admin/invites", {
      method: "POST",
      body: { employee_id: editingId.value },
    });
    // Build the full signup link from the returned token.
    inviteLink.value = `${window.location.origin}/signup?token=${data.token}`;
  } catch (e) {
    inviteError.value = (e && e.data) ? String(e.data) : "Could not generate an invite.";
  } finally {
    generatingInvite.value = false;
  }
}

async function copyInvite() {
  try {
    await navigator.clipboard.writeText(inviteLink.value);
    inviteCopied.value = true;
    setTimeout(() => { inviteCopied.value = false; }, 2000);
  } catch (e) {
    // Clipboard may be unavailable; the link is still visible to copy manually.
  }
}

// ---- Account reset ----
const resetting = ref(false);
const confirmReset = ref(false);

async function resetAccount() {
  inviteError.value = null;
  resetting.value = true;
  try {
    await $fetch(`/api/admin/employees/${editingId.value}/reset-account`, {
      method: "POST",
    });
    // They're now account-less: flip local state so the invite flow reappears.
    form._hasAccount = false;
    confirmReset.value = false;
    inviteLink.value = null;
    await loadEmployees(); // refresh the list's has_account too
  } catch (e) {
    inviteError.value = (e && e.data) ? String(e.data) : "Could not reset the account.";
  } finally {
    resetting.value = false;
  }
}

// ---- Rate management ----
async function loadRates(employeeId) {
  ratesLoading.value = true;
  rateError.value = null;
  try {
    rates.value = await $fetch(`/api/admin/employees/${employeeId}/rates`);
  } catch (e) {
    rateError.value = "Could not load rates.";
  } finally {
    ratesLoading.value = false;
  }
}

// Normal (non-private) rates.
const normalRates = computed(() => rates.value.filter((r) => !r.label.startsWith("private_")));
// Private rates as a map duration -> rate object.
const privateRates = computed(() => {
  const m = {};
  for (const r of rates.value) {
    if (r.label.startsWith("private_")) {
      const dur = Number(r.label.slice("private_".length));
      if (DURATIONS.includes(dur)) m[dur] = r;
    }
  }
  return m;
});
const hasAnyPrivate = computed(() => Object.keys(privateRates.value).length > 0);

// Categories available to ADD as a normal rate (global list minus private minus already-set).
const availableCategories = computed(() =>
  categories.value
    .filter((c) => !c.is_private)
    .map((c) => c.name)
    .filter((name) => !rates.value.some((r) => r.label === name))
);

// Add-rate form.
const newRate = reactive({ label: "", amount: null });
// Private add form: one amount per not-yet-set duration.
const newPrivate = reactive({ 20: null, 30: null, 45: null, 60: null });
const showPrivateAdd = ref(false);

async function addRate() {
  rateError.value = null;
  if (!newRate.label || newRate.amount === null || Number(newRate.amount) <= 0) {
    rateError.value = "Pick a category and a rate amount.";
    return;
  }
  try {
    await $fetch("/api/admin/rates", {
      method: "POST",
      body: { employee_id: editingId.value, label: newRate.label, amount: Number(newRate.amount) },
    });
    newRate.amount = null;
    newRate.label = "";
    await loadRates(editingId.value);
  } catch (e) {
    rateError.value = (e && e.data) ? String(e.data) : "Could not add rate.";
  }
}

// Save all filled-in private duration amounts (add or update).
async function savePrivateRates() {
  rateError.value = null;
  const toSave = DURATIONS.filter((d) => newPrivate[d] !== null && Number(newPrivate[d]) > 0);
  if (toSave.length === 0) { rateError.value = "Enter at least one duration rate."; return; }
  try {
    for (const d of toSave) {
      const existing = privateRates.value[d];
      if (existing) {
        await $fetch(`/api/admin/rates/${existing.id}`, {
          method: "PUT",
          body: { label: `private_${d}`, amount: Number(newPrivate[d]) },
        });
      } else {
        await $fetch("/api/admin/rates", {
          method: "POST",
          body: { employee_id: editingId.value, label: `private_${d}`, amount: Number(newPrivate[d]) },
        });
      }
      newPrivate[d] = null;
    }
    showPrivateAdd.value = false;
    await loadRates(editingId.value);
  } catch (e) {
    rateError.value = (e && e.data) ? String(e.data) : "Could not save private rates.";
  }
}

async function saveRate(rate) {
  rateError.value = null;
  try {
    await $fetch(`/api/admin/rates/${rate.id}`, {
      method: "PUT",
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
    await $fetch(`/api/admin/rates/${rate.id}`, { method: "DELETE" });
    await loadRates(editingId.value);
  } catch (e) {
    rateError.value = "Could not delete rate.";
  }
}

// ---- Global category management modal ----
const showCatModal = ref(false);
const catError = ref(null);
const newCat = reactive({ name: "", is_private: false });
const catSaving = ref(false);

function openCatModal() {
  catError.value = null;
  newCat.name = ""; newCat.is_private = false;
  showCatModal.value = true;
}
function closeCatModal() { showCatModal.value = false; catError.value = null; }

async function addCategory() {
  catError.value = null;
  if (!newCat.name.trim()) { catError.value = "Enter a category name."; return; }
  catSaving.value = true;
  try {
    await $fetch("/api/admin/categories", {
      method: "POST",
      body: { name: newCat.name.trim(), is_private: newCat.is_private },
    });
    newCat.name = ""; newCat.is_private = false;
    await loadCategories();
  } catch (e) {
    catError.value = (e && e.data) ? String(e.data) : "Could not add category.";
  } finally {
    catSaving.value = false;
  }
}
async function deleteCategory(cat) {
  catError.value = null;
  try {
    await $fetch(`/api/admin/categories/${cat.id}`, { method: "DELETE" });
    await loadCategories();
  } catch (e) {
    catError.value = (e && e.data) ? String(e.data) : "Could not delete category.";
  }
}

onMounted(() => { loadEmployees(); loadCategories(); });
</script>

<template>
  <div class="py-10 px-4">
    <div class="max-w-4xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-slate-800">Manage Employees</h1>
        <div class="flex items-center gap-3">
          <button @click="openCatModal"
            class="bg-slate-700 text-white rounded-lg px-4 py-2 text-sm font-medium hover:bg-slate-600">
            + Category
          </button>
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

    <!-- Employee add/edit modal -->
    <div v-if="showModal"
      class="fixed inset-0 bg-black/40 flex items-center justify-center z-50 px-4 py-8 overflow-y-auto"
      @click.self="closeModal">
      <div class="bg-white rounded-2xl shadow-xl p-6 w-full max-w-md my-auto">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">
          {{ editingId === null ? "Add Employee" : "Edit Employee" }}
        </h2>
        <!-- Account status / invite (edit mode only) -->
        <div v-if="editingId !== null" class="mb-4 p-3 rounded-lg bg-slate-50 border border-slate-100">
          <div v-if="form._hasAccount" class="flex items-center justify-between">
            <div class="text-sm text-emerald-600 flex items-center gap-1">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
              </svg>
              Account active
            </div>
            <template v-if="confirmReset">
              <div class="flex items-center gap-2">
                <span class="text-xs text-slate-500">Reset login?</span>
                <button type="button" @click="resetAccount" :disabled="resetting"
                  class="text-xs bg-red-500 text-white rounded px-2 py-1 hover:bg-red-600 disabled:opacity-50">
                  {{ resetting ? "…" : "Yes, reset" }}
                </button>
                <button type="button" @click="confirmReset = false"
                  class="text-xs text-slate-400 hover:text-slate-600">Cancel</button>
              </div>
            </template>
            <button v-else type="button" @click="confirmReset = true"
              class="text-xs text-slate-400 hover:text-red-600">Reset account</button>
          </div>
          <template v-else>
            <div class="flex items-center justify-between">
              <span class="text-sm text-slate-500">No account yet</span>
              <button type="button" @click="generateInvite" :disabled="generatingInvite"
                class="text-xs bg-indigo-600 text-white rounded px-3 py-1.5 hover:bg-indigo-500 disabled:opacity-50">
                {{ generatingInvite ? "Generating…" : "Generate invite link" }}
              </button>
            </div>
            <div v-if="inviteLink" class="mt-3">
              <div class="text-xs text-slate-500 mb-1">Send this link to the employee (valid 7 days):</div>
              <div class="flex items-center gap-2">
                <input :value="inviteLink" readonly
                  class="flex-1 rounded border border-slate-300 px-2 py-1 text-xs bg-white" />
                <button type="button" @click="copyInvite"
                  class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600 shrink-0">
                  {{ inviteCopied ? "Copied!" : "Copy" }}
                </button>
              </div>
            </div>
            <p v-if="inviteError" class="text-red-600 text-xs mt-2">{{ inviteError }}</p>
          </template>
        </div>
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
          <h3 class="text-sm font-semibold text-slate-700 mb-3">Categories &amp; Rates</h3>
          <p v-if="ratesLoading" class="text-slate-400 text-sm">Loading rates…</p>
          <template v-else>
            <div v-if="normalRates.length === 0 && !hasAnyPrivate" class="text-slate-400 text-sm mb-3">
              No categories assigned yet.
            </div>

            <!-- Normal rates -->
            <div v-for="rate in normalRates" :key="rate.id" class="flex items-center gap-2 mb-2">
              <span class="w-24 text-sm text-slate-700 capitalize">{{ rate.label }}</span>
              <span class="text-slate-400">$</span>
              <input v-model.number="rate.amount" type="number" step="0.01" min="0"
                class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
              <button @click="saveRate(rate)" class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600">Save</button>
              <button @click="deleteRate(rate)" class="text-xs bg-red-500 text-white rounded px-2 py-1 hover:bg-red-600 ml-1">Remove</button>
            </div>

            <!-- Private rates (grouped by duration) -->
            <div v-if="hasAnyPrivate" class="mt-3 mb-2">
              <div class="text-xs font-medium text-slate-500 mb-1">Private (per duration)</div>
              <div v-for="d in DURATIONS" :key="d">
                <div v-if="privateRates[d]" class="flex items-center gap-2 mb-2">
                  <span class="w-24 text-sm text-slate-700">{{ d }} min</span>
                  <span class="text-slate-400">$</span>
                  <input v-model.number="privateRates[d].amount" type="number" step="0.01" min="0"
                    class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
                  <button @click="saveRate(privateRates[d])" class="text-xs bg-slate-700 text-white rounded px-2 py-1 hover:bg-slate-600">Save</button>
                  <button @click="deleteRate(privateRates[d])" class="text-xs bg-red-500 text-white rounded px-2 py-1 hover:bg-red-600 ml-1">Remove</button>
                </div>
              </div>
            </div>

            <!-- Add a normal category rate -->
            <div v-if="availableCategories.length > 0" class="flex items-center gap-2 mt-3">
              <select v-model="newRate.label" class="w-28 rounded border border-slate-300 px-2 py-1 text-sm capitalize">
                <option value="" disabled>Category…</option>
                <option v-for="c in availableCategories" :key="c" :value="c">{{ c }}</option>
              </select>
              <span class="text-slate-400">$</span>
              <input v-model.number="newRate.amount" type="number" step="0.01" min="0" placeholder="0.00"
                class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
              <button @click="addRate" class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500">+ Add</button>
            </div>

            <!-- Add / set private rates -->
            <div class="mt-3">
              <button v-if="!showPrivateAdd" @click="showPrivateAdd = true"
                class="text-xs text-indigo-600 hover:text-indigo-800">
                {{ hasAnyPrivate ? "+ Edit private durations" : "+ Add private rates" }}
              </button>
              <div v-else class="border border-slate-200 rounded-lg p-3 mt-1">
                <div class="text-xs font-medium text-slate-500 mb-2">Set a rate for each private duration (leave blank to skip):</div>
                <div v-for="d in DURATIONS" :key="d" class="flex items-center gap-2 mb-2">
                  <span class="w-20 text-sm text-slate-700">{{ d }} min</span>
                  <span class="text-slate-400">$</span>
                  <input v-model.number="newPrivate[d]" type="number" step="0.01" min="0"
                    :placeholder="privateRates[d] ? String(privateRates[d].amount) : '0.00'"
                    class="w-24 rounded border border-slate-300 px-2 py-1 text-sm" />
                </div>
                <div class="flex gap-2 mt-1">
                  <button @click="savePrivateRates" class="text-xs bg-emerald-600 text-white rounded px-2 py-1 hover:bg-emerald-500">Save private</button>
                  <button @click="showPrivateAdd = false" class="text-xs text-slate-400 hover:text-slate-600 px-2 py-1">Cancel</button>
                </div>
              </div>
            </div>

            <p v-if="rateError" class="text-red-600 text-sm mt-2">{{ rateError }}</p>
          </template>
        </div>
      </div>
    </div>

    <!-- Global category management modal -->
    <div v-if="showCatModal"
      class="fixed inset-0 bg-black/40 flex items-center justify-center z-50 px-4 py-8 overflow-y-auto"
      @click.self="closeCatModal">
      <div class="bg-white rounded-2xl shadow-xl p-6 w-full max-w-md my-auto">
        <h2 class="text-lg font-semibold text-slate-800 mb-4">Manage Categories</h2>
        <div class="space-y-2 mb-4">
          <div v-for="cat in categories" :key="cat.id" class="flex items-center justify-between py-1 border-b border-slate-50">
            <span class="text-sm text-slate-700 capitalize">
              {{ cat.name }}
              <span v-if="cat.is_private" class="text-xs text-indigo-500 ml-1">(private)</span>
            </span>
            <button v-if="!cat.is_private" @click="deleteCategory(cat)" class="text-xs text-red-500 hover:text-red-700">delete</button>
          </div>
        </div>
        <div class="flex items-center gap-2 pt-3 border-t border-slate-100">
          <input v-model="newCat.name" type="text" placeholder="New category name"
            class="flex-1 rounded border border-slate-300 px-3 py-2 text-sm" />
          <button @click="addCategory" :disabled="catSaving"
            class="text-sm bg-emerald-600 text-white rounded px-3 py-2 hover:bg-emerald-500 disabled:opacity-50">Add</button>
        </div>
        <p v-if="catError" class="text-red-600 text-sm mt-2">{{ catError }}</p>
        <div class="flex justify-end mt-4">
          <button @click="closeCatModal" class="rounded-lg border border-slate-300 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50">Done</button>
        </div>
      </div>
    </div>
  </div>
</template>
<script setup>

definePageMeta({ layout: false });

const route = useRoute();
const { fetchMe } = useAuth();

const token = ref(route.query.token || "");
const employeeName = ref(null);
const email = ref("");
const password = ref("");
const confirmPassword = ref("");
const error = ref(null);
const checking = ref(true);
const validInvite = ref(false);
const submitting = ref(false);

// Validate the invite on load.
onMounted(async () => {
  if (!token.value) {
    error.value = "No invite token provided.";
    checking.value = false;
    return;
  }
  try {
    const data = await $fetch(`/api/auth/invite/${token.value}`);
    employeeName.value = data.employee_name;
    if (data.email) email.value = data.email;
    validInvite.value = true;
  } catch (e) {
    error.value = "This invite link is invalid or has expired.";
  } finally {
    checking.value = false;
  }
});

async function doSignup() {
  error.value = null;
  if (password.value.length < 8) {
    error.value = "Password must be at least 8 characters.";
    return;
  }
  if (password.value !== confirmPassword.value) {
    error.value = "Passwords don't match.";
    return;
  }
  submitting.value = true;
  try {
    await $fetch("/api/auth/signup", {
      method: "POST",
      body: { token: token.value, email: email.value, password: password.value },
    });
    await fetchMe();          // signup logs them in (cookie set)
    await navigateTo("/");
  } catch (e) {
    error.value = (e && e.data) ? String(e.data) : "Could not complete signup.";
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex items-center justify-center px-4">
    <div class="bg-white rounded-2xl shadow p-8 w-full max-w-sm">
      <h1 class="text-2xl font-bold text-ink-900 mb-2">Set up your account</h1>

      <p v-if="checking" class="text-ink-400 text-sm">Checking your invite…</p>

      <template v-else-if="validInvite">
        <p class="text-ink-500 text-sm mb-6">Welcome, {{ employeeName }}. Create your login below.</p>
        <form @submit.prevent="doSignup" class="space-y-4">
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Email</label>
            <input v-model="email" type="email" required autocomplete="username"
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Password</label>
            <input v-model="password" type="password" required autocomplete="new-password"
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Confirm password</label>
            <input v-model="confirmPassword" type="password" required autocomplete="new-password"
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>
          <button type="submit" :disabled="submitting"
            class="w-full bg-emerald-600 text-white rounded-lg px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
            {{ submitting ? "Creating…" : "Create account" }}
          </button>
        </form>
      </template>

      <p v-else class="text-red-600 text-sm">{{ error }}</p>
    </div>
  </div>
</template>
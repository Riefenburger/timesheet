<script setup>

definePageMeta({ layout: false });

const { login } = useAuth();
const email = ref("");
const password = ref("");
const error = ref(null);
const submitting = ref(false);

async function doLogin() {
  error.value = null;
  submitting.value = true;
  try {
    await login(email.value, password.value);
    await navigateTo("/");
  } catch (e) {
    error.value = "Invalid email or password.";
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex items-center justify-center px-4">
    <div class="bg-white rounded-2xl shadow p-8 w-full max-w-sm">
      <h1 class="text-2xl font-bold text-ink-900 mb-6">Timesheet</h1>
      <form @submit.prevent="doLogin" class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-ink-700 mb-1">Email</label>
          <input v-model="email" type="email" required autocomplete="username"
            class="w-full rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <div>
          <label class="block text-sm font-medium text-ink-700 mb-1">Password</label>
          <input v-model="password" type="password" required autocomplete="current-password"
            class="w-full rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>
        <button type="submit" :disabled="submitting"
          class="w-full bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700 disabled:opacity-50">
          {{ submitting ? "Signing in…" : "Sign in" }}
        </button>
      </form>
    </div>
  </div>
</template>
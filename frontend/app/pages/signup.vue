<script setup>

definePageMeta({ layout: false });

const route = useRoute();
const { fetchMe } = useAuth();

const token = ref(route.query.token || "");
const employeeName = ref(null);
const email = ref("");
const password = ref("");
const confirmPassword = ref("");
const phoneNumber = ref("");
const code = ref("");
// The form is one page; the code field only appears once a code has been sent.
const codeSent = ref(false);
const sending = ref(false);
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

// Everything the account needs, checked before spending a text message.
function validateDetails() {
  if (password.value.length < 8) return "Password must be at least 8 characters.";
  if (password.value !== confirmPassword.value) return "Passwords don't match.";
  if (!phoneNumber.value.trim()) return "Enter your mobile number.";
  return null;
}

async function sendCode() {
  error.value = null;
  const problem = validateDetails();
  if (problem) { error.value = problem; return; }
  sending.value = true;
  try {
    await $fetch("/api/auth/signup/start-verify", {
      method: "POST",
      body: { token: token.value, phone_number: phoneNumber.value },
    });
    codeSent.value = true;
    code.value = "";
  } catch (e) {
    error.value = (e && e.data) ? String(e.data) : "Could not send a code. Check the number and try again.";
  } finally {
    sending.value = false;
  }
}

// Let them fix a mistyped number without reloading the invite.
function changeNumber() {
  codeSent.value = false;
  code.value = "";
  error.value = null;
}

async function doSignup() {
  error.value = null;
  const problem = validateDetails();
  if (problem) { error.value = problem; return; }
  if (!code.value.trim()) { error.value = "Enter the code we texted you."; return; }
  submitting.value = true;
  try {
    await $fetch("/api/auth/signup", {
      method: "POST",
      body: {
        token: token.value,
        email: email.value,
        password: password.value,
        phone_number: phoneNumber.value,
        code: code.value.trim(),
      },
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
            <PasswordInput v-model="password" autocomplete="new-password" />
          </div>
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Confirm password</label>
            <PasswordInput v-model="confirmPassword" autocomplete="new-password" />
          </div>
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Mobile number</label>
            <input v-model="phoneNumber" type="tel" inputmode="tel" required
              autocomplete="tel" placeholder="(555) 123-4567" :disabled="codeSent"
              class="w-full rounded-lg border border-slate-300 px-3 py-2 disabled:bg-slate-50 disabled:text-ink-500" />
            <p class="text-xs text-ink-400 mt-1">
              We'll text you a code to confirm it. You can then sign in with your
              phone instead of a password.
            </p>
          </div>

          <!-- Step 2: only after a code has actually been sent. -->
          <div v-if="codeSent">
            <label class="block text-sm font-medium text-ink-700 mb-1">Verification code</label>
            <input v-model="code" type="text" inputmode="numeric" autocomplete="one-time-code"
              maxlength="10" placeholder="123456"
              class="w-full rounded-lg border border-slate-300 px-3 py-2 tracking-widest" />
            <div class="flex flex-wrap gap-3 mt-1 text-xs">
              <button type="button" @click="sendCode" :disabled="sending"
                class="text-indigo-600 hover:text-indigo-800 disabled:opacity-50">
                {{ sending ? "Sending…" : "Resend code" }}
              </button>
              <button type="button" @click="changeNumber"
                class="text-ink-400 hover:text-ink-700">Use a different number</button>
            </div>
          </div>

          <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>

          <button v-if="!codeSent" type="button" @click="sendCode" :disabled="sending"
            class="w-full bg-emerald-600 text-white rounded-lg px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
            {{ sending ? "Sending code…" : "Send verification code" }}
          </button>
          <button v-else type="submit" :disabled="submitting"
            class="w-full bg-emerald-600 text-white rounded-lg px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
            {{ submitting ? "Creating…" : "Create account" }}
          </button>
        </form>
      </template>

      <p v-else class="text-red-600 text-sm">{{ error }}</p>
    </div>
  </div>
</template>
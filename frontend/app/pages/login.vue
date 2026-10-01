<script setup>

definePageMeta({ layout: false });

const { login, fetchMe } = useAuth();

// Which path the user is on. Email/password is unchanged and remains the
// universal fallback, so it is always one click away from the phone screens.
const mode = ref("choose");   // 'choose' | 'password' | 'phone'

const email = ref("");
const password = ref("");
const error = ref(null);
const submitting = ref(false);

// --- Phone path ---
const phoneNumber = ref("");
const code = ref("");
const codeSent = ref(false);
const sending = ref(false);

function choose(next) {
  mode.value = next;
  error.value = null;
}

async function doLogin() {
  error.value = null;
  submitting.value = true;
  try {
    await login(email.value, password.value);
    await navigateTo("/");
  } catch (e) {
    // The backend distinguishes a deactivated account from a bad password;
    // show whatever it says rather than overwriting it.
    error.value = (e && e.data) ? String(e.data) : "Invalid email or password.";
  } finally {
    submitting.value = false;
  }
}

async function sendCode() {
  error.value = null;
  if (!phoneNumber.value.trim()) { error.value = "Enter your mobile number."; return; }
  sending.value = true;
  try {
    await $fetch("/api/auth/phone/start", {
      method: "POST",
      body: { phone_number: phoneNumber.value },
    });
    // Deliberately advances even for a number with no account: the response is
    // the same either way, so this screen cannot be used to find out who has one.
    codeSent.value = true;
    code.value = "";
  } catch (e) {
    error.value = (e && e.data) ? String(e.data) : "Could not send a code right now.";
  } finally {
    sending.value = false;
  }
}

async function verifyCode() {
  error.value = null;
  if (!code.value.trim()) { error.value = "Enter the code we texted you."; return; }
  submitting.value = true;
  try {
    await $fetch("/api/auth/phone/verify", {
      method: "POST",
      body: { phone_number: phoneNumber.value, code: code.value.trim() },
    });
    await fetchMe();
    await navigateTo("/");
  } catch (e) {
    error.value = (e && e.data) ? String(e.data) : "That code didn't work or has expired.";
  } finally {
    submitting.value = false;
  }
}

function changeNumber() {
  codeSent.value = false;
  code.value = "";
  error.value = null;
}
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex items-center justify-center px-4">
    <div class="bg-white rounded-2xl shadow p-8 w-full max-w-sm">
      <h1 class="text-2xl font-bold text-ink-900 mb-6">Timesheet</h1>

      <!-- Pick a path -->
      <div v-if="mode === 'choose'" class="space-y-3">
        <button type="button" @click="choose('phone')"
          class="w-full bg-slate-800 text-white rounded-lg px-4 py-3 font-medium hover:bg-slate-700">
          Log in with phone number
        </button>
        <button type="button" @click="choose('password')"
          class="w-full rounded-lg border border-slate-300 px-4 py-3 font-medium text-ink-700 hover:bg-slate-50">
          Log in with email &amp; password
        </button>
      </div>

      <!-- Email + password: unchanged -->
      <form v-else-if="mode === 'password'" @submit.prevent="doLogin" class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-ink-700 mb-1">Email</label>
          <input v-model="email" type="email" required autocomplete="username"
            class="w-full rounded-lg border border-slate-300 px-3 py-2" />
        </div>
        <div>
          <label class="block text-sm font-medium text-ink-700 mb-1">Password</label>
          <PasswordInput v-model="password" autocomplete="current-password" />
        </div>
        <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>
        <button type="submit" :disabled="submitting"
          class="w-full bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700 disabled:opacity-50">
          {{ submitting ? "Signing in…" : "Sign in" }}
        </button>
        <button type="button" @click="choose('choose')"
          class="w-full text-sm text-ink-400 hover:text-ink-700 py-1">← Other ways to log in</button>
      </form>

      <!-- Phone: number, then code -->
      <div v-else class="space-y-4">
        <template v-if="!codeSent">
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Phone number</label>
            <input v-model="phoneNumber" type="tel" inputmode="tel" autocomplete="tel"
              placeholder="(555) 123-4567"
              class="w-full rounded-lg border border-slate-300 px-3 py-2" />
          </div>
          <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>
          <button type="button" @click="sendCode" :disabled="sending"
            class="w-full bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700 disabled:opacity-50">
            {{ sending ? "Sending…" : "Send me a code" }}
          </button>
        </template>

        <template v-else>
          <div>
            <label class="block text-sm font-medium text-ink-700 mb-1">Enter the 6-digit code</label>
            <input v-model="code" type="text" inputmode="numeric" autocomplete="one-time-code"
              maxlength="10" placeholder="123456"
              class="w-full rounded-lg border border-slate-300 px-3 py-2 tracking-widest text-center text-lg" />
            <p class="text-xs text-ink-400 mt-1">Texted to {{ phoneNumber }}</p>
          </div>
          <p v-if="error" class="text-red-600 text-sm">{{ error }}</p>
          <button type="button" @click="verifyCode" :disabled="submitting"
            class="w-full bg-slate-800 text-white rounded-lg px-4 py-2 font-medium hover:bg-slate-700 disabled:opacity-50">
            {{ submitting ? "Signing in…" : "Log in" }}
          </button>
          <div class="flex flex-wrap justify-center gap-3 text-xs">
            <button type="button" @click="sendCode" :disabled="sending"
              class="text-indigo-600 hover:text-indigo-800 disabled:opacity-50">
              {{ sending ? "Sending…" : "Resend code" }}
            </button>
            <button type="button" @click="changeNumber"
              class="text-ink-400 hover:text-ink-700">Use a different number</button>
          </div>
        </template>

        <button type="button" @click="choose('password')"
          class="w-full text-sm text-ink-400 hover:text-ink-700 py-1 border-t border-slate-100 pt-3">
          Use email &amp; password instead
        </button>
      </div>
    </div>
  </div>
</template>

import { ref } from "vue";

// Shared auth state across the app.
const user = ref(null);      // { id, name, email, role } or null
const loaded = ref(false);   // have we checked /auth/me yet?

export function useAuth() {
  // Fetch the current user from the session cookie. Called on app load and
  // after login/logout.
  async function fetchMe() {
    try {
      user.value = await $fetch("/api/auth/me");
    } catch (e) {
      user.value = null;
    } finally {
      loaded.value = true;
    }
  }

  async function login(email, password) {
    await $fetch("/api/auth/login", {
      method: "POST",
      body: { email, password },
    });
    await fetchMe();
  }

  async function logout() {
    try {
      await $fetch("/api/auth/logout", { method: "POST" });
    } catch (e) { /* ignore */ }
    user.value = null;
  }

  const isAuthed = () => user.value !== null;
  const isAdmin = () => user.value && (user.value.role === "admin" || user.value.role === "super_admin");
  const isSuperAdmin = () => user.value && user.value.role === "super_admin";

  return { user, loaded, fetchMe, login, logout, isAuthed, isAdmin, isSuperAdmin };
}
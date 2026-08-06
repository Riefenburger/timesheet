export default defineNuxtRouteMiddleware(async (to) => {
  const { user, loaded, fetchMe, isAdmin, isSuperAdmin } = useAuth();

  // Public routes that don't require auth.
  const publicRoutes = ["/login", "/signup"];
  // Normalize trailing slash — prod build may serve "/signup/" while dev serves "/signup".
  const path = to.path.replace(/\/$/, "") || "/";
  if (publicRoutes.includes(path)) return;

  // Make sure we've checked auth state at least once.
  if (!loaded.value) await fetchMe();

  // Not logged in → login page.
  if (!user.value) {
    return navigateTo("/login");
  }

  // Role-gate the admin/super pages.
  const superOnly = ["/super-totals", "/admin-users"];
  const adminOrSuper = ["/admin-totals"];
  if (superOnly.includes(path) && !isSuperAdmin()) {
    return navigateTo("/");
  }
  if (adminOrSuper.includes(path) && !isAdmin()) {
    return navigateTo("/");
  }
});
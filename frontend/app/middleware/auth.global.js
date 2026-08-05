export default defineNuxtRouteMiddleware(async (to) => {
  const { user, loaded, fetchMe, isAdmin, isSuperAdmin } = useAuth();

  // Public routes that don't require auth.
  const publicRoutes = ["/login", "/signup"];
  if (publicRoutes.includes(to.path)) return;

  // Make sure we've checked auth state at least once.
  if (!loaded.value) await fetchMe();

  // Not logged in → login page.
  if (!user.value) {
    return navigateTo("/login");
  }

  // Role-gate the admin/super pages.
  const superOnly = ["/super-totals", "/admin-users"];
  const adminOrSuper = ["/admin-totals"];
  if (superOnly.includes(to.path) && !isSuperAdmin()) {
    return navigateTo("/");
  }
  if (adminOrSuper.includes(to.path) && !isAdmin()) {
    return navigateTo("/");
  }
});
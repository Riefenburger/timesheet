<script setup>
const { user, logout, isAdmin, isSuperAdmin } = useAuth();
const collapsed = ref(false);

async function doLogout() {
  await logout();
  await navigateTo("/login");
}
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex">
    <aside :class="['bg-white border-r border-slate-200 flex flex-col shrink-0 transition-all duration-200',
      collapsed ? 'w-16' : 'w-56']">
      <!-- Header + collapse toggle -->
      <div class="px-3 py-5 border-b border-slate-100 flex items-center justify-between">
        <div v-if="!collapsed" class="min-w-0">
          <div class="text-lg font-bold text-slate-800 truncate">Timesheet</div>
          <div v-if="user" class="text-xs text-slate-400 mt-1 truncate">
            {{ user.name }} · <span class="capitalize">{{ user.role.replace("_", " ") }}</span>
          </div>
        </div>
        <button @click="collapsed = !collapsed"
          class="text-slate-400 hover:text-slate-700 p-1 shrink-0" :title="collapsed ? 'Expand' : 'Collapse'">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
      </div>

      <nav class="flex-1 px-2 py-4 space-y-1">
        <NuxtLink to="/" class="flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-slate-600 hover:bg-slate-100"
          active-class="bg-slate-100 text-slate-900 font-medium" :title="collapsed ? 'My Time' : ''">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span v-if="!collapsed">My Time</span>
        </NuxtLink>

        <NuxtLink v-if="user && user.role === 'admin'" to="/admin-totals"
          class="flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-slate-600 hover:bg-slate-100"
          active-class="bg-slate-100 text-slate-900 font-medium" :title="collapsed ? 'Totals' : ''">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 17V7m4 10V11m4 6V9M5 21h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
          <span v-if="!collapsed">Totals</span>
        </NuxtLink>

        <NuxtLink v-if="isSuperAdmin()" to="/super-totals"
          class="flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-slate-600 hover:bg-slate-100"
          active-class="bg-slate-100 text-slate-900 font-medium" :title="collapsed ? 'Pay & Rates' : ''">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span v-if="!collapsed">Pay &amp; Rates</span>
        </NuxtLink>

        <NuxtLink v-if="isSuperAdmin()" to="/admin-users"
          class="flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-slate-600 hover:bg-slate-100"
          active-class="bg-slate-100 text-slate-900 font-medium" :title="collapsed ? 'Employees' : ''">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M17 20h5v-2a4 4 0 00-3-3.87M9 20H4v-2a4 4 0 013-3.87m6-1.13a4 4 0 10-4-4 4 4 0 004 4z" />
          </svg>
          <span v-if="!collapsed">Employees</span>
        </NuxtLink>
      </nav>

      <div class="px-2 py-4 border-t border-slate-100">
        <button @click="doLogout"
          class="flex items-center gap-3 w-full px-3 py-2 rounded-lg text-sm text-slate-500 hover:bg-slate-100 hover:text-slate-800"
          :title="collapsed ? 'Sign out' : ''">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
          </svg>
          <span v-if="!collapsed">Sign out</span>
        </button>
      </div>
    </aside>

    <main class="flex-1 overflow-x-auto">
      <slot />
    </main>
  </div>
</template>
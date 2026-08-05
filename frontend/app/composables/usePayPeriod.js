// Shared pay-period logic. Supports three period types:
//   'bimonthly' — 1st–15th and 16th–end of month (the default, unchanged)
//   'monthly'   — whole calendar month
//   'weekly'    — Sun–Sat week
// The super-totals page switches type via a frequency selector; other pages use
// the bimonthly default and never change it, so they keep working unchanged.
import { ref, computed } from "vue";

// --- pure date helpers ---
function pad(n) { return String(n).padStart(2, "0"); }
function toISO(year, month, day) {
  // month is 1-based here
  return `${year}-${pad(month)}-${pad(day)}`;
}
function lastDayOfMonth(year, month) {
  // month 1-based; day 0 of next month = last day of this month
  return new Date(year, month, 0).getDate();
}
function monthName(m) {
  return ["", "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"][m];
}

// === BIMONTHLY (original behavior, preserved) =========================
// Period object carries year/month/half so the calendar pickers keep working.
function makePeriod(year, month, half) {
  if (half === 1) {
    return {
      type: "bimonthly", year, month, half,
      start: toISO(year, month, 1),
      end: toISO(year, month, 15),
      label: `${monthName(month)} 1–15, ${year}`,
    };
  } else {
    const last = lastDayOfMonth(year, month);
    return {
      type: "bimonthly", year, month, half,
      start: toISO(year, month, 16),
      end: toISO(year, month, last),
      label: `${monthName(month)} 16–${last}, ${year}`,
    };
  }
}
function bimonthlyForDate(d) {
  const year = d.getFullYear();
  const month = d.getMonth() + 1;
  const half = d.getDate() <= 15 ? 1 : 2;
  return makePeriod(year, month, half);
}
function stepBimonthly(p, dir) {
  let { year, month, half } = p;
  if (dir > 0) {
    if (half === 1) half = 2;
    else { half = 1; month += 1; if (month > 12) { month = 1; year += 1; } }
  } else {
    if (half === 2) half = 1;
    else { half = 2; month -= 1; if (month < 1) { month = 12; year -= 1; } }
  }
  return makePeriod(year, month, half);
}

// === MONTHLY ===========================================================
function makeMonthly(year, month) {
  const last = lastDayOfMonth(year, month);
  return {
    type: "monthly", year, month, half: 1,
    start: toISO(year, month, 1),
    end: toISO(year, month, last),
    label: `${monthName(month)} ${year}`,
  };
}
function monthlyForDate(d) {
  return makeMonthly(d.getFullYear(), d.getMonth() + 1);
}
function stepMonthly(p, dir) {
  let year = p.year, month = p.month + dir;
  if (month > 12) { month = 1; year += 1; }
  if (month < 1) { month = 12; year -= 1; }
  return makeMonthly(year, month);
}

// === WEEKLY (Sun–Sat) ==================================================
// Weekly is anchored to actual dates, so it carries start/end Date objects
// internally via a JS Date for stepping; year/month/half are filled from the
// week's start date to satisfy the shared interface (pickers ignore weekly).
function makeWeekly(startDate) {
  const s = new Date(startDate.getFullYear(), startDate.getMonth(), startDate.getDate());
  const e = new Date(s.getFullYear(), s.getMonth(), s.getDate() + 6);
  const startLabel = `${monthName(s.getMonth() + 1)} ${s.getDate()}`;
  const endLabel = `${monthName(e.getMonth() + 1)} ${e.getDate()}, ${e.getFullYear()}`;
  return {
    type: "weekly",
    year: s.getFullYear(), month: s.getMonth() + 1, half: 1,
    start: toISO(s.getFullYear(), s.getMonth() + 1, s.getDate()),
    end: toISO(e.getFullYear(), e.getMonth() + 1, e.getDate()),
    label: `${startLabel} – ${endLabel}`,
    _weekStart: s, // internal, for stepping
  };
}
function weeklyForDate(d) {
  const dow = d.getDay(); // 0=Sun
  const start = new Date(d.getFullYear(), d.getMonth(), d.getDate() - dow);
  return makeWeekly(start);
}
function stepWeekly(p, dir) {
  const s = p._weekStart;
  const next = new Date(s.getFullYear(), s.getMonth(), s.getDate() + dir * 7);
  return makeWeekly(next);
}

// === Generic dispatch ==================================================
function periodForDate(type, d) {
  if (type === "monthly") return monthlyForDate(d);
  if (type === "weekly") return weeklyForDate(d);
  return bimonthlyForDate(d);
}
function stepPeriod(p, dir) {
  if (p.type === "monthly") return stepMonthly(p, dir);
  if (p.type === "weekly") return stepWeekly(p, dir);
  return stepBimonthly(p, dir);
}

// --- the composable ---
export function usePayPeriod(initialType = "bimonthly") {
  const current = ref(periodForDate(initialType, new Date()));

  const prev = () => { current.value = stepPeriod(current.value, -1); };
  const next = () => { current.value = stepPeriod(current.value, +1); };

  // goTo stays bimonthly (the calendar picker uses it); unchanged signature.
  const goTo = (year, month, half) => { current.value = makePeriod(year, month, half); };

  // New: switch period type — re-snaps to the current period of that type (today).
  const setType = (type) => { current.value = periodForDate(type, new Date()); };

  const label = computed(() => current.value.label);
  const start = computed(() => current.value.start);
  const end = computed(() => current.value.end);
  const type = computed(() => current.value.type);

  return { current, prev, next, goTo, setType, label, start, end, type, makePeriod, monthName };
}
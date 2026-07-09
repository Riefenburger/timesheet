// Shared pay-period logic: semimonthly periods (1st–15th, 16th–end of month).
import { ref, computed } from "vue";

// --- pure date helpers (no reactivity) ---

function pad(n) {
  return String(n).padStart(2, "0");
}

function toISO(year, month, day) {
  // month is 1-based here
  return `${year}-${pad(month)}-${pad(day)}`;
}

function lastDayOfMonth(year, month) {
  // month 1-based; day 0 of next month = last day of this month
  return new Date(year, month, 0).getDate();
}

// Given a year, month (1-based), and half (1 = first, 2 = second),
// return { start, end, label }.
function makePeriod(year, month, half) {
  if (half === 1) {
    return {
      year, month, half,
      start: toISO(year, month, 1),
      end: toISO(year, month, 15),
      label: `${monthName(month)} 1–15, ${year}`,
    };
  } else {
    const last = lastDayOfMonth(year, month);
    return {
      year, month, half,
      start: toISO(year, month, 16),
      end: toISO(year, month, last),
      label: `${monthName(month)} 16–${last}, ${year}`,
    };
  }
}

function monthName(m) {
  return ["", "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"][m];
}

// Which period does a given Date fall in?
function periodForDate(d) {
  const year = d.getFullYear();
  const month = d.getMonth() + 1; // 1-based
  const half = d.getDate() <= 15 ? 1 : 2;
  return makePeriod(year, month, half);
}

// Step a period forward (+1) or backward (-1) by one half-month.
function stepPeriod(p, dir) {
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

// --- the composable ---

export function usePayPeriod() {
  const current = ref(periodForDate(new Date()));

  const prev = () => { current.value = stepPeriod(current.value, -1); };
  const next = () => { current.value = stepPeriod(current.value, +1); };
  const goTo = (year, month, half) => { current.value = makePeriod(year, month, half); };

  const label = computed(() => current.value.label);
  const start = computed(() => current.value.start);
  const end = computed(() => current.value.end);

  return { current, prev, next, goTo, label, start, end, makePeriod, monthName };
}
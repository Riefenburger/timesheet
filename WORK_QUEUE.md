# Fishback Timesheet — Work Queue & Handoff Brief

This file briefs a coding agent (Claude Code) picking up work on this project. Read it fully before making changes. Check items off as you complete them.

**Status:** #2, #4, #5, #7 and #9 are done, verified locally by Rief, and committed on the `fixes-batch` branch (not pushed, not deployed). Remaining, in order: **#1** (in progress), #3, #8, #6.

---

## ⚠️ SAFETY RULES — READ FIRST, NON-NEGOTIABLE

This app is **LIVE IN PRODUCTION** with real employees and real payroll data at `payroll.fishback.studio` (hosted on Railway). Mistakes affect real people's pay. Therefore:

1. **LOCAL ONLY.** Do all work on the local dev machine. Do **NOT** `git push`. Do **NOT** deploy. Rief tests locally and deploys manually only when satisfied.
2. **Work on a NEW branch** off `main` (e.g. `fixes-batch`). Do **NOT** commit to or push the `railway` branch — pushing `railway` auto-deploys to the live site.
3. **NEVER connect to the production (Railway) Postgres.** Test against the LOCAL Postgres only (the dev database in Docker on this machine). Do not run migrations, dumps, or any query against Railway's database.
4. **Wait for Rief to verify each item locally before moving on** where it makes sense. This is a checkpoint-driven collaborator — explain what you changed and how to test it.
5. If a change requires a DB migration, create the migration and run it **against the local dev DB only**. Rief runs it against Railway later, himself.

If you're ever unsure whether an action touches production, STOP and ask.

---

## STACK & REPO LAYOUT

- **Backend:** Rust + Axum + SQLx (compile-time `query!`/`query_as!` macros, offline mode). PostgreSQL 16. Located in `backend/`.
- **Frontend:** Nuxt 4 SPA (`ssr: false`), Vue 3, Tailwind **v4** (CSS-first, `@theme` in `frontend/app/assets/css/main.css`, no config file). Located in `frontend/`.
- **Dev environment:** Windows + WSL2 (Ubuntu). All code edits, `cargo`, `npm`, git happen in WSL. Project at `~/projects/timesheet`.
- **Production:** Railway (three services: backend, frontend, managed Postgres). Frontend service runs Caddy (`frontend/Caddyfile`) serving the SPA and reverse-proxying `/api/*` to the backend over Railway's private network. Custom domain `payroll.fishback.studio`.
- **Repo:** GitHub `git@github.com:Riefenburger/timesheet.git`. Default branch `main`. The `railway` branch is what Railway deploys from (contains the `frontend/Caddyfile` + Railway adaptations).

### Local testing
- Backend: from `backend/`, `cargo run` (listens on :3000). Needs `DATABASE_URL` in `backend/.env` pointing at the LOCAL Postgres.
- Frontend: from `frontend/`, `npm run dev` (dev server on :3001, proxies `/api/**` → `localhost:3000` via `routeRules` in `nuxt.config.ts`).
- Local Postgres runs in Docker (`docker compose` at project root — the DEV compose, not the Railway one).

---

## CRITICAL GOTCHAS (learned the hard way on this project)

- **`sqlx` commands MUST run from `backend/`** (it reads `backend/.env` and finds `migrations/`). Running from repo root gives "error canonicalizing path migrations".
- **After ANY change to a SQLx query** (`query!`/`query_as!`), run `cargo sqlx prepare` (from `backend/`) and commit the updated `.sqlx/` directory. The Docker build uses `SQLX_OFFLINE=true` and needs the cache current, or the build fails.
- **New module?** Add `mod X;` to `backend/src/main.rs` — easy to forget.
- **Axum 0.8 panics on duplicate method+path routes.** Combine handlers on one path: `.route(path, put(...).delete(...))`, not two separate `.route(path, ...)` calls.
- **Migrations are append-only.** Never edit an applied migration; add a new one. Name via `sqlx migrate add <name>` from `backend/`.
- **SQLx nullability:** `(expr) AS "name!"` forces non-null, `AS "name?"` forces nullable. `type` is a Rust keyword → alias as `type AS "entry_type!"`. Nullable DB column ⇒ struct field must be `Option<T>` or the query macro errors.
- **Tailwind v4:** colors are defined as `@theme { --color-<name>: <hex>; }` tokens in `main.css`, which generate `text-<name>`/`bg-<name>` utilities. The app's text uses custom `ink-*` tokens (royal-blue theme) — change those four hex values to retune the whole theme.
- **SPA routing:** deep links (e.g. `/signup?token=...`) require the Caddy `try_files {path} /index.html` fallback (already in `frontend/Caddyfile`). The auth middleware (`frontend/app/middleware/auth.global.js`) normalizes trailing slashes and exempts `/login` and `/signup` as public routes — preserve that.
- **Auth:** cookie-based sessions (no header fallback — the old `X-Employee-Id` backdoor was removed). `CurrentEmployee`, `AdminEmployee`, `SuperAdminEmployee` extractors gate routes. Signup/invite endpoints are PUBLIC (token-authorized, no session) — do not add auth extractors to them.
- **Shell + `$`:** never put argon2 hashes or passwords containing `$` in double quotes (shell expands them). Use single quotes or quoted heredocs.

### Key data model notes
- `employees`: `id`, `name` (single string — see item #8), `employee_number` (nullable — payroll staff have one, check-paid staff don't), `email` (nullable), `password_hash` (nullable — null = no account yet), `role` ('user'|'admin'|'super_admin'), `pay_method` ('payroll'|'check'), `pay_frequency` ('weekly'|'bimonthly'|'monthly'), `is_salaried`, `salary`.
- `employee_rates`: `(employee_id, label, amount)`. **`label` is the category name** (lowercase) OR `private_<minutes>` for private-lesson durations. This is how rates link to categories (by name string, not FK).
- `categories`: has `is_private` and `is_lump_sum` flags. Lump-sum categories pay a flat per-entry amount (the "rate" IS the amount); their dollars flow into "Other $".
- `time_entries`: individual logged entries. `category`, `hours`, `session_duration`/`session_count` (for private lessons), `type` ('worked'|'sick').
- `private_durations`: table of valid private-lesson durations (currently 20/30/60 min) with global default rates.
- Totals computation lives in `backend/src/totals.rs` (`compute_totals()` — reused by the Excel export in `backend/src/export.rs`).

---

## THE WORK QUEUE (in order)

Grouping note: items that touch the same files are ordered together. #2/#3/#8 all touch employee editing (`admin-users.vue` + `employees.rs`). #7/#8 both touch the Excel export (`export.rs`). Do the schema/name work (#8) thoughtfully since it ripples into the export and employee UI.

### [x] #2 — FIX: Saving employee edits does nothing — DONE (9c6cfd7)
**Cause:** `openEdit` copied the now-nullable `employee_number` straight through, so an employee with no number left `form.employee_number` as `null`; `saveEmployee` then called `.trim()` on it from *outside* the `try`, throwing before any PUT was sent — no error, no spinner, dead button. A regression from 39209c0. Fixed by coercing on load, normalizing both nullable fields, moving body construction inside the `try`, and surfacing non-API errors instead of swallowing them.
**Bug:** In the admin Employees page, editing an employee, clicking Save, and closing the popup does not persist changes, and the employee list doesn't update.
**Expected:** Clicking Save writes the edited fields via the update endpoint, closes the popup, and the list reflects the changes (e.g. a changed name shows immediately).
**Where:** `frontend/app/pages/admin-users.vue` (the `saveEmployee` handler + edit modal) and the backend `update_employee` in `backend/src/employees.rs`. Diagnose whether saveEmployee calls the PUT endpoint, sends the right body, and refreshes the list afterward. Likely a missing/incorrect update call or a missing list-refresh.

### [x] #4 — FIX: Invite link shows stale value across employees — DONE (e96cd99)
**Cause:** the modal's markup is conditional but its state lives in the page scope, which stays mounted — so nothing reset on close and *every* ref leaked to the next employee. Fixed with `resetModalState()`, called from `openAdd`, `openEdit` (before the form is filled) and `closeModal`, covering all eight leaky refs. Two worse leaks from the same cause went with it: `confirmReset` stayed armed (opening another employee showed "Yes, reset" already armed — one click from wiping the wrong login), and the private-rate panel carried over prefilled with the previous employee's amounts.
**Bug:** After clicking "Generate link" for employee A, opening employee B's edit popup still shows A's generated link.
**Expected:** The generated invite link should clear whenever the edit popup closes/opens. Each employee starts with NO link shown; you must click "Generate link" to produce one. So opening any employee (A again, or B) shows no link until generated.
**Where:** `admin-users.vue` — the `inviteLink` ref (or equivalent) isn't reset on modal open/close. Reset it when the modal opens and when it closes.

### [x] #5 — FEATURE: Password show/hide (eye icon) — DONE (772044b)
Extracted `frontend/app/components/PasswordInput.vue` (first file in `app/components/`, auto-imported) and used it for all three fields: login, signup password, signup confirm — each toggling independently. 44px tap target, always visible (not hover-revealed), `mousedown.prevent` so tapping the eye on mobile doesn't dismiss/reopen the keyboard, aria-label/aria-pressed, and a scoped `::-ms-reveal` rule so Edge doesn't render a second eye.
Add a show/hide toggle (eye icon) to password fields on **login** and **signup** pages so users can reveal what they typed. Must work on desktop AND mobile. Toggles the input `type` between `password` and `text`.
**Where:** `frontend/app/pages/login.vue`, `frontend/app/pages/signup.vue`.

### [x] #7 — FIX: Excel export puts all hours in the first rate row — DONE (321a530)
Each category's hours now land on the row of the rate they were logged under (rate `label` == category name, matched case-insensitively + trimmed), via a pure `build_block()` in `export.rs` with 16 unit tests.

**Decisions made (confirmed with Rief):**
- **Private lessons** show a session **count** in the Regular column, labelled `Private Rate 1: 22.50 (per session)` with their own counter. The sheet does no arithmetic — a person reads it and runs payroll by hand — so hours and counts share the column and the row label distinguishes them.
- **⚠️ Consequence:** private *hours* are no longer folded into Regular. For anyone teaching privates the Regular column is lower than on earlier sheets, and lower than the totals screen shows, by exactly their private hours. The screen still adds private hours to its Regular column; that difference is by design.
- **Reconciliation backstop:** hours are now routed to specific rows, so anything unmatched would vanish and underpay someone. Categories with hours no rate claims get an explicit `(no rate)` row, private durations with sessions but no rate likewise, and any residue lands on an `(unassigned)` row. If an `(unassigned)` row ever appears on a real sheet, that's a routing bug — investigate.
- Sick stays employee-level (first row); zero-hour rate rows render blank; rate order stays `employee_rates.id` (Hourly = lowest id); lump-sum rates get no row (their dollars ride in Other $$).
- Two pre-existing quirks resolved themselves: export and screen can no longer disagree about private figures (both key off the same per-duration `session_count`), and `private_overtime_hours` never reaching the sheet stopped mattering (the private reg/OT split is no longer used).
- Also **dropped the Run Number prompt** — the header cell is left blank for the client to fill in, like Check Date and Run Date.

`backend/scripts/seed_test_period.sql` seeds the local DB with the cases worth eyeballing (multi-rate with OT on the second rate, two private durations, orphan hours, salaried). Local-only: refuses to run unless the DB is the dev `timesheet` one with a dev-sized employee count.

### [x] #9 — FEATURE: Export options popup (added mid-session) — DONE (339ad64)
"Export to Excel" on super-totals now opens a popup with Pay Frequency + Pay Method dropdowns instead of silently using the screen filters. Defaults to the screen's current filters and re-syncs on every open. Shows a live count and the matching employee names with their pay method, so the group is confirmed before download; Export is disabled at zero matches. The predicate is extracted as `matchesFilters()` and shared with `visibleEmployees` so the preview can't drift from the backend's own filter. Date range is untouched — the export still covers the period on screen, and the popup displays that period, because the screen's frequency dropdown *also* switches the displayed range while the popup only filters.
**Bug:** In the isolved-format Excel export, all of an employee's hours land on the first rate's row (Hourly), leaving the other rate rows blank.
**Expected:** Each category's hours should appear on the row of the rate they were logged under. So teaching hours (Rate 1/Hourly) go on the Hourly row, office hours (Rate 2) on the Rate 2 row, etc. Each rate row carries the Regular/OT/etc. hours worked at THAT rate.
**Where:** `backend/src/export.rs`. This is a rework of the per-employee block layout: currently it puts the name+rates stacked in column A with all totals on the first row. Now each rate row needs its own hours cells populated from that category's computed hours. Note the rate→category mapping: rate `label` = category name; match each rate row to its category's hours from the totals. (Confirmed intent: hours split across rate rows by the category/rate they were entered under.)

### [ ] #1 — FEATURE: Edit & delete personal time entries
Users currently cannot fix or remove a mis-entered time entry on the "My Time" page.
**Add:** an Edit affordance and a Delete affordance on each entry in the user's own entries list.
- **Edit:** opens the entry's values in the form (or an inline editor), submits an update.
- **Delete:** removes the entry (with a confirm).
- **Backend:** add update + delete endpoints for time entries, gated so a user can only edit/delete **their own** entries (use `CurrentEmployee`, check `employee_id` matches). Admins may already have edit paths — don't disturb those; this is for the personal "My Time" flow.
**Where:** `frontend/app/pages/index.vue` (My Time), `backend/src/entries.rs`.

### [ ] #3 — FEATURE: Deactivate (and optionally delete) an employee
Add the ability to **deactivate** an employee (primary action) with an **option to delete** (secondary).
- **Deactivate:** preferred for payroll — hides them from active lists but KEEPS their historical data (entries, rates) for records. Add an `is_active` (or `deactivated_at`) column; deactivated employees are filtered out of the normal employee/totals lists but their data remains. Provide a way to reactivate.
- **Delete:** a true hard-delete option (with a strong confirm) for cases where an employee was created in error and has no real history. Decide cascade behavior for their rates/entries.
**Where:** `admin-users.vue`, `backend/src/employees.rs`, + a migration for `is_active`. Consider how deactivated employees interact with totals/export (exclude by default).

### [ ] #8 — FEATURE: Split name into First / Last / Middle-initial
Currently `employees.name` is one string. Change to **first_name**, **last_name**, and **middle_initial** (optional).
- **Schema:** migration adding `first_name`, `last_name`, `middle_initial` (nullable). Decide whether to keep `name` as a generated/derived column or drop it.
- **⚠️ DATA MIGRATION WRINKLE:** the ~35 existing employees have inconsistent single-string `name` formats — payroll staff are mostly `"Last, First MI"` while check-paid staff are `"First Last"`. Splitting existing names automatically is error-prone. Recommend: write the migration to add the columns, then handle back-filling carefully (possibly a one-time script Rief reviews row-by-row, or leave existing rows for manual cleanup via the UI). **Do NOT guess-split live payroll names silently.** Discuss the back-fill approach before running it.
- **Employee form:** First, Last, optional Middle initial fields (replaces the single name field). Ties into #2/#3 (same modal).
- **Excel export (`export.rs`):** format the employee name as **`Last, MI First`** — e.g. `"Doherty, J Mark"`, or `"Doherty, Mark"` when no middle initial. (This replaces dumping the single `name` field.)
- **Everywhere `name` is displayed** (employee list, totals, entries): update to render from the new fields.

### [ ] #6 — FEATURE (LARGE, DO LAST, likely its own session): SMS / phone-number login
Let users log in via phone number → receive a texted one-time code → enter code to authenticate. Foundational for a future parent pay-portal (where phone number IS the account identity).
- **Provider:** Twilio (most reliable). Requires a Twilio account, a sending number, US A2P 10DLC registration, and per-message cost (~$0.008/SMS + number + carrier fees). Verify current pricing at build time.
- **Schema:** `phone_number` on employees (nullable, unique); a table for pending one-time codes (code, expiry, phone, attempts).
- **Backend:** endpoint to request a code (rate-limited, generates + stores code, sends SMS via Twilio), endpoint to verify a code (checks + expires it, creates session). Rate-limit both (cost + brute-force protection).
- **Frontend:** phone-login UI (enter number → enter code).
- **Long-lived session:** make the session cookie long-lived (e.g. 30–90 days, refreshed on use) so users text-to-login rarely, not every visit — saves SMS cost and reduces friction. (This is a deliberate change to session expiry.)
- **Note:** existing employees have no phone numbers stored yet — this login can't identify anyone until phones are added.
- Because this needs a paid provider, security care, and is the seed of the parent-portal auth, treat it as its own focused piece. Do the other items first.

---

## DEPLOY FLOW (for reference — Rief does this, not the agent)
When Rief is ready to ship verified changes:
1. Merge the fixes branch → `main` (or into `railway`).
2. Push the `railway` branch → Railway auto-builds & deploys.
3. DB migrations: run against Railway's Postgres manually (Rief re-enables the DB's public TCP proxy temporarily, runs `sqlx migrate run` with the public URL from `backend/`, then disables the proxy again). The agent does NOT do this.

## CHECKING IN
Rief may take specific changes back to a separate planning chat (which has the full project history) for review/verification — especially the tricky ones (#7 export rework, #8 name split/migration, #6 SMS auth). Leave clear notes on what changed and why so that review is easy.

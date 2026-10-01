//! Employee name formatting.
//!
//! Employees are stored as `first_name` / `last_name` / `middle_initial`, but the
//! back-fill from the old single `name` column is done by hand through the admin
//! UI after deploy — so during that window a row may have no parts yet. Every
//! formatter here takes the legacy string too and falls back to it, which is what
//! keeps screens and the payroll sheet correct mid-back-fill.
//!
//! Two formats, because they are genuinely different:
//!   * `roster_name` — "Last, MI First". Admin lists, totals screens, admin
//!     entries and the Excel export, so the screens and the sheet match exactly.
//!   * `personal_name` — "First Last". Addressing the person (nav, My Time
//!     header, signup welcome), where "Doherty, J Mark" would read strangely.

/// Treat absent, empty and whitespace-only the same: not set.
fn clean(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

/// The legacy string, or an explicit placeholder so a nameless row can never
/// render as a blank cell on a payroll sheet.
fn fallback(legacy: Option<&str>) -> String {
    clean(legacy).unwrap_or("(no name)").to_string()
}

/// "Last, MI First" — e.g. "Doherty, J Mark", or "Doherty, Mark" with no middle
/// initial. Falls back to the legacy string, **verbatim**: a legacy value is
/// never re-parsed, because guessing at how a real payroll name splits is
/// exactly what the manual back-fill exists to avoid.
pub fn roster_name(
    first: Option<&str>,
    middle_initial: Option<&str>,
    last: Option<&str>,
    legacy: Option<&str>,
) -> String {
    match (clean(first), clean(last)) {
        (Some(f), Some(l)) => match clean(middle_initial) {
            Some(mi) => format!("{}, {} {}", l, mi, f),
            None => format!("{}, {}", l, f),
        },
        _ => fallback(legacy),
    }
}

/// "First Last" — how the person is addressed. The middle initial is omitted.
pub fn personal_name(
    first: Option<&str>,
    _middle_initial: Option<&str>,
    last: Option<&str>,
    legacy: Option<&str>,
) -> String {
    match (clean(first), clean(last)) {
        (Some(f), Some(l)) => format!("{} {}", f, l),
        _ => fallback(legacy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convenience: the real call sites pass Option<&str> off Option<String>.
    fn roster(f: Option<&str>, mi: Option<&str>, l: Option<&str>, legacy: Option<&str>) -> String {
        roster_name(f, mi, l, legacy)
    }
    fn personal(f: Option<&str>, mi: Option<&str>, l: Option<&str>, legacy: Option<&str>) -> String {
        personal_name(f, mi, l, legacy)
    }

    #[test]
    fn roster_with_middle_initial() {
        assert_eq!(roster(Some("Mark"), Some("J"), Some("Doherty"), None), "Doherty, J Mark");
    }

    #[test]
    fn roster_without_middle_initial() {
        assert_eq!(roster(Some("Mark"), None, Some("Doherty"), None), "Doherty, Mark");
    }

    #[test]
    fn personal_ignores_the_middle_initial() {
        assert_eq!(personal(Some("Mark"), Some("J"), Some("Doherty"), None), "Mark Doherty");
    }

    // --- The formats the local demo data does not contain ---

    #[test]
    fn multi_word_surname_is_kept_whole() {
        // Exactly the shape an automatic splitter would have mangled; here it is
        // stored as one last_name and must survive untouched.
        assert_eq!(roster(Some("Anna"), None, Some("Van Der Berg"), None), "Van Der Berg, Anna");
        assert_eq!(personal(Some("Anna"), None, Some("Van Der Berg"), None), "Anna Van Der Berg");
        assert_eq!(
            roster(Some("Maria"), Some("C"), Some("Del Carmen Ruiz"), None),
            "Del Carmen Ruiz, C Maria"
        );
    }

    #[test]
    fn suffixes_ride_along_with_whichever_part_holds_them() {
        assert_eq!(roster(Some("Mark"), None, Some("Doherty Jr"), None), "Doherty Jr, Mark");
        assert_eq!(roster(Some("Robert"), Some("L"), Some("Stone III"), None), "Stone III, L Robert");
        // A compound given name is one first_name, not two fields.
        assert_eq!(roster(Some("Mary Jane"), None, Some("Watson"), None), "Watson, Mary Jane");
    }

    #[test]
    fn hyphenated_and_apostrophe_names_are_untouched() {
        assert_eq!(roster(Some("Anne"), None, Some("Smith-Jones"), None), "Smith-Jones, Anne");
        assert_eq!(roster(Some("Sean"), None, Some("O'Brien"), None), "O'Brien, Sean");
    }

    // --- Legacy fallback: the mid-back-fill window ---

    #[test]
    fn unsplit_row_falls_back_to_the_legacy_string_verbatim() {
        // A payroll-style stored name. It must come out EXACTLY as stored — the
        // formatter must never try to parse it into parts.
        assert_eq!(roster(None, None, None, Some("Doherty, J Mark")), "Doherty, J Mark");
        assert_eq!(personal(None, None, None, Some("Doherty, J Mark")), "Doherty, J Mark");
        // A check-paid-style stored name, likewise verbatim — note roster does
        // NOT reorder it, because it has no way to know which word is the surname.
        assert_eq!(roster(None, None, None, Some("Mark Doherty")), "Mark Doherty");
    }

    #[test]
    fn legacy_is_ignored_once_the_row_is_split() {
        // Both present during the transition: the parts win.
        assert_eq!(
            roster(Some("Mark"), Some("J"), Some("Doherty"), Some("Doherty, J Mark")),
            "Doherty, J Mark"
        );
        assert_eq!(
            personal(Some("Mark"), None, Some("Doherty"), Some("Mark Doherty")),
            "Mark Doherty"
        );
    }

    #[test]
    fn a_half_filled_row_still_falls_back() {
        // The name_parts_paired CHECK should prevent this reaching the database,
        // so this is belt and braces: never render "Doherty, " or a bare first name.
        assert_eq!(roster(Some("Mark"), None, None, Some("Mark Doherty")), "Mark Doherty");
        assert_eq!(roster(None, None, Some("Doherty"), Some("Mark Doherty")), "Mark Doherty");
        assert_eq!(personal(Some("Mark"), None, None, Some("Mark Doherty")), "Mark Doherty");
    }

    #[test]
    fn blank_and_whitespace_parts_count_as_absent() {
        assert_eq!(roster(Some(""), None, Some(""), Some("Mark Doherty")), "Mark Doherty");
        assert_eq!(roster(Some("   "), None, Some("  "), Some("Mark Doherty")), "Mark Doherty");
        // A blank middle initial must not leave a double space.
        assert_eq!(roster(Some("Mark"), Some(""), Some("Doherty"), None), "Doherty, Mark");
        assert_eq!(roster(Some("Mark"), Some("  "), Some("Doherty"), None), "Doherty, Mark");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(roster(Some(" Mark "), Some(" J "), Some(" Doherty "), None), "Doherty, J Mark");
        assert_eq!(roster(None, None, None, Some("  Mark Doherty  ")), "Mark Doherty");
    }

    #[test]
    fn nothing_at_all_renders_a_placeholder_not_a_blank_cell() {
        assert_eq!(roster(None, None, None, None), "(no name)");
        assert_eq!(roster(None, None, None, Some("   ")), "(no name)");
        assert_eq!(personal(None, None, None, None), "(no name)");
    }

    #[test]
    fn non_ascii_names_are_unaffected() {
        assert_eq!(roster(Some("José"), Some("Á"), Some("Núñez"), None), "Núñez, Á José");
    }
}

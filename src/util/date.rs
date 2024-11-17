use std::cmp::Ordering;
use std::fmt;
use anyhow::{bail, Error};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Date {
    year: u32,
    month: u8,
    day: u8,
}

/// Contains the number of days between two dates, always in positive terms.
/// Designed for convenient printing in human-readable terms.
pub struct Duration {
    years: u32,
    months: u8,
    days: u8,
    total_days: u32,
}

impl Date {
    /// Constructor to parse a string in the "YYYY-mm-dd" format
    pub fn from_str(date_str: &str) -> Result<Date, Error> {
        let parts: Vec<&str> = date_str.split('-').collect();
        if parts.len() != 3 {
            bail!("Date format must be YYYY-MM-DD");
        }

        let year = parts[0].parse::<u32>()?;
        let month = parts[1].parse::<u8>()?;
        let day = parts[2].parse::<u8>()?;

        // Validate the date
        if !Date::is_valid_date(year, month, day) {
            bail!("Invalid date");
        }

        Ok(Date { year, month, day })
    }

    /// Calculate the duration in calendar years, months, and days, and the
    /// total number of days, between two dates
    pub fn duration_between(&self, other: &Date) -> Duration {
        let (earlier, later) = if self < other {
            (self, other)
        } else {
            (other, self)
        };

        let mut year_diff = later.year as i32 - earlier.year as i32;
        let mut month_diff = later.month as i32 - earlier.month as i32;
        let mut day_diff = later.day as i32 - earlier.day as i32;

        if day_diff < 0 {
            month_diff -= 1;
            let days_in_prev_month = Date::days_in_month(
                earlier.year, earlier.month,
            );
            day_diff += days_in_prev_month as i32;
        }

        if month_diff < 0 {
            year_diff -= 1;
            month_diff += 12;
        }

        let total_days = Date::days_between(earlier, later);

        Duration {
            years: year_diff as u32,
            months: month_diff as u8,
            days: day_diff as u8,
            total_days,
        }
    }

    /// Calculate the total number of days between two dates
    fn days_between(start: &Date, end: &Date) -> u32 {
        let days_in_start_year = Date::days_since_year_start(
            start.year, start.month, start.day,
        );
        let days_in_end_year = Date::days_since_year_start(
            end.year, end.month, end.day,
        );

        let days_in_full_years = (start.year..end.year)
            .map(|year| if Date::is_leap_year(year) { 366 } else { 365 })
            .sum::<u32>();

        days_in_full_years + days_in_end_year - days_in_start_year
    }

    /// Calculate the number of days since the start of the given year
    fn days_since_year_start(year: u32, month: u8, day: u8) -> u32 {
        let mut days = 0;
        for m in 1..month {
            days += Date::days_in_month(year, m) as u32;
        }
        days + day as u32
    }

    fn is_leap_year(year: u32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    fn days_in_month(year: u32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Date::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            _ => 0, // Invalid month
        }
    }

    fn is_valid_date(year: u32, month: u8, day: u8) -> bool {
        if month < 1 || month > 12 {
            return false;
        }
        if day < 1 || day > Date::days_in_month(year, month) {
            return false;
        }
        true
    }
}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.year, self.month, self.day)
            .cmp(&(other.year, other.month, other.day))
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_date() {
        let date1 = Date::from_str("2024-11-15").unwrap();
        let date2 = Date::from_str("2024-11-15").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 0, 0, 0));
    }

    #[test]
    fn test_one_day_difference() {
        let date1 = Date::from_str("2024-11-15").unwrap();
        let date2 = Date::from_str("2024-11-16").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 0, 1, 1));
    }

    #[test]
    fn test_one_month_difference() {
        let date1 = Date::from_str("2024-11-15").unwrap();
        let date2 = Date::from_str("2024-12-15").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 1, 0, 30));
    }

    #[test]
    fn test_one_year_difference() {
        let date1 = Date::from_str("2024-11-15").unwrap();
        let date2 = Date::from_str("2025-11-15").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (1, 0, 0, 365));
    }

    #[test]
    fn test_leap_year() {
        let date1 = Date::from_str("2024-02-28").unwrap();
        let date2 = Date::from_str("2024-03-01").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 0, 2, 2));
    }

    #[test]
    fn test_crossing_year_boundary() {
        let date1 = Date::from_str("2023-12-30").unwrap();
        let date2 = Date::from_str("2024-01-02").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 0, 3, 3));
    }

    #[test]
    fn test_large_difference() {
        let date1 = Date::from_str("2000-01-01").unwrap();
        let date2 = Date::from_str("2024-11-15").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (24, 10, 14, 9085));
    }

    #[test]
    fn test_reverse_order() {
        let date1 = Date::from_str("2025-11-17").unwrap();
        let date2 = Date::from_str("2024-11-15").unwrap();
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (1, 0, 2, 367));
    }

    #[test]
    fn test_end_of_month() {
        let date1 = Date::from_str("2024-01-31").unwrap();
        let date2 = Date::from_str("2024-02-29").unwrap(); // Leap year
        let result = date1.duration_between(&date2);
        assert_eq!((result.years, result.months, result.days, result.total_days), (0, 0, 29, 29));
    }
}

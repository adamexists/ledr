use std::cmp::Ordering;
use std::fmt;
use anyhow::{bail, Error};

#[derive(Debug, PartialEq, Eq)]
pub struct Date {
    year: u32,
    month: u8,
    day: u8,
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

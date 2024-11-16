use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg};
use anyhow::{bail, Error};

#[derive(Clone, Copy, Default, Hash)]
pub struct Money {
    amount: i64,
    resolution: u32,
}

pub const ZERO: Money = Money {
    amount: 0,
    resolution: 0,
};

impl Money {
    pub fn new(amount: &str) -> Result<Self, Error> {
        // Split the input string by the decimal point, if it exists
        let parts: Vec<&str> = amount.split('.').collect();
        let (amt, resolution) = match parts.len() {
            1 => {
                let amount = parts[0].parse::<i64>()?;
                (amount, 0)
            }
            2 => {
                let whole_part = parts[0];
                let decimal_part = parts[1];
                let resolution = decimal_part.len() as u32;
                let amount_str = format!("{}{}", whole_part, decimal_part);
                let amount = amount_str.parse::<i64>()?;
                (amount, resolution)
            }
            _ => bail!("could not parse amount"),
        };

        Ok(Self {
            amount: amt,
            resolution,
        })
    }

    fn align_resolution(&self, other: &Money) -> (i64, i64, u32) {
        let max_resolution = self.resolution.max(other.resolution);
        let factor_self = 10i64.pow(max_resolution - self.resolution);
        let factor_other = 10i64.pow(max_resolution - other.resolution);

        (
            self.amount * factor_self,
            other.amount * factor_other,
            max_resolution,
        )
    }

    fn to_f64(&self) -> f64 {
        self.amount as f64 / 10f64.powi(self.resolution as i32)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut amount_str = self.amount.abs().to_string();

        if self.resolution > 0 {
            // Ensure the string has enough digits for the decimal placement
            while amount_str.len() <= self.resolution as usize {
                amount_str.insert(0, '0');
            }
            let decimal_index = amount_str.len() - self.resolution as usize;
            amount_str.insert(decimal_index, '.');
        }

        if self.amount < 0 {
            write!(f, "-{}", amount_str)
        } else {
            write!(f, "{}", amount_str)
        }
    }
}

// -----------------
// -- BOILERPLATE --
// -----------------

impl Add for Money {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let (amount_self, amount_other, resolution) =
            self.align_resolution(&rhs);
        Self {
            amount: amount_self + amount_other,
            resolution,
        }
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        let (amount_self, amount_other, resolution) =
            self.align_resolution(&rhs);
        self.amount = amount_self + amount_other;
        self.resolution = resolution;
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

impl Neg for Money {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            amount: -self.amount,
            resolution: self.resolution,
        }
    }
}

impl PartialEq<Self> for Money {
    fn eq(&self, other: &Self) -> bool {
        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self == amount_other
    }
}

impl Eq for Money {}

impl PartialEq<f64> for Money {
    fn eq(&self, &other: &f64) -> bool {
        (self.to_f64() - other).abs() < f64::EPSILON
    }
}

impl PartialEq<f64> for &Money {
    fn eq(&self, &other: &f64) -> bool {
        (self.to_f64() - other).abs() < f64::EPSILON
    }
}

impl PartialOrd for Money {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self.partial_cmp(&amount_other)
    }
}

impl PartialOrd<f64> for Money {
    fn partial_cmp(&self, &other: &f64) -> Option<std::cmp::Ordering> {
        self.to_f64().partial_cmp(&other)
    }
}

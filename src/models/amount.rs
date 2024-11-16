use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use anyhow::{bail, Error};

#[derive(Debug, Clone, Copy, Hash)]
pub struct Amount {
    amount: i64,
    resolution: u32,
    currency: u32, // map elsewhere; e.g. 0 -> 'USD', 1 -> 'CAD', etc.
}

impl Amount {
    pub fn new(amount: i64, resolution: u32, currency_identifier: u32) -> Self {
        Self {
            amount,
            resolution,
            currency: currency_identifier,
        }
    }

    pub fn new_from_str(input: String, identifier: u32) -> Result<Self, Error> {
        // Split the input string by the decimal point, if it exists
        let parts: Vec<&str> = input.split('.').collect();
        let (amount, resolution) = match parts.len() {
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
            amount,
            resolution,
            currency: identifier,
        })
    }

    pub fn raw_amt(&self) -> i64 {
        self.amount
    }

    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }

    pub fn is_neg(&self) -> bool {
        self.amount < 0
    }

    pub fn ident(&self) -> u32 {
        self.currency
    }

    fn align_resolution(&self, other: &Amount) -> (i64, i64, u32) {
        let max_resolution = self.resolution.max(other.resolution);
        let factor_self = 10i64.pow(max_resolution - self.resolution);
        let factor_other = 10i64.pow(max_resolution - other.resolution);

        (
            self.amount * factor_self,
            other.amount * factor_other,
            max_resolution,
        )
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if self.currency == other.currency {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&other);
            Self {
                amount: amount_self + amount_other,
                resolution,
                currency: self.currency.clone(),
            }
        } else {
            panic!("mismatched currency addition");
        }
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        if self.currency == rhs.currency {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&rhs);
            self.amount = amount_self + amount_other;
            self.resolution = resolution;
        } else {
            panic!("mismatched currency addition");
        }
    }
}

impl Sub for Amount {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        if self.currency == other.currency {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&other);
            Self {
                amount: amount_self - amount_other,
                resolution,
                currency: self.currency.clone(),
            }
        } else {
            panic!("mismatched currency subtraction");
        }
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        if self.currency == rhs.currency {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&rhs);
            self.amount = amount_self - amount_other;
            self.resolution = resolution;
        } else {
            panic!("mismatched currency subtraction");
        }
    }
}

impl Neg for Amount {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            amount: -self.amount,
            resolution: self.resolution,
            currency: self.currency,
        }
    }
}

impl PartialEq for Amount {
    fn eq(&self, other: &Self) -> bool {
        if self.currency != other.currency {
            return false;
        }

        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self == amount_other
    }
}

impl Eq for Amount {}

impl fmt::Display for Amount {
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

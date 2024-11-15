use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Debug, Clone, Copy)]
pub struct Currency {
    amount: i64,
    resolution: u32,
    identifier: u32, // map elsewhere; e.g. 0 -> 'USD', 1 -> 'CAD', etc.
}

impl Currency {
    // Constructor
    pub fn new(amount: i64, resolution: u32, identifier: u32) -> Self {
        Self {
            amount,
            resolution,
            identifier,
        }
    }

    // Constructor from String
    pub fn from_string(input: &str, identifier: u32) -> Self {
        let sanitized: String = input.chars().filter(|&c| c != ',').collect();
        if !sanitized.chars().all(|c| c.is_digit(10) || c == '.') {
            panic!("Invalid number format: {}", input);
        }

        let parts: Vec<&str> = sanitized.split('.').collect();
        let amount;
        let resolution;

        match parts.len() {
            1 => {
                // No decimal point
                amount = parts[0].parse::<i64>().expect("Failed to parse integer part");
                resolution = 0;
            }
            2 => {
                // One decimal point
                let integer_part = parts[0];
                let fractional_part = parts[1];

                resolution = fractional_part.len() as u32;
                let combined = format!("{}{}", integer_part, fractional_part);
                amount = combined
                    .parse::<i64>()
                    .expect("Failed to parse combined integer and fractional parts");
            }
            _ => panic!("Invalid number format: {}", input),
        }

        Self {
            amount,
            resolution,
            identifier,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }

    pub fn ident(&self) -> u32 {
        self.identifier
    }

    // Arithmetic operations
    fn align_resolution(&self, other: &Currency) -> (i64, i64, u32) {
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

impl Add for Currency {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if self.identifier == other.identifier {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&other);
            Self {
                amount: amount_self + amount_other,
                resolution,
                identifier: self.identifier.clone(),
            }
        } else {
            panic!("mismatched currency addition");
        }
    }
}

impl AddAssign for Currency {
    fn add_assign(&mut self, rhs: Self) {
        if self.identifier == rhs.identifier {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&rhs);
            self.amount = amount_self + amount_other;
            self.resolution = resolution;
        } else {
            panic!("mismatched currency addition");
        }
    }
}

impl Sub for Currency {
    type Output = Self;

    fn sub(&self, other: Self) -> Self {
        if self.identifier == other.identifier {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&other);
            Self {
                amount: amount_self - amount_other,
                resolution,
                identifier: self.identifier.clone(),
            }
        } else {
            panic!("mismatched currency subtraction");
        }
    }
}

impl SubAssign for Currency {
    fn sub_assign(&mut self, rhs: Self) {
        if self.identifier == rhs.identifier {
            let (amount_self, amount_other, resolution) =
                self.align_resolution(&rhs);
            self.amount = amount_self - amount_other;
            self.resolution = resolution;
        } else {
            panic!("mismatched currency subtraction");
        }
    }
}

impl PartialEq for Currency {
    fn eq(&self, other: &Self) -> bool {
        if self.identifier != other.identifier {
            return false;
        }

        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self == amount_other
    }
}

impl Eq for Currency {}

impl fmt::Display for Currency {
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

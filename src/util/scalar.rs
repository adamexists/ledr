/* Copyright (C) 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use anyhow::{bail, Error};
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

const MAX_RESOLUTION: u32 = 32; // TODO: Document.

/// A general-purpose number, capable of holding an exact decimal value, backed
/// by integer arithmetic and not float arithmetic.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scalar {
    amount: i128,
    resolution: u32,
}

pub const ZERO: Scalar = Scalar {
    amount: 0,
    resolution: 0,
};

impl Scalar {
    pub fn new(amount: i128, resolution: u32) -> Self {
        Self { amount, resolution }
    }

    pub fn from_i128(amount: i128) -> Self {
        Self::new(amount, 0)
    }

    pub fn from_str(amount: &str) -> Result<Self, Error> {
        // Remove all commas from the input string
        let sanitized_amount: String = amount.chars().filter(|&c| c != ',').collect();

        // Split the sanitized string by the decimal point, if it exists
        let parts: Vec<&str> = sanitized_amount.split('.').collect();
        let (amt, resolution) = match parts.len() {
            1 => {
                let amount = parts[0].parse::<i128>()?;
                (amount, 0)
            }
            2 => {
                let whole_part = parts[0];
                let decimal_part = parts[1];
                let resolution = decimal_part.len() as u32;
                let amount_str = format!("{}{}", whole_part, decimal_part);
                let amount = amount_str.parse::<i128>()?;
                (amount, resolution)
            }
            _ => bail!("Cannot parse amount"),
        };

        Ok(Self {
            amount: amt,
            resolution,
        })
    }
    pub fn amount(&self) -> i128 {
        self.amount
    }

    pub fn resolution(&self) -> u32 {
        self.resolution
    }

    pub fn set_resolution(&mut self, resolution: u32) {
        if resolution == self.resolution {
            return;
        }

        if resolution < self.resolution {
            // Truncate the underlying amount, losing precision
            let factor = 10i128.pow(self.resolution - resolution);
            self.amount /= factor;
        } else {
            // Pad the underlying amount with zeroes
            let factor = 10i128.pow(resolution - self.resolution);
            self.amount *= factor;
        }

        self.resolution = resolution;
    }

    fn align_resolution(&self, other: &Scalar) -> (i128, i128, u32) {
        let max_resolution = self.resolution.max(other.resolution);
        let factor_self = 10i128.pow(max_resolution - self.resolution);
        let factor_other = 10i128.pow(max_resolution - other.resolution);

        (
            self.amount * factor_self,
            other.amount * factor_other,
            max_resolution,
        )
    }

    pub fn abs(&self) -> Self {
        Self {
            amount: self.amount.abs(),
            resolution: self.resolution,
        }
    }

    pub fn negate(&mut self) {
        self.amount *= -1
    }

    fn reduce(&mut self, min_resolution: u32) {
        while self.amount % 10 == 0 && self.resolution > min_resolution {
            self.amount /= 10;
            self.resolution -= 1;
        }
    }
}

impl fmt::Display for Scalar {
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

        // Insert commas every three digits on the left of the decimal point
        if let Some(decimal_index) = amount_str.find('.') {
            let mut i = decimal_index as isize - 3;
            while i > 0 {
                amount_str.insert(i as usize, ',');
                i -= 3;
            }
        } else {
            // If there's no decimal point, add commas to the entire string
            let mut i = amount_str.len() as isize - 3;
            while i > 0 {
                amount_str.insert(i as usize, ',');
                i -= 3;
            }
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

impl Add for Scalar {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let (amount_self, amount_other, resolution) = self.align_resolution(&rhs);
        Self {
            amount: amount_self + amount_other,
            resolution,
        }
    }
}

impl AddAssign for Scalar {
    fn add_assign(&mut self, rhs: Self) {
        let (amount_self, amount_other, resolution) = self.align_resolution(&rhs);
        self.amount = amount_self + amount_other;
        self.resolution = resolution;
    }
}

impl Sub for Scalar {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let (amount_self, amount_other, resolution) = self.align_resolution(&rhs);
        Self {
            amount: amount_self - amount_other,
            resolution,
        }
    }
}

impl SubAssign for Scalar {
    fn sub_assign(&mut self, rhs: Self) {
        let (amount_self, amount_other, resolution) = self.align_resolution(&rhs);
        self.amount = amount_self - amount_other;
        self.resolution = resolution;
    }
}

impl Sum for Scalar {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

impl Mul for Scalar {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let initial_resolution = self.resolution.max(rhs.resolution);

        let product_amount = self.amount * rhs.amount;
        let product_resolution = self.resolution + rhs.resolution;

        let mut result = Self {
            amount: product_amount,
            resolution: product_resolution,
        };
        result.reduce(initial_resolution);
        result
    }
}

impl MulAssign for Scalar {
    fn mul_assign(&mut self, rhs: Self) {
        let initial_resolution = self.resolution.max(rhs.resolution);

        self.amount *= rhs.amount;
        self.resolution += rhs.resolution;

        // Reduce the resolution if possible without losing precision
        self.reduce(initial_resolution);
    }
}

impl Div for Scalar {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if rhs.amount == 0 {
            panic!("Attempt to divide by zero");
        }

        let mut result = self;
        result /= rhs;
        result
    }
}

impl Div<i128> for Scalar {
    type Output = Self;

    fn div(self, rhs: i128) -> Self::Output {
        if rhs == 0 {
            panic!("Attempt to divide by zero");
        }

        let scalar = Scalar::from_i128(rhs);
        self / scalar
    }
}

impl Div<Scalar> for i128 {
    type Output = Scalar;

    fn div(self, rhs: Scalar) -> Self::Output {
        let scalar = Scalar::from_i128(self);
        scalar / rhs
    }
}

impl DivAssign for Scalar {
    fn div_assign(&mut self, rhs: Self) {
        if rhs.amount == 0 {
            panic!("Attempt to divide by zero");
        }
        // Start with the initial values of the amounts and resolutions
        let (mut aligned_self, aligned_rhs, mut resolution) = self.align_resolution(&rhs);

        let initial_resolution = resolution;

        // Scale the dividend until the division yields an integer, or until we reach MAX_RESOLUTION
        while aligned_self % aligned_rhs != 0 && resolution < MAX_RESOLUTION {
            aligned_self *= 10;
            resolution += 1;
        }

        // Perform the division
        let quotient = aligned_self / aligned_rhs;

        // Update self with the result and the final resolution
        self.amount = quotient;
        self.resolution = resolution - initial_resolution;
        self.set_resolution(resolution);

        // Reduce to remove any unnecessary trailing zeros while keeping precision
        self.reduce(initial_resolution);
    }
}

impl Neg for Scalar {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            amount: -self.amount,
            resolution: self.resolution,
        }
    }
}

impl PartialEq<Self> for Scalar {
    fn eq(&self, other: &Self) -> bool {
        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self == amount_other
    }
}

impl PartialEq<i128> for Scalar {
    fn eq(&self, other: &i128) -> bool {
        let factor = 10i128.pow(self.resolution);

        if self.amount % factor != 0 {
            return false;
        }

        self.amount / factor == *other
    }
}

impl Eq for Scalar {}

impl PartialOrd for Scalar {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Scalar {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (amount_self, amount_other, _) = self.align_resolution(other);
        amount_self.cmp(&amount_other)
    }
}

impl PartialOrd<i128> for Scalar {
    fn partial_cmp(&self, other: &i128) -> Option<std::cmp::Ordering> {
        let other = Scalar::from_i128(*other);
        self.partial_cmp(&other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_resolution() {
        let mut money = Scalar::from_str("123.45").unwrap();
        assert_eq!(money.amount, 12345);
        assert_eq!(money.resolution, 2);
        money.set_resolution(0);
        assert_eq!(money.amount, 123);
        assert_eq!(money.resolution, 0);

        let mut money = Scalar::from_str("123.4567").unwrap();
        assert_eq!(money.amount, 1234567);
        assert_eq!(money.resolution, 4);
        money.set_resolution(6);
        assert_eq!(money.amount, 123456700);
        assert_eq!(money.resolution, 6);

        let mut money = Scalar::from_str("123.45").unwrap();
        assert_eq!(money.amount, 12345);
        assert_eq!(money.resolution, 2);
        money.set_resolution(2);
        assert_eq!(money.amount, 12345);
        assert_eq!(money.resolution, 2);
    }

    #[test]
    fn test_mul_with_zero() {
        let money = Scalar::from_str("123.45").unwrap();
        let zero = ZERO;
        let result = money * zero;
        assert_eq!(result.amount, 0);
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_same_resolution() {
        let money1 = Scalar::from_str("2.50").unwrap(); // 250, resolution 2
        let money2 = Scalar::from_str("3.00").unwrap(); // 300, resolution 2
        let result = money1 * money2;
        assert_eq!(result.amount, 750); // 75000 reduced to prior precision
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_different_resolution() {
        let money1 = Scalar::from_str("1.5").unwrap(); // 15, resolution 1
        let money2 = Scalar::from_str("2.00").unwrap(); // 200, resolution 2
        let result = money1 * money2;
        assert_eq!(result.amount, 300); // 3000 reduced to prior precision
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_negative_values() {
        let money1 = Scalar::from_str("-2.50").unwrap(); // -250, resolution 2
        let money2 = Scalar::from_str("4.00").unwrap(); // 400, resolution 2
        let result = money1 * money2;
        assert_eq!(result.amount, -1000); // -100000 reduced to prior precision
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_both_negative() {
        let money1 = Scalar::from_str("-3.25").unwrap(); // -325, resolution 2
        let money2 = Scalar::from_str("-2.00").unwrap(); // -200, resolution 2
        let result = money1 * money2;
        assert_eq!(result.amount, 650); // 65000 reduced to prior precision
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_large_numbers() {
        let money1 = Scalar::from_str("1000.00").unwrap(); // 100000, resolution 2
        let money2 = Scalar::from_str("2000.00").unwrap(); // 200000, resolution 2
        let result = money1 * money2;
        assert_eq!(result.amount, 200000000); // 20,000,000,000 reduced to prior precision
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_mul_high_resolution() {
        let money1 = Scalar::from_str("0.1234").unwrap(); // 1234, resolution 4
        let money2 = Scalar::from_str("0.5678").unwrap(); // 5678, resolution 4
        let result = money1 * money2;
        assert_eq!(result.amount, 1234 * 5678); // 7006652
        assert_eq!(result.resolution, 8); // 4 + 4 = 8
    }

    #[test]
    fn test_mul_edge_case_high_precision() {
        let money1 = Scalar::from_str("0.0001").unwrap(); // 1, resolution 4
        let money2 = Scalar::from_str("0.0002").unwrap(); // 2, resolution 4
        let result = money1 * money2;
        assert_eq!(result.amount, 1 * 2); // 2
        assert_eq!(result.resolution, 8); // 4 + 4 = 8
    }

    #[test]
    fn test_scalar_div_scalar_same_resolution() {
        let scalar1 = Scalar::from_str("10.00").unwrap(); // 1000, resolution 2
        let scalar2 = Scalar::from_str("2.00").unwrap(); // 200, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 500); // 10.00 / 2.00 = 5.00 -> 500
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_scalar_div_scalar_different_resolutions() {
        let scalar1 = Scalar::from_str("10.5").unwrap(); // 105, resolution 1
        let scalar2 = Scalar::from_str("0.5").unwrap(); // 5, resolution 1
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 210); // 10.5 / 0.5 = 21.0 -> 210
        assert_eq!(result.resolution, 1);
    }

    #[test]
    fn test_scalar_div_scalar_high_precision() {
        let scalar1 = Scalar::from_str("1.0000").unwrap(); // 10000, resolution 4
        let scalar2 = Scalar::from_str("0.25").unwrap(); // 25, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 40000); // 1.0000 / 0.25 = 4.0000 -> 40000
        assert_eq!(result.resolution, 4);
    }

    #[test]
    fn test_scalar_div_i128_no_resolution_change() {
        let scalar = Scalar::from_str("10.00").unwrap(); // 1000, resolution 2
        let result = scalar / 2;
        assert_eq!(result.amount, 500); // 10.00 / 2 = 5.00 -> 500
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_scalar_div_i128_with_resolution_increase() {
        let scalar = Scalar::from_str("1").unwrap(); // 1, resolution 0
        let result = scalar / 2;
        assert_eq!(result.amount, 5); // 1 / 2 = 0.5 -> 5
        assert_eq!(result.resolution, 1);
    }

    #[test]
    fn test_scalar_div_i128_large_number() {
        let scalar = Scalar::from_str("1000.00").unwrap(); // 100000, resolution 2
        let result = scalar / 250;
        assert_eq!(result.amount, 400); // 1000.00 / 250 = 4.00 -> 400
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_scalar_div_i128_small_number() {
        let scalar = Scalar::from_str("0.001").unwrap(); // 1, resolution 3
        let result = scalar / 2;
        assert_eq!(result.amount, 5); // 0.001 / 2 = 0.0005 -> 5
        assert_eq!(result.resolution, 4);
    }

    #[test]
    #[should_panic(expected = "Attempt to divide by zero")]
    fn test_div_by_zero_scalar() {
        let scalar1 = Scalar::from_str("10.00").unwrap();
        let scalar2 = Scalar::from_str("0.00").unwrap();
        let _ = scalar1 / scalar2; // Should panic
    }

    #[test]
    #[should_panic(expected = "Attempt to divide by zero")]
    fn test_div_by_zero_i128() {
        let scalar = Scalar::from_str("10.00").unwrap();
        let _ = scalar / 0; // Should panic
    }

    #[test]
    fn test_div_negative_scalar() {
        let scalar1 = Scalar::from_str("-10.00").unwrap(); // -1000, resolution 2
        let scalar2 = Scalar::from_str("2.00").unwrap(); // 200, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, -500); // -10.00 / 2.00 = -5.00 -> -500
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_div_negative_i128() {
        let scalar = Scalar::from_str("10.00").unwrap(); // 1000, resolution 2
        let result = scalar / -2;
        assert_eq!(result.amount, -500); // 10.00 / -2 = -5.00 -> -500
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_div_both_negative() {
        let scalar1 = Scalar::from_str("-10.00").unwrap(); // -1000, resolution 2
        let scalar2 = Scalar::from_str("-2.00").unwrap(); // -200, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 500); // -10.00 / -2.00 = 5.00 -> 500
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_div_assign_simple_case() {
        let mut scalar1 = Scalar::from_str("10.00").unwrap(); // 1000, resolution 2
        let scalar2 = Scalar::from_str("2.00").unwrap(); // 200, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 500); // 10.00 / 2.00 = 5.00 -> 500
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    fn test_div_assign_with_different_resolutions() {
        let mut scalar1 = Scalar::from_str("10.50").unwrap(); // 105, resolution 2
        let scalar2 = Scalar::from_str("0.5").unwrap(); // 5, resolution 1
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 2100); // 10.5 / 0.5 = 21.0 -> 2100
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    fn test_div_assign_result_with_higher_resolution() {
        let mut scalar1 = Scalar::from_str("1.0000").unwrap(); // 10000, resolution 4
        let scalar2 = Scalar::from_str("0.25").unwrap(); // 25, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 40000); // 1.0000 / 0.25 = 4.0000 -> 40000
        assert_eq!(scalar1.resolution, 4);
    }

    #[test]
    fn test_div_assign_large_numbers() {
        let mut scalar1 = Scalar::from_str("1000000.00").unwrap(); // 100000000, resolution 2
        let scalar2 = Scalar::from_str("1000.00").unwrap(); // 100000, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 100000); // 1000000.00 / 1000.00 = 1000.00 -> 1000
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    fn test_div_assign_small_numbers() {
        let mut scalar1 = Scalar::from_str("0.001").unwrap(); // 1, resolution 3
        let scalar2 = Scalar::from_str("0.0005").unwrap(); // 5, resolution 4
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 20000); // 0.001 / 0.0005 = 2.000 -> 2000
        assert_eq!(scalar1.resolution, 4);
    }

    #[test]
    fn test_div_assign_negative_numbers() {
        let mut scalar1 = Scalar::from_str("-10.00").unwrap(); // -1000, resolution 2
        let scalar2 = Scalar::from_str("2.00").unwrap(); // 200, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, -500); // -10.00 / 2.00 = -5.00 -> -500
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    fn test_div_assign_both_negative() {
        let mut scalar1 = Scalar::from_str("-10.00").unwrap(); // -1000, resolution 2
        let scalar2 = Scalar::from_str("-2.00").unwrap(); // -200, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 500); // -10.00 / -2.00 = 5.00 -> 500
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    fn test_div_assign_with_zero_amount() {
        let mut scalar1 = Scalar::from_str("0.00").unwrap(); // 0, resolution 2
        let scalar2 = Scalar::from_str("1.00").unwrap(); // 100, resolution 2
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 0); // 0.00 / 1.00 = 0.00 -> 0
        assert_eq!(scalar1.resolution, 2);
    }

    #[test]
    #[should_panic(expected = "Attempt to divide by zero")]
    fn test_div_assign_by_zero() {
        let mut scalar1 = Scalar::from_str("10.00").unwrap(); // 1000, resolution 2
        let scalar2 = Scalar::from_str("0.00").unwrap(); // 0, resolution 2
        scalar1 /= scalar2; // Should panic
    }

    #[test]
    fn test_div_assign_high_precision() {
        let mut scalar1 = Scalar::from_str("0.00012345").unwrap(); // 12345, resolution 8
        let scalar2 = Scalar::from_str("0.0001").unwrap(); // 1, resolution 4
        scalar1 /= scalar2;
        assert_eq!(scalar1.amount, 123450000); // 0.00012345 / 0.0001 = 1.2345
        assert_eq!(scalar1.resolution, 8); // preserved from highest-precision operand
    }

    #[test]
    fn test_mul_large_numbers_extreme() {
        let money1 = Scalar::from_str("999999999999.99").unwrap(); // 99999999999999, resolution 2
        let money2 = Scalar::from_str("0.0000000001").unwrap(); // 1, resolution 10
        let result = money1 * money2;
        assert_eq!(result.amount, 99999999999999); // Result should retain as much precision as possible
        assert_eq!(result.resolution, 12); // 2 + 10 = 12
    }

    #[test]
    fn test_mul_minimal_numbers() {
        let money1 = Scalar::from_str("0.0000000001").unwrap(); // 1, resolution 10
        let money2 = Scalar::from_str("0.0000000001").unwrap(); // 1, resolution 10
        let result = money1 * money2;
        assert_eq!(result.amount, 1); // 1 * 1 = 1
        assert_eq!(result.resolution, 20); // 10 + 10 = 20
    }

    #[test]
    fn test_mul_max_resolution_limit() {
        let money1 = Scalar::from_str("1.234567890123456789").unwrap(); // 1234567890123456789, resolution 18
        let money2 = Scalar::from_str("0.000000000000000001").unwrap(); // 1, resolution 18
        let result = money1 * money2;
        assert_eq!(result.amount, 1234567890123456789); // Retain maximum precision
        assert_eq!(result.resolution, 36); // 18 + 18 = 36
    }

    #[test]
    fn test_mul_max_resolution_edge_case() {
        let money1 = Scalar::from_str("1").unwrap(); // 1, resolution 0
        let money2 = Scalar::from_str("0.000000000000000001").unwrap(); // 1, resolution 18
        let result = money1 * money2;
        assert_eq!(result.amount, 1); // Minimal precision case
        assert_eq!(result.resolution, 18); // 0 + 18 = 18
    }

    #[test]
    fn test_mul_large_reduce() {
        let money1 = Scalar::from_str("1000000.000").unwrap(); // 1000000, resolution 3
        let money2 = Scalar::from_str("12345.000").unwrap(); // 12345, resolution 3
        let result = money1 * money2;
        assert_eq!(result.amount, 12345000000000);
        assert_eq!(result.resolution, 3);
    }

    #[test]
    fn test_div_large_numbers_with_precision_loss() {
        let scalar1 = Scalar::from_str("12345678901234567890.00").unwrap(); // 1234567890123456789000, resolution 2
        let scalar2 = Scalar::from_str("3.00").unwrap(); // 300, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 411522630041152263000);
        assert_eq!(result.resolution, 2);
    }

    #[test]
    fn test_div_large_numbers_with_larger_divisor() {
        let scalar1 = Scalar::from_str("3.00").unwrap(); // 300, resolution 2
        let scalar2 = Scalar::from_str("12345678901234.00").unwrap(); // 1234567890123400, resolution 2
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 243000002187011197);
        assert_eq!(result.resolution, 30);
    }

    #[test]
    fn test_div_small_numbers_high_resolution() {
        let scalar1 = Scalar::from_str("0.000000000000001").unwrap(); // 1, resolution 15
        let scalar2 = Scalar::from_str("0.0000000001").unwrap(); // 1, resolution 10
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 10000000000); // 1, resolution 15
        assert_eq!(result.resolution, 15); // highest resolution among operands
    }

    #[test]
    fn test_div_near_zero_with_high_resolution() {
        let scalar1 = Scalar::from_str("0.0000001").unwrap(); // 1, resolution 7
        let scalar2 = Scalar::from_str("1000000").unwrap(); // 1000000, resolution 0
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 1); // Near-zero division
        assert_eq!(result.resolution, 13);
    }

    #[test]
    fn test_mul_extremely_small_numbers() {
        let scalar1 = Scalar::from_str("0.0000000000000001").unwrap(); // 1, resolution 16
        let scalar2 = Scalar::from_str("0.0000000000000001").unwrap(); // 1, resolution 16
        let result = scalar1 * scalar2;
        assert_eq!(result.amount, 1); // 1 * 1 = 1
        assert_eq!(result.resolution, 32); // 16 + 16 = 32
    }

    #[test]
    fn test_div_high_resolution_numbers() {
        let scalar1 = Scalar::from_str("0.123456789012345678").unwrap(); // 123456789012345678, resolution 18
        let scalar2 = Scalar::from_str("0.000000000000000001").unwrap(); // 1, resolution 18
        let result = scalar1 / scalar2;
        assert_eq!(result.amount, 123456789012345678000000000000000000);
        assert_eq!(result.resolution, 18); // Precision maintained
    }

    #[test]
    fn test_reduce_no_trailing_zeros() {
        let mut scalar = Scalar {
            amount: 123,
            resolution: 2,
        };
        scalar.reduce(1);
        assert_eq!(scalar.amount, 123);
        assert_eq!(scalar.resolution, 2);
    }

    #[test]
    fn test_reduce_with_trailing_zeros() {
        let mut scalar = Scalar {
            amount: 1200,
            resolution: 3,
        };
        scalar.reduce(1);
        assert_eq!(scalar.amount, 12);
        assert_eq!(scalar.resolution, 1);
    }

    #[test]
    fn test_reduce_with_min_resolution_limit() {
        let mut scalar = Scalar {
            amount: 1000,
            resolution: 4,
        };
        scalar.reduce(2);
        assert_eq!(scalar.amount, 10); // Reduced from 1000 to 10
        assert_eq!(scalar.resolution, 2); // Stopped at min_resolution
    }

    #[test]
    fn test_reduce_when_min_resolution_equals_current() {
        let mut scalar = Scalar {
            amount: 100,
            resolution: 2,
        };
        scalar.reduce(2);
        assert_eq!(scalar.amount, 100); // Should not change
        assert_eq!(scalar.resolution, 2); // Should not change
    }

    #[test]
    fn test_reduce_minimal_case() {
        let mut scalar = Scalar {
            amount: 0,
            resolution: 0,
        };
        scalar.reduce(0);
        assert_eq!(scalar.amount, 0); // No change for amount of 0
        assert_eq!(scalar.resolution, 0); // No change for resolution of 0
    }

    #[test]
    fn test_display() {
        let money = Scalar::from_str("12345.6789").unwrap(); // 123456789, resolution 4
        assert_eq!(money.to_string(), "12,345.6789");

        let negative_money = Scalar::from_str("-1000000.50").unwrap(); // -100000050, resolution 2
        assert_eq!(negative_money.to_string(), "-1,000,000.50");

        let zero_money = Scalar::from_str("0.00").unwrap(); // 0, resolution 2
        assert_eq!(zero_money.to_string(), "0.00")
    }
}
